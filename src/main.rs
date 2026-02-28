use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{
    glib, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, ListBox, Orientation,
    ScrolledWindow, TextView, CssProvider, EventControllerKey, TextBuffer, TextTag, Notebook,
};
use serde::{Deserialize, Serialize};
use ssh2::Session as SshSession;
use std::net::TcpStream;
use std::io::{Read, Write};
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use vte::{Parser, Perform};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ConnectionSettings {
    name: String,
    host: String,
    port: u16,
    username: String,
}

fn get_config_path() -> PathBuf {
    let mut path = dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".terminal_ssh_sessions.json");
    path
}

fn load_sessions() -> Vec<ConnectionSettings> {
    let path = get_config_path();
    if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn save_sessions(sessions: &[ConnectionSettings]) {
    let path = get_config_path();
    if let Ok(content) = serde_json::to_string_pretty(sessions) {
        let _ = fs::write(path, content);
    }
}

struct TerminalState {
    buffer: TextBuffer,
    current_tags: Vec<String>,
}

impl TerminalState {
    fn new(buffer: TextBuffer) -> Self {
        let tag_table = buffer.tag_table();
        let colors = [
            ("30", "#2e3436"), ("31", "#cc0000"), ("32", "#4e9a06"), ("33", "#c4a000"),
            ("34", "#3465a4"), ("35", "#75507b"), ("36", "#06989a"), ("37", "#d3d7cf"),
            ("90", "#555753"), ("91", "#ef2929"), ("92", "#8ae234"), ("93", "#fce94f"),
            ("94", "#729fcf"), ("95", "#ad7fa8"), ("96", "#34e2e2"), ("97", "#eeeeec"),
        ];
        for (code, color) in colors {
            let tag = TextTag::new(Some(&format!("fg-{}", code)));
            tag.set_foreground(Some(color));
            tag_table.add(&tag);
        }
        let bold_tag = TextTag::new(Some("bold"));
        bold_tag.set_weight(700);
        tag_table.add(&bold_tag);
        Self { buffer, current_tags: Vec::new() }
    }

    fn apply_sgr(&mut self, params: &[i64]) {
        if params.is_empty() || params[0] == 0 {
            self.current_tags.clear();
            return;
        }
        for &param in params {
            match param {
                0 => self.current_tags.clear(),
                1 => if !self.current_tags.contains(&"bold".to_string()) { self.current_tags.push("bold".to_string()); },
                30..=37 | 90..=97 => {
                    self.current_tags.retain(|t| !t.starts_with("fg-"));
                    self.current_tags.push(format!("fg-{}", param));
                }
                _ => {}
            }
        }
    }
}

impl Perform for TerminalState {
    fn print(&mut self, c: char) {
        let mut iter = self.buffer.end_iter();
        let start_offset = iter.offset();
        self.buffer.insert(&mut iter, &c.to_string());
        if !self.current_tags.is_empty() {
            let start_iter = self.buffer.iter_at_offset(start_offset);
            let end_iter = self.buffer.end_iter();
            for tag_name in &self.current_tags {
                if let Some(tag) = self.buffer.tag_table().lookup(tag_name) {
                    self.buffer.apply_tag(&tag, &start_iter, &end_iter);
                }
            }
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => { 
                let mut iter = self.buffer.end_iter(); 
                self.buffer.insert(&mut iter, "\n"); 
            }
            b'\r' => {}
            b'\x08' | b'\x7f' => {
                let mut iter = self.buffer.end_iter();
                if iter.backward_char() {
                    let mut end = self.buffer.end_iter();
                    self.buffer.delete(&mut iter, &mut end);
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &vte::Params, _intermediates: &[u8], _ignore: bool, c: char) {
        if c == 'm' {
            let p: Vec<i64> = params.iter().map(|it| it[0] as i64).collect();
            self.apply_sgr(&p);
        }
    }
}

fn main() {
    let app = Application::builder().application_id("org.terminal.ssh").build();
    app.connect_startup(|_| {
        let provider = CssProvider::new();
        provider.load_from_data("
            window { background-color: #1a1a1a; color: #ffffff; }
            .connection-box { padding: 40px; background-color: #1a1a1a; }
            entry { 
                border-radius: 8px; 
                padding: 10px; 
                background-color: #2d2d2d; 
                color: #ffffff; 
                caret-color: #ffffff;
                border: 1px solid #3d3d3d; 
                margin-bottom: 12px; 
            }
            entry:focus { border-color: #3d5afe; }
            button.suggested-action { background-color: #3d5afe; color: white; border-radius: 8px; padding: 14px; font-weight: bold; margin-top: 10px; }
            button.secondary-action { background-color: #424242; color: white; border-radius: 8px; padding: 10px; margin-top: 10px; }
            button.destructive-action { background-color: transparent; padding: 4px; border-radius: 4px; color: #ffffff; }
            button.destructive-action:hover { background-color: #e53935; }
            textview { background-color: #000000; color: #0dcf21; font-family: 'Monospace', monospace; font-size: 14px; padding: 10px; }
            listbox { background-color: #242424; border-radius: 8px; border: 1px solid #3d3d3d; margin-top: 10px; }
            .session-row { padding: 8px 12px; border-bottom: 1px solid #333; }
            label.title { font-size: 32px; font-weight: bold; margin-bottom: 40px; color: #3d5afe; }
            notebook { background: #1a1a1a; }
            notebook header tabs { background: #1a1a1a; }
            notebook header tabs tab { 
                padding: 8px 16px; 
                border-right: 1px solid #333333; 
                background-color: #2d2d2d; 
                color: #aaaaaa; 
            }
            notebook header tabs tab label { color: #aaaaaa; }
            notebook header tabs tab:hover { background-color: #3d3d3d; color: #ffffff; }
            notebook header tabs tab:hover label { color: #ffffff; }
            notebook header tabs tab:checked { background-color: #3d5afe; color: white; }
            notebook header tabs tab:checked label { color: white; }
            notebook stack { background: #1a1a1a; }
        ");
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("Could not connect to a display."),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });
    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder().application(app).title("Terminal SSH").default_width(1000).default_height(750).build();
    let notebook = Notebook::builder().tab_pos(gtk::PositionType::Top).build();
    window.set_child(Some(&notebook));

    let connection_box = GtkBox::new(Orientation::Vertical, 0);
    connection_box.add_css_class("connection-box");
    let title = Label::builder().label("Terminal SSH").css_classes(["title"]).halign(gtk::Align::Center).build();
    connection_box.append(&title);

    let host_entry = Entry::builder().placeholder_text("Hostname or IP").build();
    connection_box.append(&host_entry);

    let row1 = GtkBox::new(Orientation::Horizontal, 12);
    let port_entry = Entry::builder().text("22").width_chars(6).build();
    row1.append(&port_entry);
    let user_entry = Entry::builder().placeholder_text("Username").hexpand(true).build();
    row1.append(&user_entry);
    connection_box.append(&row1);

    let pass_entry = Entry::builder().placeholder_text("Password").visibility(false).build();
    connection_box.append(&pass_entry);

    let btn_box = GtkBox::new(Orientation::Horizontal, 12);
    let connect_btn = Button::builder().label("Connect").css_classes(["suggested-action"]).hexpand(true).build();
    btn_box.append(&connect_btn);
    let save_btn = Button::builder().label("Save").css_classes(["secondary-action"]).build();
    btn_box.append(&save_btn);
    connection_box.append(&btn_box);

    let sessions_list = ListBox::new();
    let scrolled = ScrolledWindow::builder().min_content_height(250).child(&sessions_list).vexpand(true).build();
    connection_box.append(&scrolled);

    notebook.append_page(&connection_box, Some(&Label::new(Some("Connect"))));

    let sessions = Arc::new(Mutex::new(load_sessions()));
    let s_init = sessions.lock().unwrap();
    populate_list(&sessions_list, &s_init, &host_entry, &port_entry, &user_entry, sessions.clone());
    drop(s_init);

    let sessions_weak = Arc::downgrade(&sessions);
    let h_e_weak = host_entry.downgrade();
    let p_e_weak = port_entry.downgrade();
    let u_e_weak = user_entry.downgrade();
    let list_weak = sessions_list.downgrade();
    let sess_clone_for_save = sessions.clone();
    save_btn.connect_clicked(move |_| {
        let sessions_up = match sessions_weak.upgrade() { Some(v) => v, None => return };
        let host_e = match h_e_weak.upgrade() { Some(v) => v, None => return };
        let port_e = match p_e_weak.upgrade() { Some(v) => v, None => return };
        let user_e = match u_e_weak.upgrade() { Some(v) => v, None => return };
        let list = match list_weak.upgrade() { Some(v) => v, None => return };
        let host = host_e.text().to_string();
        let port = port_e.text().parse::<u16>().unwrap_or(22);
        let user = user_e.text().to_string();
        if !host.is_empty() && !user.is_empty() {
            let mut s = sessions_up.lock().unwrap();
            if !s.iter().any(|x| x.host == host && x.username == user) {
                s.push(ConnectionSettings { name: format!("{}@{}", user, host), host, port, username: user });
                save_sessions(&s);
                populate_list(&list, &s, &host_e, &port_e, &user_e, sess_clone_for_save.clone());
            }
        }
    });

    let notebook_weak_conn = notebook.downgrade();
    let h_e_weak = host_entry.downgrade();
    let p_e_weak = port_entry.downgrade();
    let u_e_weak = user_entry.downgrade();
    let pass_e_weak = pass_entry.downgrade();
    connect_btn.connect_clicked(move |_| {
        let notebook_up = match notebook_weak_conn.upgrade() { Some(v) => v, None => return };
        let host_e = match h_e_weak.upgrade() { Some(v) => v, None => return };
        let port_e = match p_e_weak.upgrade() { Some(v) => v, None => return };
        let user_e = match u_e_weak.upgrade() { Some(v) => v, None => return };
        let pass_e = match pass_e_weak.upgrade() { Some(v) => v, None => return };
        let host = host_e.text().to_string();
        let port = port_e.text().parse::<u16>().unwrap_or(22);
        let user = user_e.text().to_string();
        let pass = pass_e.text().to_string();
        if host.is_empty() || user.is_empty() { return; }
        
        create_terminal_tab(&notebook_up, &host, port, &user, &pass);
    });

    window.present();
}

fn create_terminal_tab(notebook: &Notebook, host: &str, port: u16, user: &str, pass: &str) {
    let terminal_box = GtkBox::new(Orientation::Vertical, 0);
    let text_view = TextView::builder().editable(false).monospace(true).cursor_visible(true).focusable(true).can_focus(true).build();
    let term_scrolled = ScrolledWindow::builder().child(&text_view).vexpand(true).build();
    terminal_box.append(&term_scrolled);

    let label_box = GtkBox::new(Orientation::Horizontal, 6);
    let label = Label::new(Some(&format!("{}@{}", user, host)));
    label_box.append(&label);
    let close_btn = Button::builder().icon_name("window-close-symbolic").css_classes(["destructive-action"]).build();
    label_box.append(&close_btn);

    let page_num = notebook.append_page(&terminal_box, Some(&label_box));
    notebook.set_current_page(Some(page_num));
    text_view.grab_focus();

    let (input_tx, input_rx) = flume::unbounded::<Vec<u8>>();
    let (output_tx, output_rx) = flume::unbounded::<Vec<u8>>();
    let term_state = Arc::new(Mutex::new(TerminalState::new(text_view.buffer())));
    let mut parser = Parser::new();

    let tv_weak = text_view.downgrade();
    let ts_weak = term_state.clone();
    let out_rx_clone = output_rx.clone();
    glib::timeout_add_local(Duration::from_millis(10), move || {
        let tv = match tv_weak.upgrade() { Some(v) => v, None => return glib::ControlFlow::Break };
        let mut updated = false;
        while let Ok(bytes) = out_rx_clone.try_recv() {
            let mut state = ts_weak.lock().unwrap();
            parser.advance(&mut *state, &bytes);
            updated = true;
        }
        if updated {
            if let Some(adj) = tv.vadjustment() { adj.set_value(adj.upper() - adj.page_size()); }
        }
        glib::ControlFlow::Continue
    });

    let itx = input_tx.clone();
    let key_controller = EventControllerKey::new();
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    key_controller.connect_key_pressed(move |_controller, keyval, _keycode, state| {
        if let Some(data) = keyval_to_bytes(keyval, state) {
            let _ = itx.send(data);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    text_view.add_controller(key_controller);

    let nb_weak = notebook.downgrade();
    let tb_weak = terminal_box.downgrade();
    close_btn.connect_clicked(move |_| {
        if let (Some(nb), Some(tb)) = (nb_weak.upgrade(), tb_weak.upgrade()) {
            if let Some(pos) = nb.page_num(&tb) {
                nb.remove_page(Some(pos));
            }
        }
    });

    let host_s = host.to_string();
    let user_s = user.to_string();
    let pass_s = pass.to_string();
    
    // Set initial text
    text_view.buffer().set_text(&format!("Connecting to {}@{}...\n", user, host));

    std::thread::spawn(move || {
        match connect_ssh(&host_s, port, &user_s, &pass_s) {
            Ok((session, mut channel)) => {
                let _ = output_tx.send(b"Connection established.\r\n".to_vec());
                let _ = session.set_blocking(false);
                let mut buffer = [0; 8192];
                loop {
                    match channel.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(size) => { let _ = output_tx.send(buffer[..size].to_vec()); }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            while let Ok(data) = input_rx.try_recv() {
                                let mut pos = 0;
                                while pos < data.len() {
                                    match channel.write(&data[pos..]) {
                                        Ok(written) => pos += written,
                                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                            std::thread::sleep(Duration::from_millis(10));
                                            continue;
                                        }
                                        Err(_) => break,
                                    }
                                }
                                let _ = channel.flush();
                            }
                            std::thread::sleep(Duration::from_millis(10));
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

fn populate_list(list: &ListBox, sessions: &[ConnectionSettings], host_e: &Entry, port_e: &Entry, user_e: &Entry, sessions_arc: Arc<Mutex<Vec<ConnectionSettings>>>) {
    while let Some(child) = list.first_child() { list.remove(&child); }
    for (index, s) in sessions.iter().enumerate() {
        let row_box = GtkBox::new(Orientation::Horizontal, 10);
        row_box.add_css_class("session-row");
        let label = Label::builder().label(&s.name).halign(gtk::Align::Start).hexpand(true).build();
        row_box.append(&label);
        let delete_btn = Button::builder().icon_name("user-trash-symbolic").css_classes(["destructive-action"]).build();
        row_box.append(&delete_btn);
        let row = gtk::ListBoxRow::builder().child(&row_box).build();
        list.append(&row);
        let s_arc_clone = sessions_arc.clone();
        let list_weak = list.downgrade();
        let h_e_weak = host_e.downgrade();
        let p_e_weak = port_e.downgrade();
        let u_e_weak = user_e.downgrade();
        delete_btn.connect_clicked(move |_| {
            let list_up = match list_weak.upgrade() { Some(v) => v, None => return };
            let h_e = match h_e_weak.upgrade() { Some(v) => v, None => return };
            let p_e = match p_e_weak.upgrade() { Some(v) => v, None => return };
            let u_e = match u_e_weak.upgrade() { Some(v) => v, None => return };
            let mut s = s_arc_clone.lock().unwrap();
            if index < s.len() {
                s.remove(index);
                save_sessions(&s);
                populate_list(&list_up, &s, &h_e, &p_e, &u_e, s_arc_clone.clone());
            }
        });
    }
    let sessions_vec = sessions.to_vec();
    let h_e_weak = host_e.downgrade();
    let p_e_weak = port_e.downgrade();
    let u_e_weak = user_e.downgrade();
    list.connect_row_activated(move |_, row| {
        let h_e = match h_e_weak.upgrade() { Some(v) => v, None => return };
        let p_e = match p_e_weak.upgrade() { Some(v) => v, None => return };
        let u_e = match u_e_weak.upgrade() { Some(v) => v, None => return };
        if let Some(s) = sessions_vec.get(row.index() as usize) {
            h_e.set_text(&s.host); p_e.set_text(&s.port.to_string()); u_e.set_text(&s.username);
        }
    });
}

fn keyval_to_bytes(keyval: gdk::Key, state: gdk::ModifierType) -> Option<Vec<u8>> {
    use gdk::Key;
    let is_ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
    match keyval {
        Key::Return | Key::KP_Enter => Some(b"\r".to_vec()),
        Key::BackSpace => Some(b"\x7f".to_vec()),
        Key::Tab => Some(b"\t".to_vec()),
        Key::Escape => Some(b"\x1b".to_vec()),
        Key::Left => Some(b"\x1b[D".to_vec()),
        Key::Right => Some(b"\x1b[C".to_vec()),
        Key::Up => Some(b"\x1b[A".to_vec()),
        Key::Down => Some(b"\x1b[B".to_vec()),
        _ => {
            if is_ctrl {
                let val = keyval.to_unicode().unwrap_or('\0');
                if val >= 'a' && val <= 'z' { return Some(vec![(val as u8) - b'a' + 1]); }
                if val >= 'A' && val <= 'Z' { return Some(vec![(val as u8) - b'A' + 1]); }
            }
            if let Some(c) = keyval.to_unicode() { if c >= ' ' { return Some(c.to_string().into_bytes()); } }
            None
        }
    }
}

use gtk::gdk;

fn connect_ssh(host: &str, port: u16, user: &str, pass: &str) -> Result<(ssh2::Session, ssh2::Channel), Box<dyn std::error::Error + Send + Sync>> {
    let tcp = TcpStream::connect(format!("{}:{}", host, port))?;
    let mut sess = SshSession::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;
    sess.userauth_password(user, pass)?;
    let mut channel = sess.channel_session()?;
    channel.request_pty("xterm-256color", None, Some((80, 24, 0, 0)))?;
    channel.shell()?;
    Ok((sess, channel))
}
