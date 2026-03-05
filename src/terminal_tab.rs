use gtk4 as gtk;
use gtk::prelude::*;
use std::time::Duration;
use std::io::{Read, Write};
use portable_pty::{CommandBuilder, NativePtySystem, PtySystem, PtySize};

use crate::ssh::{connect_ssh, SshEvent};
use crate::terminal_state::TerminalState;

pub enum ConnectionControl {
    Input(Vec<u8>),
    Resize(u32, u32, u32, u32),
    SetBackend(TerminalBackend),
}

pub enum TerminalBackend {
    Ssh(ssh2::Channel),
    Local(LocalPty),
}

pub struct LocalPty {
    pub master: Box<dyn portable_pty::MasterPty + Send>,
    pub writer: Box<dyn std::io::Write + Send>,
}

fn spawn_local_shell(cols: u16, rows: u16, cwd: Option<&std::path::Path>) -> Result<(LocalPty, Box<dyn std::io::Read + Send>), Box<dyn std::error::Error + Send + Sync>> {
    // On macOS, TMPDIR can be very long (>100 chars) which causes ENAMETOOLONG (error 40)
    // when portable-pty creates unix sockets. Use a shorter temp dir.
    #[cfg(target_os = "macos")]
    {
        if let Ok(tmpdir) = std::env::var("TMPDIR") {
            if tmpdir.len() > 80 {
                unsafe { std::env::set_var("TMPDIR", "/tmp"); }
            }
        }
    }

    let pty_system = NativePtySystem::default();
    let pair = pty_system.openpty(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
    let mut cmd = CommandBuilder::new(shell);
    // Use explicitly specified dir, then fall back to the process cwd.
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    } else if let Ok(d) = std::env::current_dir() {
        cmd.cwd(d);
    }
    let _child = pair.slave.spawn_command(cmd)?;

    let reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;

    Ok((LocalPty {
        master: pair.master,
        writer,
    }, reader))
}

pub fn add_terminal_tab(
    stack: &gtk::Widget, // Using Widget to be flexible for now, but expecting Notebook or similar
    settings: crate::config::ConnectionSettings,
    override_pass: Option<String>,
    is_local: bool,
) {
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .overlay_scrolling(true)
        .build();

    let text_view = gtk::TextView::builder()
        .editable(false)
        .monospace(true)
        .cursor_visible(true)
        .focusable(true)
        .can_focus(true)
        .wrap_mode(gtk::WrapMode::None)
        .accepts_tab(false)
        .build();

    text_view.add_css_class("terminal-view");

    text_view.set_direction(gtk::TextDirection::Ltr);
    let provider = gtk::CssProvider::new();
    let bg = &settings.bg_color;
    let fg = &settings.fg_color;
    let css = format!(
        ".terminal-view {{ background-color: {0}; color: {1}; }} \
         .terminal-view text {{ background-color: {0}; color: {1}; font-size: {2}pt; }} \
         .terminal-view selection {{ background-color: #3584e4; color: #ffffff; }}",
        bg, fg, settings.font_size
    );
    provider.load_from_data(&css);
    gtk::style_context_add_provider_for_display(
        &text_view.display(),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 500,
    );

    scrolled.set_child(Some(&text_view));

    let dummy_label = gtk::Label::new(Some(&settings.name));
    
    if let Some(nb) = stack.downcast_ref::<gtk::Notebook>() {
        let page_num = nb.append_page(&scrolled, Some(&dummy_label));
        nb.set_tab_reorderable(&scrolled, true);
        nb.set_tab_detachable(&scrolled, true);
        // Switch to the newly created tab immediately.
        nb.set_current_page(Some(page_num));
    }

    // Focus the TextView so keyboard input works immediately without requiring
    // a mouse click.  Do it on the next idle tick so the widget is fully mapped.
    let tv_focus = text_view.clone();
    gtk::glib::idle_add_local_once(move || {
        tv_focus.grab_focus();
    });

    let (input_tx, input_rx) = flume::unbounded::<ConnectionControl>();
    let (output_tx, output_rx) = flume::unbounded::<Vec<u8>>();
    // Shared current terminal size — written by the resize timer (main thread),
    // read by the backend thread when SetBackend fires so SSH/local both get
    // the correct size immediately on connect regardless of timing.
    let current_size = std::sync::Arc::new(std::sync::Mutex::new((80u32, 24u32)));

    let palette = settings.palette.clone();
    let mut state = TerminalState::new(text_view.downgrade(), dummy_label.downgrade(), palette);
    // Give TerminalState a sender so embedded widgets can push input back to the PTY.
    state.pty_input_tx = Some(input_tx.clone()).map(|tx| {
        let (wtx, wrx) = flume::unbounded::<Vec<u8>>();
        // Bridge: forward widget event bytes as ConnectionControl::Input
        let itx = tx.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            while let Ok(data) = wrx.try_recv() {
                let _ = itx.send(ConnectionControl::Input(data));
            }
            gtk::glib::ControlFlow::Continue
        });
        wtx
    });
    let state_rc = std::rc::Rc::new(std::cell::RefCell::new(state));

    let state_for_output = state_rc.clone();
    // The default VTE OSC buffer is 1024 bytes — far too small for base64-encoded
    // images.  Use a 4 MB buffer so the full image payload reaches osc_dispatch.
    let mut parser: vte::Parser<{4 * 1024 * 1024}> = Default::default();
    gtk::glib::timeout_add_local(Duration::from_millis(10), move || {
        let mut received = false;
        while let Ok(data) = output_rx.try_recv() {
            let mut state = state_for_output.borrow_mut();
            parser.advance(&mut *state, &data);
            received = true;
        }
        if received {
            // Defer the scroll until GTK has re-laid out the buffer changes.
            // Calling scroll_to_iter immediately after buffer edits uses stale
            // layout geometry and scrolls to the wrong position.
            let state_scroll = state_for_output.clone();
            gtk::glib::idle_add_local_once(move || {
                state_scroll.borrow().update_visual_cursor();
            });
        }
        gtk::glib::ControlFlow::Continue
    });

    // Capture-phase controller on the ScrolledWindow: intercepts Ctrl+key and
    // Cmd+key BEFORE any child widget (entry, button, etc.) can eat them.
    // This ensures Ctrl+C always sends SIGINT to the PTY.
    let itx_capture = input_tx.clone();
    let tv_capture = text_view.clone();
    let capture_controller = gtk::EventControllerKey::new();
    capture_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    capture_controller.connect_key_pressed(move |_, keyval, _, state| {
        let is_ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
        let is_super = state.contains(gtk::gdk::ModifierType::SUPER_MASK)
            || state.contains(gtk::gdk::ModifierType::META_MASK);

        if is_super {
            match keyval {
                gtk::gdk::Key::c | gtk::gdk::Key::C => {
                    tv_capture.emit_by_name::<()>("copy-clipboard", &[]);
                    return gtk::glib::Propagation::Stop;
                }
                gtk::gdk::Key::v | gtk::gdk::Key::V => {
                    let clipboard = tv_capture.clipboard();
                    let itx_v = itx_capture.clone();
                    clipboard.read_text_async(gtk::gio::Cancellable::NONE, move |res| {
                        if let Ok(Some(text)) = res {
                            let _ = itx_v.send(ConnectionControl::Input(text.into_bytes()));
                        }
                    });
                    return gtk::glib::Propagation::Stop;
                }
                _ => {}
            }
        }

        // Ctrl+key: always send to PTY (Ctrl+C, Ctrl+Z, etc.) even if a child has focus
        if is_ctrl {
            let bytes = crate::terminal_state::keyval_to_bytes(keyval, state);
            if let Some(data) = bytes {
                let _ = itx_capture.send(ConnectionControl::Input(data));
                return gtk::glib::Propagation::Stop;
            }
        }

        gtk::glib::Propagation::Proceed
    });
    scrolled.add_controller(capture_controller);

    // Bubble-phase controller on the TextView: handles normal typing
    let itx_key = input_tx.clone();
    let key_controller = gtk::EventControllerKey::new();
    key_controller.connect_key_pressed(move |_, keyval, _, state| {
        let bytes = crate::terminal_state::keyval_to_bytes(keyval, state);
        if let Some(data) = bytes {
            let _ = itx_key.send(ConnectionControl::Input(data));
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    text_view.add_controller(key_controller);

    let tv_for_resize = text_view.clone();
    let scrolled_for_resize = scrolled.clone();
    let itx_resize = input_tx.clone();
    let mut last_cols = 0;
    let mut last_rows = 0;
    
    let state_for_resize = state_rc.clone();
    let current_size_thread = current_size.clone(); // cloned before the timer moves current_size
    gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
        // scrolled_for_resize.width/height() is the allocation of the ScrolledWindow
        // widget itself, which equals the visible viewport size.  This is always correct —
        // never affected by content height or scrollbar presence.
        let width  = scrolled_for_resize.width();
        let height = scrolled_for_resize.height();
        if width > 0 && height > 0 {
            // Measure using the exact font the TextView is already rendering with.
            // Use layout.size() which returns Pango units (never rounded to integers)
            // and divide by pango::SCALE to get exact float pixel dimensions.
            // pixel_size() rounds to integers — for fractional char heights that makes
            // rows/cols 1 wrong, producing the persistent empty strip at bottom/right.
            let sample = "MMMMMMMMMM\nMMMMMMMMMM\nMMMMMMMMMM\nMMMMMMMMMM\nMMMMMMMMMM\nMMMMMMMMMM\nMMMMMMMMMM\nMMMMMMMMMM\nMMMMMMMMMM\nMMMMMMMMMM";
            let layout = tv_for_resize.create_pango_layout(Some(sample));
            let (w_pu, h_pu) = layout.size(); // Pango units
            let scale = gtk::pango::SCALE as f32;
            let char_w = (w_pu as f32 / scale / 10.0).max(1.0);
            let char_h = (h_pu as f32 / scale / 10.0).max(1.0);

            let cols = (width  as f32 / char_w).floor().max(1.0) as u32;
            let rows = (height as f32 / char_h).floor().max(1.0) as u32;

            if cols != last_cols || rows != last_rows {
                last_cols = cols;
                last_rows = rows;
                *current_size.lock().unwrap() = (cols, rows);
                state_for_resize.borrow_mut().resize(cols as usize, rows as usize);
                let _ = itx_resize.send(ConnectionControl::Resize(cols, rows, width as u32, height as u32));
            }
        }
        gtk::glib::ControlFlow::Continue
    });

    let s_clone = settings.clone();
    let final_pass = override_pass.clone();
    let (event_tx, event_rx) = flume::unbounded::<SshEvent>();

    // Shared Emulation Loop
    let output_tx_shared = output_tx.clone();
    let input_rx_shared = input_rx.clone();
    std::thread::spawn(move || {
        let mut backend: Option<TerminalBackend> = None;
        let mut buffer = [0u8; 8192];

        loop {
            if let Some(ref mut b) = backend {
                match b {
                    TerminalBackend::Local(_) => {
                        // Handled by background thread
                    }
                    TerminalBackend::Ssh(channel) => {
                        match channel.read(&mut buffer) {
                            Ok(0) => { backend = None; let _ = output_tx_shared.send(b"\r\n[SSH connection closed]\r\n".to_vec()); }
                            Ok(n) => { let _ = output_tx_shared.send(buffer[..n].to_vec()); }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                            Err(_) => { backend = None; }
                        }
                    }
                }
            }

            while let Ok(ctrl) = input_rx_shared.try_recv() {
                match ctrl {
                    ConnectionControl::SetBackend(b) => {
                        backend = Some(b);
                        // Immediately resize to the current window size.
                        // This is critical for SSH: the resize timer fires long before
                        // auth completes, so the Resize message was dropped (no backend
                        // yet). Now that we have a backend, apply the known size.
                        let (cols, rows) = *current_size_thread.lock().unwrap();
                        if cols > 0 && rows > 0 {
                            if let Some(ref mut b) = backend {
                                match b {
                                    TerminalBackend::Local(local) => {
                                        let _ = local.master.resize(portable_pty::PtySize {
                                            rows: rows as u16,
                                            cols: cols as u16,
                                            pixel_width: 0,
                                            pixel_height: 0,
                                        });
                                    }
                                    TerminalBackend::Ssh(channel) => {
                                        let _ = channel.request_pty_size(cols, rows, None, None);
                                    }
                                }
                            }
                        }
                    }
                    ConnectionControl::Input(data) => {
                        if let Some(ref mut b) = backend {
                            match b {
                                TerminalBackend::Local(local) => { let _ = local.writer.write_all(&data); let _ = local.writer.flush(); }
                                TerminalBackend::Ssh(channel) => { let _ = channel.write_all(&data); let _ = channel.flush(); }
                            }
                        }
                    }
                    ConnectionControl::Resize(cols, rows, width_px, height_px) => {
                        if let Some(ref mut b) = backend {
                            match b {
                                TerminalBackend::Local(local) => {
                                    let _ = local.master.resize(portable_pty::PtySize {
                                        rows: rows as u16,
                                        cols: cols as u16,
                                        pixel_width: width_px as u16,
                                        pixel_height: height_px as u16,
                                    });
                                }
                                TerminalBackend::Ssh(channel) => {
                                    let _ = channel.request_pty_size(cols, rows, Some(width_px), Some(height_px));
                                }
                            }
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    if is_local {
        let itx_l = input_tx.clone();
        let otx_l = output_tx.clone();
        let initial_dir = settings.initial_dir.clone();
        std::thread::spawn(move || {
            match spawn_local_shell(80, 24, initial_dir.as_deref()) {
                Ok((local, mut reader)) => {
                    let _ = otx_l.send(b"Local shell spawned.\r\n".to_vec());
                    let _ = itx_l.send(ConnectionControl::SetBackend(TerminalBackend::Local(local)));

                    let otx_bg = otx_l.clone();
                    std::thread::spawn(move || {
                        let mut buf = [0u8; 8192];
                        loop {
                            match reader.read(&mut buf) {
                                Ok(0) => { let _ = otx_bg.send(b"\r\n[Local shell closed]\r\n".to_vec()); break; }
                                Ok(n) => { let _ = otx_bg.send(buf[..n].to_vec()); }
                                Err(_) => break,
                            }
                        }
                    });
                }
                Err(e) => {
                    let _ = otx_l.send(format!("Failed to spawn local shell: {}\r\n", e).into_bytes());
                }
            }
        });
    } else {
        let itx_s = input_tx.clone();
        let otx_s = output_tx.clone();
        let otx_s2 = output_tx.clone();
        std::thread::spawn(move || {
            match connect_ssh(&s_clone, final_pass.as_deref().unwrap_or(""), Some(event_tx), otx_s2) {
                Ok((session, channel)) => {
                    let _ = otx_s.send(b"Connection established.\r\n".to_vec());
                    let _ = session.set_blocking(false);
                    let _ = itx_s.send(ConnectionControl::SetBackend(TerminalBackend::Ssh(channel)));
                }
                Err(e) => {
                    let _ = otx_s.send(format!("Connection failed: {}\r\n", e).as_bytes().to_vec());
                }
            }
        });
    }

    // EVENT LISTENER
    let tv_for_event = text_view.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
        while let Ok(event) = event_rx.try_recv() {
            match event {
                SshEvent::HostKeyVerify { host, port, fingerprint, response } => {
                    let win = match tv_for_event.root().and_then(|r| r.downcast::<gtk::Window>().ok()) {
                        Some(w) => w,
                        None => continue,
                    };
                    let dialog = gtk::AlertDialog::builder()
                        .message("SSH Host Key Verification")
                        .detail(&format!(
                            "The authenticity of host '{}:{}' can't be established.\n\nSHA256 Fingerprint: {}\n\nAre you sure you want to continue connecting?",
                            host, port, fingerprint
                        ))
                        .buttons(["No", "Yes"])
                        .cancel_button(0)
                        .default_button(1)
                        .modal(true)
                        .build();
                    let resp_tx = response.clone();
                    let h_clone = host.clone();
                    let fp_clone = fingerprint.clone();
                    dialog.choose(Some(&win), None::<&gtk::gio::Cancellable>, move |result| {
                        let approved = matches!(result, Ok(1));
                        if approved {
                            let mut current = crate::config::load_known_hosts();
                            current.retain(|kh| !(kh.host == h_clone && kh.port == port));
                            current.push(crate::config::KnownHost {
                                host: h_clone.clone(),
                                port,
                                fingerprint: fp_clone.clone(),
                            });
                            crate::config::save_known_hosts(&current);

                            crate::app_state::KNOWN_HOSTS_LIST.with(|cell| {
                                if let Some(list) = cell.borrow().upgrade() {
                                    crate::session_manager_ui::populate_known_hosts_list(&list);
                                }
                            });
                        }
                        let _ = resp_tx.send(approved);
                    });
                }
            }
        }
        gtk::glib::ControlFlow::Continue
    });
}
