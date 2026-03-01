use crate::config::{ConnectionSettings, THEMES, get_config_path};
use crate::app_state::{ACTIVE_TERMINALS, ActiveTerminal, update_active_terminals};
use crate::ssh::connect_ssh;
use crate::terminal_state::TerminalState;
use flume;
use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{glib, Label, Notebook, ScrolledWindow, TextView, CssProvider, EventControllerKey};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vte::Parser;

pub enum ConnectionControl {
    Input(Vec<u8>),
    Resize(u32, u32, u32, u32),
}

fn load_sessions() -> Vec<ConnectionSettings> {
    let path = get_config_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn save_sessions(sessions: &[ConnectionSettings]) {
    let path = get_config_path();
    if let Ok(content) = serde_json::to_string_pretty(sessions) {
        let _ = std::fs::write(path, content);
    }
}

fn keyval_to_bytes(keyval: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> Option<Vec<u8>> {
    let is_ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
    if is_ctrl && keyval == gtk::gdk::Key::space {
        return Some(vec![0]);
    }

    use gtk::gdk::Key;
    match keyval {
        Key::Return | Key::KP_Enter => Some(b"\r".to_vec()),
        Key::BackSpace => Some(b"\x08".to_vec()),
        Key::Tab => Some(b"\t".to_vec()),
        Key::Escape => Some(b"\x1b".to_vec()),
        Key::Left => Some(b"\x1b[D".to_vec()),
        Key::Right => Some(b"\x1b[C".to_vec()),
        Key::Up => Some(b"\x1b[A".to_vec()),
        Key::Down => Some(b"\x1b[B".to_vec()),
        Key::Home => Some(b"\x1b[H".to_vec()),
        Key::End => Some(b"\x1b[F".to_vec()),
        Key::Page_Up => Some(b"\x1b[5~".to_vec()),
        Key::Page_Down => Some(b"\x1b[6~".to_vec()),
        Key::Insert => Some(b"\x1b[2~".to_vec()),
        Key::Delete => Some(b"\x1b[3~".to_vec()),
        Key::F1 => Some(b"\x1bOP".to_vec()),
        Key::F2 => Some(b"\x1bOQ".to_vec()),
        Key::F3 => Some(b"\x1bOR".to_vec()),
        Key::F4 => Some(b"\x1bOS".to_vec()),
        Key::F5 => Some(b"\x1b[15~".to_vec()),
        Key::F6 => Some(b"\x1b[17~".to_vec()),
        Key::F7 => Some(b"\x1b[18~".to_vec()),
        Key::F8 => Some(b"\x1b[19~".to_vec()),
        Key::F9 => Some(b"\x1b[20~".to_vec()),
        Key::F10 => Some(b"\x1b[21~".to_vec()),
        Key::F11 => Some(b"\x1b[23~".to_vec()),
        Key::F12 => Some(b"\x1b[24~".to_vec()),
        _ => {
            if is_ctrl {
                if let Some(lower) = keyval.to_lower().to_unicode() {
                    if lower >= 'a' && lower <= 'z' {
                        return Some(vec![(lower as u8) - b'a' + 1]);
                    }
                }
                
                let val = keyval.to_unicode().unwrap_or('\0');
                if val == '[' { return Some(vec![27]); }
                if val == '\\' { return Some(vec![28]); }
                if val == ']' { return Some(vec![29]); }
                if val == '^' { return Some(vec![30]); }
                if val == '_' { return Some(vec![31]); }
                if val == '?' { return Some(vec![127]); }
                if val == ' ' || val == '@' { return Some(vec![0]); }
                
                return None;
            }
            if let Some(c) = keyval.to_unicode() { 
                if c >= ' ' { 
                    return Some(c.to_string().into_bytes()); 
                } 
            }
            None
        }
    }
}

pub fn add_terminal_tab(notebook: &Notebook, settings: &ConnectionSettings, override_pass: Option<String>) {
    let text_view = TextView::builder().editable(false).monospace(true).cursor_visible(true).focusable(true).can_focus(true).build();
    let provider = CssProvider::new();
    let css = format!(
        "textview, textview text {{ background-color: {0}; background: {0}; color: {1}; font-size: {2}pt; }}" ,
        settings.bg_color, settings.fg_color, settings.font_size
    );
    provider.load_from_data(&css);
    text_view.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 500);

    let scrolled = ScrolledWindow::builder().child(&text_view).vexpand(true).build();
    let label = Label::new(Some(&settings.name));
    let label_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    label_box.append(&label);
    
    let index = notebook.append_page(&scrolled, Some(&label_box));
    notebook.set_current_page(Some(index));
    text_view.grab_focus();

    let context_menu = gtk::gio::Menu::new();
    let theme_submenu = gtk::gio::Menu::new();
    let action_group = gtk::gio::SimpleActionGroup::new();
    label_box.insert_action_group("term", Some(&action_group));

    let sid_for_menu = settings.name.clone();
    
    for (theme_idx, theme) in THEMES.iter().enumerate() {
        let action_name = format!("set_theme_{}", theme_idx);
        theme_submenu.append(Some(theme.name), Some(&format!("term.{}", action_name)));

        let action_theme = gtk::gio::SimpleAction::new(&action_name, None);
        let theme_name = theme.name.to_string();
        let theme_fg = theme.fg.to_string();
        let theme_bg = theme.bg.to_string();
        let theme_pal = theme.palette.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let sid_clone = sid_for_menu.clone();

        action_theme.connect_activate(move |_, _| {
            let mut sessions = load_sessions();
            let mut updated_settings = None;
            if let Some(s) = sessions.iter_mut().find(|s| s.name == sid_clone) {
                s.theme = theme_name.clone();
                s.fg_color = theme_fg.clone();
                s.bg_color = theme_bg.clone();
                s.palette = theme_pal.clone();
                updated_settings = Some(s.clone());
            }
            if let Some(s) = updated_settings {
                save_sessions(&sessions);
                update_active_terminals(&sid_clone, &s);
            }
        });
        action_group.add_action(&action_theme);
    }
    context_menu.append_submenu(Some("Quick Set Theme"), &theme_submenu);

    let popover = gtk::PopoverMenu::from_model(Some(&context_menu));
    popover.set_parent(&label_box);
    popover.set_has_arrow(false);
    
    let gesture = gtk::GestureClick::new();
    gesture.set_button(3);
    let p_weak = popover.downgrade();
    gesture.connect_pressed(move |_, _, _, _| {
        if let Some(p) = p_weak.upgrade() {
            p.popup();
        }
    });
    label_box.add_controller(gesture);

    let (input_tx, input_rx) = flume::unbounded::<ConnectionControl>();
    let (output_tx, output_rx) = flume::unbounded::<Vec<u8>>();
    
    let term_state = Arc::new(Mutex::new(TerminalState::new(text_view.downgrade(), label.downgrade(), settings.palette.clone())));
    
    let sid = settings.name.clone();
    let tv_weak = text_view.downgrade();
    let prov_clone = provider.clone();
    let ts_clone = term_state.clone();
    ACTIVE_TERMINALS.with(|at| {
        at.borrow_mut().push(ActiveTerminal {
            session_id: sid,
            text_view: tv_weak,
            css_provider: prov_clone,
            term_state: ts_clone,
        });
    });
    let mut parser = Parser::new();

    let tv_weak = text_view.downgrade();
    let ts_weak = term_state.clone();
    let out_rx_clone = output_rx.clone();
    let itx_resize = input_tx.clone();
    let mut last_cols = 0;
    let mut last_rows = 0;
    let font_size_u32 = settings.font_size as u32;

    let ts_weak_loop = ts_weak.clone();
    glib::timeout_add_local(Duration::from_millis(10), move || {
        let tv = match tv_weak.upgrade() { Some(v) => v, None => return glib::ControlFlow::Break };
        
        let width = tv.width();
        let height = tv.height();
        if width > 0 && height > 0 {
            let char_w = (font_size_u32 as f32 * 0.6).max(1.0);
            let char_h = (font_size_u32 as f32 * 1.5).max(1.0);
            let cols = (width as f32 / char_w).max(1.0) as u32;
            let rows = (height as f32 / char_h).max(1.0) as u32;
            if cols != last_cols || rows != last_rows {
                last_cols = cols;
                last_rows = rows;
                let _ = itx_resize.send(ConnectionControl::Resize(cols, rows, width as u32, height as u32));
            }
        }

        let mut updated = false;
        while let Ok(bytes) = out_rx_clone.try_recv() {
            let mut state = ts_weak_loop.lock().unwrap();
            parser.advance(&mut *state, &bytes);
            updated = true;
        }
        if updated {
            if let Some(adj) = tv.vadjustment() { adj.set_value(adj.upper() - adj.page_size()); }
        }
        glib::ControlFlow::Continue
    });

    let itx = input_tx.clone();
    let tv_for_key = text_view.clone();
    let key_controller = EventControllerKey::new();
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let s_arc_key = ts_weak.clone();
    let original_font_size = font_size_u32;
    let sid_for_key = sid_for_menu.clone();
    key_controller.connect_key_pressed(move |_controller, keyval, _keycode, state| {
        let is_ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
        let is_shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);
        let is_cmd = state.contains(gtk::gdk::ModifierType::META_MASK) || state.contains(gtk::gdk::ModifierType::SUPER_MASK);

        let is_copy = (is_ctrl && is_shift && (keyval == gtk::gdk::Key::c || keyval == gtk::gdk::Key::C)) ||
                      (is_cmd && (keyval == gtk::gdk::Key::c || keyval == gtk::gdk::Key::C));
        let is_paste = (is_ctrl && is_shift && (keyval == gtk::gdk::Key::v || keyval == gtk::gdk::Key::V)) ||
                       (is_cmd && (keyval == gtk::gdk::Key::v || keyval == gtk::gdk::Key::V));

        let is_zoom_in = (is_ctrl || is_cmd) && (keyval == gtk::gdk::Key::equal || keyval == gtk::gdk::Key::plus || keyval == gtk::gdk::Key::KP_Add);
        let is_zoom_out = (is_ctrl || is_cmd) && (keyval == gtk::gdk::Key::minus || keyval == gtk::gdk::Key::KP_Subtract);
        let is_zoom_reset = (is_ctrl || is_cmd) && (keyval == gtk::gdk::Key::_0 || keyval == gtk::gdk::Key::KP_0);
        let is_clear = (is_ctrl || is_cmd) && (keyval == gtk::gdk::Key::k || keyval == gtk::gdk::Key::K);

        if is_copy {
            let clipboard = tv_for_key.clipboard();
            if let Some((start, end)) = tv_for_key.buffer().selection_bounds() {
                let text = tv_for_key.buffer().text(&start, &end, false);
                clipboard.set_text(&text);
            }
            return glib::Propagation::Stop;
        } else if is_paste {
            let clipboard = tv_for_key.clipboard();
            let itx_clone = itx.clone();
            let ts_paste = s_arc_key.clone();
            clipboard.read_text_async(None::<&gtk::gio::Cancellable>, move |result| {
                if let Ok(Some(text)) = result {
                    let is_bracketed = ts_paste.lock().unwrap().bracketed_paste_mode;
                    let mut data = Vec::new();
                    if is_bracketed { data.extend_from_slice(b"\x1b[200~"); }
                    data.extend_from_slice(text.as_bytes());
                    if is_bracketed { data.extend_from_slice(b"\x1b[201~"); }
                    
                    let _ = itx_clone.send(ConnectionControl::Input(data));
                }
            });
            return glib::Propagation::Stop;
        } else if is_clear {
            let mut state = s_arc_key.lock().unwrap();
            let mut p_end = state.primary_buffer.end_iter();
            let mut p_start = state.primary_buffer.start_iter();
            state.primary_buffer.delete(&mut p_start, &mut p_end);
            
            let mut a_end = state.alternate_buffer.end_iter();
            let mut a_start = state.alternate_buffer.start_iter();
            state.alternate_buffer.delete(&mut a_start, &mut a_end);
            
            state.cursor_x = 0; state.cursor_y = 0;
            state.alt_cursor_x = 0; state.alt_cursor_y = 0;
            return glib::Propagation::Stop;
        } else if is_zoom_in || is_zoom_out || is_zoom_reset {
            let sid_to_update = sid_for_key.clone();
            
            ACTIVE_TERMINALS.with(|at| {
                let mut list = at.borrow_mut();
                if let Some(term) = list.iter_mut().find(|t| t.session_id == sid_to_update) {
                    let mut sessions = load_sessions();
                    if let Some(s) = sessions.iter_mut().find(|s| s.name == sid_to_update) {
                        let new_size = if is_zoom_reset {
                            original_font_size as i32
                        } else if is_zoom_in {
                            s.font_size + 1
                        } else {
                            (s.font_size - 1).clone().max(6)
                        };
                        
                        s.font_size = new_size;
                        
                        let css = format!(
                            "textview, textview text {{ background-color: {0}; background: {0}; color: {1}; font-size: {2}pt; }}",
                            s.bg_color, s.fg_color, new_size
                        );
                        term.css_provider.load_from_data(&css);
                        
                        save_sessions(&sessions);
                    }
                }
            });
            return glib::Propagation::Stop;
        }

        if let Some(data) = keyval_to_bytes(keyval, state) {
            let _ = itx.send(ConnectionControl::Input(data));
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    text_view.add_controller(key_controller);

    let scroll_controller = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let s_arc_scroll = ts_weak.clone();
    let itx_scroll = input_tx.clone();
    scroll_controller.connect_scroll(move |_controller, _dx, dy| {
        let ts = s_arc_scroll.lock().unwrap();
        if ts.mouse_tracking_mode > 0 {
            // SGR format: ESC [ < Pcb ; Px ; Py (M for press, m for release)
            // Simplified scroll: we don't know precise X/Y here easily, so we just use 1;1
            // Button 4 (scroll up) is usually code 64, Button 5 (scroll down) is 65.
            let button = if dy < 0.0 { 64 } else { 65 };
            let sgr = format!("\x1b[<{};1;1M", button);
            let _ = itx_scroll.send(ConnectionControl::Input(sgr.into_bytes()));
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    text_view.add_controller(scroll_controller);

    let click_controller = gtk::GestureClick::new();
    click_controller.set_button(0); // All buttons
    let s_arc_click = ts_weak.clone();
    let itx_click = input_tx.clone();
    let fs_click = font_size_u32;
    
    click_controller.connect_pressed(move |gesture, _n_press, x, y| {
        let ts = s_arc_click.lock().unwrap();
        if ts.mouse_tracking_mode > 0 {
            let button = match gesture.current_button() {
                1 => 0, // Left
                2 => 1, // Middle
                3 => 2, // Right
                _ => 0,
            };
            let char_w = (fs_click as f32 * 0.6).max(1.0);
            let char_h = (fs_click as f32 * 1.5).max(1.0);
            let col = (x as f32 / char_w).max(0.0) as u32 + 1;
            let row = (y as f32 / char_h).max(0.0) as u32 + 1;
            
            let sgr = format!("\x1b[<{};{};{}M", button, col, row);
            let _ = itx_click.send(ConnectionControl::Input(sgr.into_bytes()));
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    
    let s_arc_release = ts_weak.clone();
    let itx_release = input_tx.clone();
    let fs_release = font_size_u32;
    click_controller.connect_released(move |gesture, _n_press, x, y| {
        let ts = s_arc_release.lock().unwrap();
        if ts.mouse_tracking_mode > 0 {
            let button = match gesture.current_button() {
                1 => 0,
                2 => 1,
                3 => 2,
                _ => 0,
            };
            let char_w = (fs_release as f32 * 0.6).max(1.0);
            let char_h = (fs_release as f32 * 1.5).max(1.0);
            let col = (x as f32 / char_w).max(0.0) as u32 + 1;
            let row = (y as f32 / char_h).max(0.0) as u32 + 1;
            
            let sgr = format!("\x1b[<{};{};{}m", button, col, row); // 'm' for release in 1006
            let _ = itx_release.send(ConnectionControl::Input(sgr.into_bytes()));
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    text_view.add_controller(click_controller);

    let s_clone = settings.clone();
    let final_pass = override_pass.or(settings.password.clone());
    text_view.buffer().set_text(&format!("Connecting to {}...\n", settings.name));

    std::thread::spawn(move || {
        match connect_ssh(&s_clone.host, s_clone.port, &s_clone.username, final_pass.as_deref().unwrap_or(""), s_clone.private_key.as_deref(), s_clone.keepalive, s_clone.agent_forwarding, &s_clone.term_type) {
            Ok((session, mut channel)) => {
                let _ = output_tx.send(b"Connection established.\r\n".to_vec());

                // PARSE LOCAL FORWARDS
                let mut local_listeners = Vec::new();
                for forward in s_clone.local_forwards.split(',') {
                    let parts: Vec<&str> = forward.trim().split(':').collect();
                    if parts.len() == 3 {
                        if let (Ok(l_port), Ok(r_port)) = (parts[0].parse::<u16>(), parts[2].parse::<u16>()) {
                            let r_host = parts[1].to_string();
                            if let Ok(listener) = std::net::TcpListener::bind(format!("127.0.0.1:{}", l_port)) {
                                let _ = listener.set_nonblocking(true);
                                local_listeners.push((listener, r_host, r_port));
                                let _ = output_tx.send(format!("-L {}:{}:{} forwarded.\r\n", l_port, parts[1], r_port).into_bytes());
                            }
                        }
                    }
                }

                // PARSE REMOTE FORWARDS
                let mut remote_listeners = Vec::new();
                for forward in s_clone.remote_forwards.split(',') {
                    let parts: Vec<&str> = forward.trim().split(':').collect();
                    if parts.len() == 3 {
                        if let (Ok(r_port), Ok(l_port)) = (parts[0].parse::<u16>(), parts[2].parse::<u16>()) {
                            let l_host = parts[1].to_string();
                            // Attempt to use None for host. The API usually expects `port, host_option, bound_port, backlog` or similar
                            // We will use standard u16 defaults. The compiler will guide us if wrong:
                            // remote_port_forward(port: u16, host: Option<&str>, bound_port: u16, max_connections: Option<u32>)
                            match session.channel_forward_listen(r_port, Some("0.0.0.0"), None) {
                                Ok((listener, _bound_port)) => {
                                    remote_listeners.push((listener, l_host, l_port));
                                    let _ = output_tx.send(format!("-R {}:{}:{} forwarded.\r\n", r_port, parts[1], l_port).into_bytes());
                                }
                                Err(e) => {
                                    let _ = output_tx.send(format!("-R proxy failed to bind remote port {}: {}\r\n", r_port, e).into_bytes());
                                }
                            }
                        }
                    }
                }

                let _ = session.set_blocking(false);
                let mut buffer = [0; 8192];
                
                // Active TCP proxies
                let mut active_local_tunnels: Vec<(std::net::TcpStream, ssh2::Channel)> = Vec::new();
                let mut active_remote_tunnels: Vec<(std::net::TcpStream, ssh2::Channel)> = Vec::new();
                
                loop {
                    // Check main terminal channel output
                    match channel.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(size) => { let _ = output_tx.send(buffer[..size].to_vec()); }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // Check Terminal Input
                            while let Ok(ctrl) = input_rx.try_recv() {
                                match ctrl {
                                    ConnectionControl::Input(data) => {
                                        let mut pos = 0;
                                        while pos < data.len() {
                                            match channel.write(&data[pos..]) {
                                                Ok(written) => pos += written,
                                                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                                                Err(_) => break,
                                            }
                                        }
                                    },
                                    ConnectionControl::Resize(cols, rows, width_px, height_px) => {
                                        let _ = channel.request_pty_size(cols, rows, Some(width_px), Some(height_px));
                                    }
                                }
                                let _ = channel.flush();
                            }
                            
                            // Check new local connections (-L)
                            for (listener, r_host, r_port) in &local_listeners {
                                if let Ok((tcp_stream, _addr)) = listener.accept() {
                                    let _ = tcp_stream.set_nonblocking(true);
                                    // Momentarily block to establish SSH channel reliably
                                    let _ = session.set_blocking(true);
                                    if let Ok(forward_channel) = session.channel_direct_tcpip(r_host, *r_port, None) {
                                        active_local_tunnels.push((tcp_stream, forward_channel));
                                    }
                                    let _ = session.set_blocking(false);
                                }
                            }
                            
                            // Check new remote connections (-R)
                            for (listener, l_host, l_port) in &mut remote_listeners {
                                match listener.accept() {
                                    Ok(forward_channel) => {
                                        if let Ok(tcp_stream) = std::net::TcpStream::connect(format!("{}:{}", l_host, l_port)) {
                                            let _ = tcp_stream.set_nonblocking(true);
                                            active_remote_tunnels.push((tcp_stream, forward_channel));
                                        }
                                    }
                                    Err(_) => {}
                                }
                            }
                            
                            // Pipe local tunnels (TCP -> SSH) and (SSH -> TCP)
                            let mut drop_local_idx = Vec::new();
                            for (idx, (tcp, ch)) in active_local_tunnels.iter_mut().enumerate() {
                                // TCP -> SSH
                                match tcp.read(&mut buffer) {
                                    Ok(0) => drop_local_idx.push(idx),
                                    Ok(size) => { let _ = ch.write_all(&buffer[..size]); }
                                    Err(_) => ()
                                }
                                // SSH -> TCP
                                match ch.read(&mut buffer) {
                                    Ok(0) => if !drop_local_idx.contains(&idx) { drop_local_idx.push(idx) },
                                    Ok(size) => { let _ = tcp.write_all(&buffer[..size]); }
                                    Err(_) => ()
                                }
                            }
                            for idx in drop_local_idx.into_iter().rev() { active_local_tunnels.remove(idx); }
                            
                            // Pipe remote tunnels (SSH -> TCP) and (TCP -> SSH)
                            let mut drop_remote_idx = Vec::new();
                            for (idx, (tcp, ch)) in active_remote_tunnels.iter_mut().enumerate() {
                                // SSH -> TCP
                                match ch.read(&mut buffer) {
                                    Ok(0) => drop_remote_idx.push(idx),
                                    Ok(size) => { let _ = tcp.write_all(&buffer[..size]); }
                                    Err(_) => ()
                                }
                                // TCP -> SSH
                                match tcp.read(&mut buffer) {
                                    Ok(0) => if !drop_remote_idx.contains(&idx) { drop_remote_idx.push(idx) },
                                    Ok(size) => { let _ = ch.write_all(&buffer[..size]); }
                                    Err(_) => ()
                                }
                            }
                            for idx in drop_remote_idx.into_iter().rev() { active_remote_tunnels.remove(idx); }

                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
                let _ = output_tx.send(b"\r\n[Connection closed]\r\n".to_vec());
            }
            Err(e) => {
                let _ = output_tx.send(format!("Connection failed: {}\r\n", e).as_bytes().to_vec());
            }
        }
    });
}
