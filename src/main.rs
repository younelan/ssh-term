use gtk4 as gtk;
use gtk::gio;
use gtk::prelude::*;
use gtk::{
    glib, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, ListBox, Orientation,
    ScrolledWindow, TextView, CssProvider, EventControllerKey, TextBuffer, TextTag, HeaderBar, 
    ColorButton, DropDown, StringList, Notebook, MenuButton, CheckButton, Grid,
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

thread_local! {
    static CONN_WIN: std::cell::RefCell<Option<ApplicationWindow>> = std::cell::RefCell::new(None);
    static TARGET_NB: std::cell::RefCell<glib::object::WeakRef<Notebook>> = std::cell::RefCell::new(glib::object::WeakRef::new());
    static ACTIVE_TERMINALS: std::cell::RefCell<Vec<ActiveTerminal>> = std::cell::RefCell::new(Vec::new());
    static IS_PROGRAMMATIC: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

struct ActiveTerminal {
    session_id: String,
    text_view: glib::object::WeakRef<TextView>,
    css_provider: CssProvider,
    term_state: Arc<Mutex<TerminalState>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ConnectionSettings {
    name: String,
    host: String,
    port: u16,
    username: String,
    #[serde(default)]
    password: Option<String>,
    #[serde(default = "default_fg")]
    fg_color: String,
    #[serde(default = "default_bg")]
    bg_color: String,
    #[serde(default = "default_font_size")]
    font_size: i32,
    #[serde(default = "default_palette")]
    palette: Vec<String>,
    #[serde(default = "default_cursor_style")]
    cursor_style: String,
    #[serde(default = "default_cursor_blink")]
    cursor_blink: bool,
    #[serde(default = "default_scrollback")]
    scrollback: i32,
    #[serde(default)]
    private_key: Option<String>,
    #[serde(default = "default_keepalive")]
    keepalive: u32,
    #[serde(default)]
    pub agent_forwarding: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_method")]
    pub method: u32, // 0: Password, 1: Key
}

fn default_keepalive() -> u32 { 0 }
fn default_theme() -> String { "Default".to_string() }
fn default_method() -> u32 { 0 }

struct Theme {
    name: &'static str,
    fg: &'static str,
    bg: &'static str,
    palette: [&'static str; 16],
}

const THEMES: [Theme; 6] = [
    Theme {
        name: "Basic",
        fg: "#ffffff",
        bg: "#000000",
        palette: [
            "#000000", "#cc0000", "#4e9a06", "#c4a000", "#3465a4", "#75507b", "#06989a", "#d3d7cf",
            "#555753", "#ef2929", "#8ae234", "#fce94f", "#729fcf", "#ad7fa8", "#34e2e2", "#eeeeec",
        ],
    },
    Theme {
        name: "Peppermint",
        fg: "#b3fffd",
        bg: "#050808",
        palette: [
            "#222222", "#ff3333", "#33ff33", "#ffff33", "#3333ff", "#ff33ff", "#33ffff", "#ffffff",
            "#444444", "#ff6666", "#66ff66", "#ffff66", "#6666ff", "#ff66ff", "#66ffff", "#ffffff",
        ],
    },
    Theme {
        name: "Novel",
        fg: "#3b2311",
        bg: "#dfdbc3",
        palette: [
            "#000000", "#cc0000", "#4e9a06", "#c4a000", "#3465a4", "#75507b", "#06989a", "#d3d7cf",
            "#555753", "#ef2929", "#8ae234", "#fce94f", "#729fcf", "#ad7fa8", "#34e2e2", "#eeeeec",
        ],
    },
    Theme {
        name: "Silver Aerogel",
        fg: "#000000",
        bg: "#adadad",
        palette: [
            "#000000", "#941100", "#11a200", "#7d7a00", "#0048ad", "#c800c8", "#008787", "#ffffff",
            "#474747", "#ff0000", "#00ff00", "#ffff00", "#0000ff", "#ff00ff", "#00ffff", "#ffffff",
        ],
    },
    Theme {
        name: "Homebrew",
        fg: "#2aff42",
        bg: "#000000",
        palette: [
            "#000000", "#cc0000", "#4e9a06", "#c4a000", "#3465a4", "#75507b", "#06989a", "#d3d7cf",
            "#555753", "#ef2929", "#8ae234", "#fce94f", "#729fcf", "#ad7fa8", "#34e2e2", "#eeeeec",
        ],
    },
    Theme {
        name: "Dracula",
        fg: "#f8f8f2",
        bg: "#282a36",
        palette: [
            "#21222c", "#ff5555", "#50fa7b", "#f1fa8c", "#bd93f9", "#ff79c6", "#8be9fd", "#f8f8f2",
            "#6272a4", "#ff6e6e", "#69ff94", "#ffffa5", "#d6acff", "#ff92df", "#a4ffff", "#ffffff",
        ],
    },
];

fn default_fg() -> String { "#00ff00".to_string() }
fn default_bg() -> String { "#000000".to_string() }
fn default_font_size() -> i32 { 14 }
fn default_cursor_style() -> String { "Block".to_string() }
fn default_cursor_blink() -> bool { true }
fn default_scrollback() -> i32 { 1000 }
fn default_palette() -> Vec<String> {
    vec![
        "#2e3436".to_string(), "#cc0000".to_string(), "#4e9a06".to_string(), "#c4a000".to_string(),
        "#3465a4".to_string(), "#75507b".to_string(), "#06989a".to_string(), "#d3d7cf".to_string(),
        "#555753".to_string(), "#ef2929".to_string(), "#8ae234".to_string(), "#fce94f".to_string(),
        "#729fcf".to_string(), "#ad7fa8".to_string(), "#34e2e2".to_string(), "#eeeeec".to_string(),
    ]
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
    primary_buffer: TextBuffer,
    alternate_buffer: TextBuffer,
    is_alternate: bool,
    current_tags: Vec<String>,
    cursor_x: usize,
    cursor_y: usize,
    alt_cursor_x: usize,
    alt_cursor_y: usize,
    mouse_tracking_mode: u32, // 0 = off, 1000 = normal tracking, 1002 = button-event tracking, 1006 = SGR coordinates
    view: glib::WeakRef<TextView>,
}

impl TerminalState {
    fn new(view: glib::WeakRef<TextView>, palette: Vec<String>) -> Self {
        let primary_buffer = view.upgrade().unwrap().buffer();
        let tag_table = primary_buffer.tag_table();
        let alternate_buffer = TextBuffer::new(Some(&tag_table));

        let codes = [
            "30", "31", "32", "33", "34", "35", "36", "37",
            "90", "91", "92", "93", "94", "95", "96", "97",
        ];
        for (i, &code) in codes.iter().enumerate() {
            if let Some(color) = palette.get(i) {
                let tag = TextTag::new(Some(&format!("fg-{}", code)));
                tag.set_foreground(Some(color));
                tag_table.add(&tag);
            }
        }
        let bold_tag = TextTag::new(Some("bold"));
        bold_tag.set_weight(700);
        tag_table.add(&bold_tag);
        
        Self { 
            primary_buffer, 
            alternate_buffer, 
            is_alternate: false, 
            current_tags: Vec::new(),
            cursor_x: 0,
            cursor_y: 0,
            alt_cursor_x: 0,
            alt_cursor_y: 0,
            mouse_tracking_mode: 0,
            view,
        }
    }

    fn active_buffer(&self) -> &TextBuffer {
        if self.is_alternate { &self.alternate_buffer } else { &self.primary_buffer }
    }

    fn ensure_cursor_position(&self, cx: usize, cy: usize) -> gtk::TextIter {
        let buffer = self.active_buffer();
        let line_count = buffer.line_count() as usize;
        if cy >= line_count {
            let mut end = buffer.end_iter();
            let newlines = "\n".repeat((cy + 1).saturating_sub(line_count));
            buffer.insert(&mut end, &newlines);
        }
        
        if let Some(mut iter) = buffer.iter_at_line(cy as i32) {
            iter.set_line_index(cx as i32);
            if !iter.ends_line() { iter.forward_to_line_end(); }
            let current_idx = iter.line_index() as usize;
            if cx > current_idx {
                let spaces = " ".repeat(cx - current_idx);
                buffer.insert(&mut iter, &spaces);
            }
        }
        buffer.iter_at_line_index(cy as i32, cx as i32).unwrap_or_else(|| buffer.end_iter())
    }

    fn update_palette(&mut self, palette: &[String]) {
        let tag_table = self.active_buffer().tag_table();
        let codes = [
            "30", "31", "32", "33", "34", "35", "36", "37",
            "90", "91", "92", "93", "94", "95", "96", "97",
        ];
        for (i, &code) in codes.iter().enumerate() {
            if let Some(color) = palette.get(i) {
                let tag_name = format!("fg-{}", code);
                if let Some(tag) = tag_table.lookup(&tag_name) {
                    tag.set_foreground(Some(color));
                }
            }
        }
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
        let cx;
        let cy;
        {
            cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
            cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
        }

        let mut iter = self.ensure_cursor_position(cx, cy);
        let buffer = self.active_buffer();
        
        if !iter.ends_line() {
            let mut next = iter.clone();
            next.forward_char();
            buffer.delete(&mut iter, &mut next);
        }
        
        let start_offset = iter.offset();
        buffer.insert(&mut iter, &c.to_string());
        
        if self.is_alternate {
            self.alt_cursor_x += 1;
        } else {
            self.cursor_x += 1;
        }
        
        let buffer = self.active_buffer();
        if !self.current_tags.is_empty() {
            let start_iter = buffer.iter_at_offset(start_offset);
            let end_iter = buffer.iter_at_offset(start_offset + 1);
            for tag_name in &self.current_tags {
                if let Some(tag) = buffer.tag_table().lookup(tag_name) {
                    buffer.apply_tag(&tag, &start_iter, &end_iter);
                }
            }
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => { if self.is_alternate { self.alt_cursor_y += 1; } else { self.cursor_y += 1; } }
            b'\r' => { if self.is_alternate { self.alt_cursor_x = 0; } else { self.cursor_x = 0; } }
            b'\x08' | b'\x7f' => {
                let cx = if self.is_alternate { &mut self.alt_cursor_x } else { &mut self.cursor_x };
                if *cx > 0 { *cx -= 1; }
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &vte::Params, intermediates: &[u8], _ignore: bool, c: char) {
        let p: Vec<u16> = params.iter().map(|it| it[0]).collect();
        let arg0 = *p.first().unwrap_or(&0) as usize;
        let arg1 = *p.get(1).unwrap_or(&0) as usize;

        if intermediates.contains(&b'?') {
            for param in params.iter() {
                match param[0] {
                    1000 => self.mouse_tracking_mode = if c == 'h' { 1000 } else { 0 },
                    1002 => self.mouse_tracking_mode = if c == 'h' { 1002 } else { 0 },
                    1006 => self.mouse_tracking_mode = if c == 'h' { 1006 } else { 0 },
                    1049 => {
                        if c == 'h' && !self.is_alternate {
                            self.is_alternate = true;
                            self.alternate_buffer.set_text("");
                            self.alt_cursor_x = 0;
                            self.alt_cursor_y = 0;
                            if let Some(v) = self.view.upgrade() { v.set_buffer(Some(&self.alternate_buffer)); }
                        } else if c == 'l' && self.is_alternate {
                            self.is_alternate = false;
                            if let Some(v) = self.view.upgrade() { v.set_buffer(Some(&self.primary_buffer)); }
                        }
                    }
                    _ => {}
                }
            }
            return;
        }

        let mut cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
        let mut cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };

        match c {
            'm' => {
                let sgr_p: Vec<i64> = params.iter().map(|it| it[0] as i64).collect();
                self.apply_sgr(&sgr_p);
            }
            'A' => { cy = cy.saturating_sub(arg0.max(1)); }
            'B' => { cy += arg0.max(1); }
            'C' => { cx += arg0.max(1); }
            'D' => { cx = cx.saturating_sub(arg0.max(1)); }
            'H' | 'f' => {
                cy = arg0.max(1) - 1;
                cx = arg1.max(1) - 1;
            }
            'J' => {
                let buffer = self.active_buffer();
                let mut iter = self.ensure_cursor_position(cx, cy);
                match arg0 {
                    0 => { let mut end = buffer.end_iter(); buffer.delete(&mut iter, &mut end); }
                    1 => { let mut start = buffer.start_iter(); buffer.delete(&mut start, &mut iter); }
                    2 | 3 => { 
                        let mut start = buffer.start_iter(); let mut end = buffer.end_iter(); 
                        buffer.delete(&mut start, &mut end); 
                        cx = 0; cy = 0; 
                    }
                    _ => {}
                }
            }
            'K' => {
                let buffer = self.active_buffer();
                let mut iter = self.ensure_cursor_position(cx, cy);
                match arg0 {
                    0 => {
                        let mut end = iter.clone();
                        end.forward_to_line_end();
                        buffer.delete(&mut iter, &mut end);
                    }
                    2 => {
                        if let Some(mut start) = buffer.iter_at_line(cy as i32) {
                            let mut end = start.clone();
                            end.forward_to_line_end();
                            buffer.delete(&mut start, &mut end);
                            cx = 0;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        
        if self.is_alternate { 
            self.alt_cursor_x = cx; 
            self.alt_cursor_y = cy; 
        } else { 
            self.cursor_x = cx; 
            self.cursor_y = cy; 
        }
    }
}

fn main() {
    let app = Application::builder().application_id("org.terminal.ssh").build();
    app.connect_startup(setup_app);
    app.connect_activate(|app| {
        ensure_connect_window(app, None);
    });
    app.run();
}

fn setup_app(app: &Application) {
    let provider = CssProvider::new();
    provider.load_from_data("
        window { background-color: #1a1a1a; color: #ffffff; }
        .connection-box { background-color: #1a1a1a; }
        entry { 
            border-radius: 8px; 
            padding: 10px; 
            background-color: #2d2d2d; 
            color: #ffffff; 
            caret-color: #ffffff;
            border: 1px solid #3d3d3d; 
            margin-bottom: 12px; 
        }
        entry:focus { border-color: #3d5afe; color: #ffffff; }
        
        popover, popover.menu, popover contents { 
            background-color: #2d2d2d; 
            color: white; 
            border: 1px solid #3d3d3d;
        }
        
        listview, listview row { 
            background-color: transparent; 
            color: white; 
        }
        
        listview row label { 
            color: white; 
        }

        listview row:hover, listview row:selected { 
            background-color: #3d5afe; 
            color: white; 
        }
        
        listview row:selected label { 
            color: white; 
        }

        label.section-title { font-weight: bold; margin-top: 20px; margin-bottom: 10px; color: #3d5afe; font-size: 14px; text-transform: uppercase; }
        .session-row { padding: 8px; border-radius: 6px; }
        .session-row:hover { background-color: #2d2d2d; }
        button.suggested-action { background-color: #3d5afe; color: white; border-radius: 8px; padding: 14px; font-weight: bold; margin-top: 10px; }
        button.secondary-action { background-color: #424242; color: #448aff; border-radius: 8px; padding: 10px; margin-top: 0px; }
        button.destructive-action { background-color: transparent; padding: 4px; border-radius: 4px; color: #ff5252; }
        button.destructive-action:hover { background-color: #e53935; }
        textview { background-color: #000000; color: #0dcf21; font-family: 'Monospace', monospace; font-size: 14px; padding: 10px; }
        textview:focus { caret-color: #3d5afe; }
        .cursor-active { background-color: #ffffff; color: #000000; }
        listbox { background-color: #242424; border-radius: 8px; border: 1px solid #3d3d3d; margin-top: 10px; }
        .session-row { padding: 8px 12px; border-bottom: 1px solid #333; }
        headerbar { background: #2d2d2d; color: white; border-bottom: 1px solid #3d3d3d; }
        
        notebook { background-color: #1a1a1a; border: none; }
        notebook > header { background: #2d2d2d; padding: 5px; }
        notebook > header tab { 
            padding: 8px 16px; 
            margin: 0 2px; 
            border-radius: 6px 6px 0 0; 
            background: #3d3d3d; 
            color: #888; 
        }
        notebook > header tab:checked { 
            background: #3d5afe; 
            color: white; 
        }
        notebook > header tab:hover { 
            background: #4d4d4d; 
        }
        notebook stack { background: #1a1a1a; padding: 30px; }
        
        grid { margin-top: 10px; }
        colorbutton { border-radius: 4px; }
        label.palette-label { font-size: 10px; color: #888; margin-top: 5px; }
    ");
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let menubar = gio::Menu::new();
    
    let file_menu = gio::Menu::new();
    file_menu.append(Some("New Connection"), Some("app.new_connection"));
    file_menu.append(Some("Quit"), Some("app.quit"));
    menubar.append_submenu(Some("File"), &file_menu);
    
    let sessions_menu = gio::Menu::new();
    setup_sessions_actions(app, &sessions_menu);
    menubar.append_submenu(Some("Sessions"), &sessions_menu);
    
    app.set_menubar(Some(&menubar));

    let action_new = gio::SimpleAction::new("new_connection", None);
    let app_weak = app.downgrade();
    action_new.connect_activate(move |_, _| {
        if let Some(app) = app_weak.upgrade() {
            ensure_connect_window(&app, None);
        }
    });
    app.add_action(&action_new);
    app.set_accels_for_action("app.new_connection", &["<Primary>n"]);

    let action_quit = gio::SimpleAction::new("quit", None);
    let app_weak2 = app.downgrade();
    action_quit.connect_activate(move |_, _| {
        if let Some(app) = app_weak2.upgrade() {
            app.quit();
        }
    });
    app.add_action(&action_quit);
    app.set_accels_for_action("app.quit", &["<Primary>q"]);
}

fn setup_sessions_actions(app: &Application, menu: &gio::Menu) {
    let sessions = load_sessions();
    for s in sessions {
        let name_safe = s.name.replace(' ', "_").replace('@', "_").replace('.', "_");
        let action_name = format!("connect_{}", name_safe);
        menu.append(Some(&s.name), Some(&format!("app.{}", action_name)));

        let action = gio::SimpleAction::new(&action_name, None);
        let app_weak = app.downgrade();
        let s_clone = s.clone();
        action.connect_activate(move |_, _| {
            if let Some(app) = app_weak.upgrade() {
                handle_connect(&app, &s_clone, s_clone.password.clone());
            }
        });
        app.add_action(&action);
    }
}

fn ensure_connect_window(app: &Application, target_nb: Option<Notebook>) {
    TARGET_NB.with(|cell| *cell.borrow_mut() = match target_nb {
        Some(nb) => nb.downgrade(),
        None => glib::object::WeakRef::new(),
    });
    
    let existing = CONN_WIN.with(|cell| cell.borrow().clone());
    if let Some(win) = existing {
        win.present();
        return;
    }

    let window = ApplicationWindow::builder()
        .application(app)
        .title("SSH Connection Manager")
        .default_width(600)
        .default_height(650)
        .build();
    let header = HeaderBar::new();
    header.set_show_title_buttons(true);
    
    let close_btn = Button::builder().icon_name("window-close-symbolic").tooltip_text("Close").build();
    let win_weak = window.downgrade();
    close_btn.connect_clicked(move |_| {
        if let Some(win) = win_weak.upgrade() {
            win.close();
        }
    });
    header.pack_end(&close_btn);
    window.set_titlebar(Some(&header));

    let connection_box = GtkBox::new(Orientation::Vertical, 0);
    connection_box.add_css_class("connection-box");
    window.set_child(Some(&connection_box));

    let settings_nb = Notebook::new();
    connection_box.append(&settings_nb);

    // Tab 1: Connection (includes Connect button and Sessions List)
    let conn_tab = GtkBox::new(Orientation::Vertical, 12);
    conn_tab.set_margin_top(10);
    conn_tab.set_margin_bottom(10);
    conn_tab.set_margin_start(10);
    conn_tab.set_margin_end(10);
    
    let conn_page = GtkBox::new(Orientation::Vertical, 12);
    conn_page.set_margin_top(20);
    conn_page.set_margin_bottom(20);
    conn_page.set_margin_start(20);
    conn_page.set_margin_end(20);

    // Row 1: Host, Port, Method
    let row1 = GtkBox::new(Orientation::Horizontal, 10);
    let host_entry = Entry::builder().placeholder_text("Host Address (e.g. 1.2.3.4)").hexpand(true).build();
    let port_entry = Entry::builder().placeholder_text("Port").width_chars(6).text("22").build();
    let method_model = StringList::new(&["Password", "Private Key"]);
    let method_dropdown = DropDown::builder().model(&method_model).build();
    row1.append(&host_entry);
    row1.append(&port_entry);
    row1.append(&method_dropdown);
    conn_page.append(&row1);

    // Row 2: User, Password/Key
    let row2 = GtkBox::new(Orientation::Horizontal, 10);
    let user_entry = Entry::builder().placeholder_text("Username").hexpand(true).build();
    let pass_entry = Entry::builder().placeholder_text("Password").visibility(false).hexpand(true).build();
    let save_pass_check = CheckButton::builder().label("Save Password").build();
    
    let key_box = GtkBox::new(Orientation::Horizontal, 5);
    key_box.set_hexpand(true);
    let key_entry = Entry::builder().placeholder_text("Private Key Path").hexpand(true).build();
    let key_btn = Button::builder().icon_name("folder-open-symbolic").build();
    key_box.append(&key_entry);
    key_box.append(&key_btn);
    
    row2.append(&user_entry);
    row2.append(&pass_entry);
    row2.append(&save_pass_check);
    row2.append(&key_box);
    key_box.set_visible(false); // Default to password
    conn_page.append(&row2);

    let pass_weak = pass_entry.downgrade();
    let key_box_weak = key_box.downgrade();
    let sp_weak = save_pass_check.downgrade();
    method_dropdown.connect_selected_notify(move |d| {
        let is_key = d.selected() == 1;
        if let Some(p) = pass_weak.upgrade() { p.set_visible(!is_key); }
        if let Some(sp) = sp_weak.upgrade() { sp.set_visible(!is_key); }
        if let Some(k) = key_box_weak.upgrade() { k.set_visible(is_key); }
    });

    let win_for_key = window.clone();
    let key_e_clone = key_entry.clone();
    key_btn.connect_clicked(move |_| {
        let dialog = gtk::FileChooserDialog::new(
            Some("Select Private Key"),
            Some(&win_for_key),
            gtk::FileChooserAction::Open,
            &[("Open", gtk::ResponseType::Accept), ("Cancel", gtk::ResponseType::Cancel)],
        );
        let key_e = key_e_clone.clone();
        dialog.connect_response(move |d, res| {
            if res == gtk::ResponseType::Accept {
                if let Some(file) = d.file() {
                    if let Some(path) = file.path() {
                        key_e.set_text(&path.to_string_lossy());
                    }
                }
            }
            d.destroy();
        });
        dialog.show();
    });

    let btn_box = GtkBox::new(Orientation::Horizontal, 12);
    let connect_btn = Button::builder().label("Connect").css_classes(["suggested-action"]).hexpand(true).build();
    btn_box.append(&connect_btn);
    let save_btn = Button::builder().label("Save").css_classes(["secondary-action"]).build();
    btn_box.append(&save_btn);
    conn_page.append(&btn_box);

    conn_page.append(&Label::builder().label("Saved Sessions").css_classes(["section-title"]).halign(gtk::Align::Start).build());
    let sessions_list = ListBox::new();
    let scroll_sessions = ScrolledWindow::builder().min_content_height(250).child(&sessions_list).vexpand(true).build();
    conn_page.append(&scroll_sessions);
    conn_tab.append(&conn_page);

    settings_nb.append_page(&conn_tab, Some(&Label::new(Some("Connection"))));

    // Tab 2: Appearance
    let app_tab = GtkBox::new(Orientation::Vertical, 12);
    app_tab.set_margin_top(10);
    app_tab.set_margin_bottom(10);
    app_tab.set_margin_start(10);
    app_tab.set_margin_end(10);
    
    let theme_row = GtkBox::new(Orientation::Horizontal, 12);
    theme_row.append(&Label::new(Some("Theme:")));
    let theme_names: Vec<String> = THEMES.iter().map(|t| t.name.to_string()).chain(std::iter::once("Custom".to_string())).collect();
    let theme_list = StringList::new(&theme_names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let theme_dropdown = DropDown::builder().model(&theme_list).selected(0).build();
    theme_row.append(&theme_dropdown);
    app_tab.append(&theme_row);

    let custom_row = GtkBox::new(Orientation::Horizontal, 12);
    let fg_btn = ColorButton::builder().rgba(&gtk::gdk::RGBA::GREEN).build();
    custom_row.append(&Label::new(Some("Text:")));
    custom_row.append(&fg_btn);

    let bg_btn = ColorButton::builder().rgba(&gtk::gdk::RGBA::BLACK).build();
    custom_row.append(&Label::new(Some("BG:")));
    custom_row.append(&bg_btn);

    let font_list = StringList::new(&["10", "12", "14", "16", "18", "20", "24"]);
    let font_dropdown = DropDown::builder().model(&font_list).selected(2).build();
    custom_row.append(&Label::new(Some("Font:")));
    custom_row.append(&font_dropdown);
    app_tab.append(&custom_row);

    app_tab.append(&Label::builder().label("ANSI Palette").css_classes(["section-title"]).halign(gtk::Align::Start).build());
    let palette_grid = Grid::new();
    palette_grid.set_column_spacing(15);
    palette_grid.set_row_spacing(10);
    let mut palette_buttons = Vec::new();
    let default_p = default_palette();
    for i in 0..16 {
        let col_box = GtkBox::new(Orientation::Vertical, 2);
        let btn = ColorButton::builder().rgba(&hex_to_rgba(&default_p[i])).build();
        let lbl = Label::builder().label(&format!("{}", i)).css_classes(["palette-label"]).build();
        col_box.append(&btn);
        col_box.append(&lbl);
        palette_grid.attach(&col_box, (i % 8) as i32, (i / 8) as i32, 1, 1);
        palette_buttons.push(btn);
    }
    app_tab.append(&palette_grid);
    settings_nb.append_page(&app_tab, Some(&Label::new(Some("Appearance"))));

    // Tab 3: Behavior
    let behave_tab = GtkBox::new(Orientation::Vertical, 12);
    behave_tab.set_margin_top(10);
    behave_tab.set_margin_bottom(10);
    behave_tab.set_margin_start(10);
    behave_tab.set_margin_end(10);
    
    let behavior_row = GtkBox::new(Orientation::Horizontal, 12);
    let cursor_styles = StringList::new(&["Block", "I-Beam", "Underline"]);
    let cursor_dropdown = DropDown::builder().model(&cursor_styles).selected(0).build();
    behavior_row.append(&Label::new(Some("Cursor:")));
    behavior_row.append(&cursor_dropdown);

    let blink_check = CheckButton::builder().label("Blink").active(true).build();
    behavior_row.append(&blink_check);
    behave_tab.append(&behavior_row);

    let ka_box = GtkBox::new(Orientation::Horizontal, 10);
    ka_box.append(&Label::new(Some("Keepalive (s):")));
    let ka_entry = Entry::builder().text("0").width_chars(6).placeholder_text("Seconds (0 to disable)").build();
    ka_box.append(&ka_entry);
    let agent_check = CheckButton::builder().label("Forward Agent").active(false).build();
    ka_box.append(&agent_check);
    behave_tab.append(&ka_box);

    let scroll_box = GtkBox::new(Orientation::Horizontal, 10);
    scroll_box.append(&Label::new(Some("Scrollback Lines:")));
    let scroll_entry = Entry::builder().text("1000").width_chars(8).placeholder_text("Lines").build();
    scroll_box.append(&scroll_entry);
    behave_tab.append(&scroll_box);

    settings_nb.append_page(&behave_tab, Some(&Label::new(Some("Behavior"))));

    // Theme switching logic
    let fg_clone = fg_btn.clone();
    let bg_clone = bg_btn.clone();
    let pal_clone = palette_buttons.clone();
    let h_e_for_theme = host_entry.clone();
    let u_e_for_theme = user_entry.clone();
    let f_d_for_theme = font_dropdown.clone();
    let c_d_for_theme = cursor_dropdown.clone();
    let b_c_for_theme = blink_check.clone();
    let s_e_for_theme = scroll_entry.clone();

    theme_dropdown.connect_selected_notify(move |d| {
        let idx = d.selected();
        if idx < THEMES.len() as u32 {
            IS_PROGRAMMATIC.with(|f| f.set(true));
            let theme = &THEMES[idx as usize];
            fg_clone.set_rgba(&hex_to_rgba(theme.fg));
            bg_clone.set_rgba(&hex_to_rgba(theme.bg));
            for (i, &p) in theme.palette.iter().enumerate() {
                pal_clone[i].set_rgba(&hex_to_rgba(p));
            }
            IS_PROGRAMMATIC.with(|f| f.set(false));
            
            // Proactively update preview/active terms if name matches
            let host = h_e_for_theme.text().to_string();
            let user = u_e_for_theme.text().to_string();
            if !host.is_empty() && !user.is_empty() {
                let sid = format!("{}@{}", user, host);
                let mock_settings = ConnectionSettings {
                    name: sid.clone(), host, port: 22, username: user, password: None,
                    fg_color: theme.fg.to_string(), bg_color: theme.bg.to_string(),
                    font_size: f_d_for_theme.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().parse().unwrap_or(14)).unwrap_or(14),
                    palette: theme.palette.iter().map(|s| s.to_string()).collect(),
                    cursor_style: c_d_for_theme.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_else(|| "Block".to_string()),
                    cursor_blink: b_c_for_theme.is_active(),
                    scrollback: s_e_for_theme.text().parse().unwrap_or(1000),
                    private_key: None, keepalive: 0, agent_forwarding: false,
                    theme: theme.name.to_string(),
                    method: 0, // Default to password for mock
                };
                update_active_terminals(&sid, &mock_settings);
            }
        }
    });

    let theme_d_for_color = theme_dropdown.clone();
    let update_custom = move || {
        if IS_PROGRAMMATIC.with(|f| f.get()) { return; }
        let custom_pos = THEMES.len() as u32;
        if theme_d_for_color.selected() != custom_pos {
            theme_d_for_color.set_selected(custom_pos);
        }
    };

    let up_c = Arc::new(update_custom);
    let up_c1 = up_c.clone();
    fg_btn.connect_rgba_notify(move |_| { (*up_c1)(); });
    let up_c2 = up_c.clone();
    bg_btn.connect_rgba_notify(move |_| { (*up_c2)(); });
    for b in &palette_buttons {
        let up_ci = up_c.clone();
        b.connect_rgba_notify(move |_| { (*up_ci)(); });
    }

    let sessions_vec = load_sessions();
    let sessions_arc = Arc::new(Mutex::new(sessions_vec.clone()));
    populate_list(&sessions_list, &sessions_vec, &host_entry, &port_entry, &user_entry, &pass_entry, &save_pass_check, &fg_btn, &bg_btn, &font_dropdown, &cursor_dropdown, &blink_check, &scroll_entry, &palette_buttons, sessions_arc.clone(), &key_entry, &theme_dropdown, &ka_entry, &agent_check, &method_dropdown);

    let h_e_weak = host_entry.downgrade();
    let p_e_weak = port_entry.downgrade();
    let u_e_weak = user_entry.downgrade();
    let ps_e_weak = pass_entry.downgrade();
    let key_e_weak = key_entry.downgrade();
    let sp_c_weak = save_pass_check.downgrade();
    let theme_weak = theme_dropdown.downgrade();
    let list_weak = sessions_list.downgrade();
    let fg_weak = fg_btn.downgrade();
    let bg_weak = bg_btn.downgrade();
    let font_weak = font_dropdown.downgrade();
    let cur_weak = cursor_dropdown.downgrade();
    let blink_weak = blink_check.downgrade();
    let scroll_weak = scroll_entry.downgrade();
    let ka_weak = ka_entry.downgrade();
    let ag_weak = agent_check.downgrade();
    let method_weak = method_dropdown.downgrade();
    let pal_weaks: Vec<_> = palette_buttons.iter().map(|b| b.downgrade()).collect();
    let sess_clone_for_save = sessions_arc.clone();
    let pal_buttons_clone = palette_buttons.clone();
    save_btn.connect_clicked(move |_| {
        let host_e = match h_e_weak.upgrade() { Some(v) => v, None => return };
        let port_e = match p_e_weak.upgrade() { Some(v) => v, None => return };
        let user_e = match u_e_weak.upgrade() { Some(v) => v, None => return };
        let pass_e = match ps_e_weak.upgrade() { Some(v) => v, None => return };
        let key_e = match key_e_weak.upgrade() { Some(v) => v, None => return };
        let save_p_c = match sp_c_weak.upgrade() { Some(v) => v, None => return };
        let theme_d = match theme_weak.upgrade() { Some(v) => v, None => return };
        let list = match list_weak.upgrade() { Some(v) => v, None => return };
        let fg_b = match fg_weak.upgrade() { Some(v) => v, None => return };
        let bg_b = match bg_weak.upgrade() { Some(v) => v, None => return };
        let font_d = match font_weak.upgrade() { Some(v) => v, None => return };
        let cur_d = match cur_weak.upgrade() { Some(v) => v, None => return };
        let blink_c = match blink_weak.upgrade() { Some(v) => v, None => return };
        let scroll_e = match scroll_weak.upgrade() { Some(v) => v, None => return };
        let ka_e = match ka_weak.upgrade() { Some(v) => v, None => return };
        let ag_c = match ag_weak.upgrade() { Some(v) => v, None => return };
        let method_d = match method_weak.upgrade() { Some(v) => v, None => return };
        let mut pal = Vec::new();
        for pw in &pal_weaks { if let Some(pb) = pw.upgrade() { pal.push(rgba_to_hex(pb.rgba())); } }

        let host = host_e.text().to_string();
        let port = port_e.text().parse::<u16>().unwrap_or(22);
        let user = user_e.text().to_string();
        let pass = pass_e.text().to_string();
        let key = key_e.text().to_string();
        let fg = rgba_to_hex(fg_b.rgba());
        let bg = rgba_to_hex(bg_b.rgba());
        let font_size = font_d.selected_item().unwrap().downcast::<gtk::StringObject>().unwrap().string().parse::<i32>().unwrap_or(14);
        let cur_style = cur_d.selected_item().unwrap().downcast::<gtk::StringObject>().unwrap().string().to_string();
        let blink = blink_c.is_active();
        let scroll = scroll_e.text().parse::<i32>().unwrap_or(1000);
        let keepalive = ka_e.text().parse::<u32>().unwrap_or(0);
        let agent = ag_c.is_active();
        let theme_name = theme_d.selected_item().unwrap().downcast::<gtk::StringObject>().unwrap().string().to_string();

        if !host.is_empty() && !user.is_empty() {
            let mut s = sess_clone_for_save.lock().unwrap();
            let save_pass = save_p_c.is_active();
            let settings = ConnectionSettings { 
                name: format!("{}@{}", user, host), host, port, username: user,
                password: if save_pass && !pass.is_empty() { Some(pass) } else { None },
                fg_color: fg, bg_color: bg, font_size,
                palette: pal,
                cursor_style: cur_style, cursor_blink: blink, scrollback: scroll,
                private_key: if method_d.selected() == 1 { Some(key) } else { None },
                keepalive, agent_forwarding: agent, theme: theme_name,
                method: method_d.selected(),
            };
            let s_name = settings.name.clone();
            if let Some(pos) = s.iter().position(|x| x.host == settings.host && x.username == settings.username) {
                s[pos] = settings.clone();
            } else {
                s.push(settings.clone());
            }
            save_sessions(&s);
            populate_list(&list, &s, &host_e, &port_e, &user_e, &pass_e, &save_p_c, &fg_b, &bg_b, &font_d, &cur_d, &blink_c, &scroll_e, &pal_buttons_clone, sess_clone_for_save.clone(), &key_e, &theme_d, &ka_e, &ag_c, &method_d);
            update_active_terminals(&s_name, &settings);
        }
    });

    let app_weak = app.downgrade();
    let h_e_weak = host_entry.downgrade();
    let p_e_weak = port_entry.downgrade();
    let u_e_weak = user_entry.downgrade();
    let pass_e_weak = pass_entry.downgrade();
    let key_e_weak = key_entry.downgrade();
    let fg_weak = fg_btn.downgrade();
    let bg_weak = bg_btn.downgrade();
    let font_weak = font_dropdown.downgrade();
    let cur_weak = cursor_dropdown.downgrade();
    let blink_weak = blink_check.downgrade();
    let scroll_weak = scroll_entry.downgrade();
    let ka_weak = ka_entry.downgrade();
    let ag_weak = agent_check.downgrade();
    let theme_weak = theme_dropdown.downgrade();
    let method_weak = method_dropdown.downgrade();
    let pal_weaks: Vec<_> = palette_buttons.iter().map(|b| b.downgrade()).collect();
    connect_btn.connect_clicked(move |_| {
        let app = match app_weak.upgrade() { Some(v) => v, None => return };
        let host_e = match h_e_weak.upgrade() { Some(v) => v, None => return };
        let port_e = match p_e_weak.upgrade() { Some(v) => v, None => return };
        let user_e = match u_e_weak.upgrade() { Some(v) => v, None => return };
        let pass_e = match pass_e_weak.upgrade() { Some(v) => v, None => return };
        let key_e = match key_e_weak.upgrade() { Some(v) => v, None => return };
        let fg_b = match fg_weak.upgrade() { Some(v) => v, None => return };
        let bg_b = match bg_weak.upgrade() { Some(v) => v, None => return };
        let font_d = match font_weak.upgrade() { Some(v) => v, None => return };
        let cur_d = match cur_weak.upgrade() { Some(v) => v, None => return };
        let blink_c = match blink_weak.upgrade() { Some(v) => v, None => return };
        let scroll_e = match scroll_weak.upgrade() { Some(v) => v, None => return };
        let ka_e = match ka_weak.upgrade() { Some(v) => v, None => return };
        let ag_c = match ag_weak.upgrade() { Some(v) => v, None => return };
        let theme_d = match theme_weak.upgrade() { Some(v) => v, None => return };
        let method_d = match method_weak.upgrade() { Some(v) => v, None => return };
        let mut pal = Vec::new();
        for pw in &pal_weaks { if let Some(pb) = pw.upgrade() { pal.push(rgba_to_hex(pb.rgba())); } }

        let host = host_e.text().to_string();
        let port = port_e.text().parse::<u16>().unwrap_or(22);
        let user = user_e.text().to_string();
        let pass = pass_e.text().to_string();
        let key = key_e.text().to_string();
        let fg = rgba_to_hex(fg_b.rgba());
        let bg = rgba_to_hex(bg_b.rgba());
        let font_size = font_d.selected_item().unwrap().downcast::<gtk::StringObject>().unwrap().string().parse::<i32>().unwrap_or(14);
        let cur_style = cur_d.selected_item().unwrap().downcast::<gtk::StringObject>().unwrap().string().to_string();
        let blink = blink_c.is_active();
        let scroll = scroll_e.text().parse::<i32>().unwrap_or(1000);
        let keepalive = ka_e.text().parse::<u32>().unwrap_or(0);
        let agent = ag_c.is_active();
        let theme_name = theme_d.selected_item().unwrap().downcast::<gtk::StringObject>().unwrap().string().to_string();

        let mut pal = Vec::new();
        for pw in &pal_weaks { if let Some(pb) = pw.upgrade() { pal.push(rgba_to_hex(pb.rgba())); } }

        if host.is_empty() || user.is_empty() { return; }
        
        handle_connect(&app, &ConnectionSettings {
            name: format!("{}@{}", user, host), host, port, username: user,
            password: if pass.is_empty() { None } else { Some(pass.clone()) },
            fg_color: fg, bg_color: bg, font_size,
            palette: if pal.len() == 16 { pal } else { default_palette() },
            cursor_style: cur_style, cursor_blink: blink, scrollback: scroll,
            private_key: if method_d.selected() == 1 { Some(key) } else { None },
            keepalive, agent_forwarding: agent, theme: theme_name,
            method: method_d.selected(),
        }, if pass.is_empty() { None } else { Some(pass) });
    });

    window.connect_destroy(move |_| {
        CONN_WIN.with(|cell| *cell.borrow_mut() = None);
        TARGET_NB.with(|cell| *cell.borrow_mut() = glib::object::WeakRef::new());
    });
    
    window.set_hide_on_close(true);
    CONN_WIN.with(|cell| *cell.borrow_mut() = Some(window.clone()));
    window.present();
}

fn handle_connect(app: &Application, settings: &ConnectionSettings, override_pass: Option<String>) {
    let target = TARGET_NB.with(|cell| cell.borrow().upgrade());
    if let Some(nb) = target {
        add_terminal_tab(&nb, settings, override_pass);
        TARGET_NB.with(|cell| *cell.borrow_mut() = glib::object::WeakRef::new());
        if let Some(win) = nb.root().and_then(|r| r.downcast::<gtk::Window>().ok()) {
            win.present();
        }
        if let Some(win) = CONN_WIN.with(|cell| cell.borrow().clone()) {
            win.close();
        }
    } else {
        create_terminal_window(app, settings, override_pass);
    }
}

fn create_terminal_window(app: &Application, settings: &ConnectionSettings, override_pass: Option<String>) {
    let window = ApplicationWindow::builder().application(app).title(&format!("Terminal SSH: {}", settings.name)).default_width(1000).default_height(750).build();
    let header = HeaderBar::new();
    header.set_show_title_buttons(true);
    
    let menu = gio::Menu::new();
    menu.append(Some("New Tab (Same Host)"), Some("win.new_tab_same"));
    menu.append(Some("New Tab (Other Host)"), Some("win.new_tab_other"));
    menu.append(Some("New Connection (New Window)"), Some("win.new_window"));
    
    let menu_btn = MenuButton::builder().icon_name("list-add-symbolic").menu_model(&menu).tooltip_text("New Connection Options").build();
    header.pack_start(&menu_btn);
    
    let close_btn = Button::builder().icon_name("window-close-symbolic").tooltip_text("Close Window").build();
    let win_weak = window.downgrade();
    close_btn.connect_clicked(move |_| {
        if let Some(win) = win_weak.upgrade() {
            win.close();
        }
    });
    header.pack_end(&close_btn);
    window.set_titlebar(Some(&header));

    let notebook = Notebook::new();
    notebook.set_scrollable(true);
    notebook.set_show_border(false);
    window.set_child(Some(&notebook));
    
    let nb_weak = notebook.downgrade();
    let app_weak = app.downgrade();
    let s_clone = settings.clone();
    let p_clone = override_pass.clone();
    
    let action_same = gio::SimpleAction::new("new_tab_same", None);
    action_same.connect_activate(move |_, _| {
        if let Some(nb) = nb_weak.upgrade() {
            add_terminal_tab(&nb, &s_clone, p_clone.clone());
        }
    });
    window.add_action(&action_same);

    let app_weak2 = app_weak.clone();
    let nb_weak2 = notebook.downgrade();
    let action_other = gio::SimpleAction::new("new_tab_other", None);
    action_other.connect_activate(move |_, _| {
        if let (Some(app), Some(nb)) = (app_weak2.upgrade(), nb_weak2.upgrade()) {
            ensure_connect_window(&app, Some(nb));
        }
    });
    window.add_action(&action_other);

    let app_weak3 = app_weak.clone();
    let action_window = gio::SimpleAction::new("new_window", None);
    action_window.connect_activate(move |_, _| {
        if let Some(app) = app_weak3.upgrade() {
            ensure_connect_window(&app, None);
        }
    });
    window.add_action(&action_window);

    add_terminal_tab(&notebook, settings, override_pass);
    window.present();
}

fn add_terminal_tab(notebook: &Notebook, settings: &ConnectionSettings, override_pass: Option<String>) {
    let text_view = TextView::builder().editable(false).monospace(true).cursor_visible(true).focusable(true).can_focus(true).build();
    let provider = CssProvider::new();
    provider.load_from_data(&format!(
        "textview, textview text {{ background-color: {}; color: {}; font-size: {}pt; }}",
        settings.bg_color, settings.fg_color, settings.font_size
    ));
    text_view.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 200);

    let scrolled = ScrolledWindow::builder().child(&text_view).vexpand(true).build();
    let label = Label::new(Some(&settings.name));
    let index = notebook.append_page(&scrolled, Some(&label));
    notebook.set_current_page(Some(index));
    text_view.grab_focus();

    let (input_tx, input_rx) = flume::unbounded::<ConnectionControl>();
    let (output_tx, output_rx) = flume::unbounded::<Vec<u8>>();
    
    let term_state = Arc::new(Mutex::new(TerminalState::new(text_view.downgrade(), settings.palette.clone())));
    
    // Register for active updates
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

    glib::timeout_add_local(Duration::from_millis(10), move || {
        let tv = match tv_weak.upgrade() { Some(v) => v, None => return glib::ControlFlow::Break };
        
        // PTY Resize checking
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
    let tv_for_key = text_view.clone();
    let key_controller = EventControllerKey::new();
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    key_controller.connect_key_pressed(move |_controller, keyval, _keycode, state| {
        let is_ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
        if is_ctrl {
            if keyval == gtk::gdk::Key::c || keyval == gtk::gdk::Key::C {
                let clipboard = tv_for_key.clipboard();
                if let Some((start, end)) = tv_for_key.buffer().selection_bounds() {
                    let text = tv_for_key.buffer().text(&start, &end, false);
                    clipboard.set_text(&text);
                }
                return glib::Propagation::Stop;
            } else if keyval == gtk::gdk::Key::v || keyval == gtk::gdk::Key::V {
                let clipboard = tv_for_key.clipboard();
                let itx_clone = itx.clone();
                clipboard.read_text_async(None::<&gio::Cancellable>, move |result| {
                    if let Ok(Some(text)) = result {
                        let _ = itx_clone.send(ConnectionControl::Input(text.into_bytes()));
                    }
                });
                return glib::Propagation::Stop;
            }
        }
        if let Some(data) = keyval_to_bytes(keyval, state) {
            let _ = itx.send(ConnectionControl::Input(data));
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    text_view.add_controller(key_controller);

    let s_clone = settings.clone();
    let final_pass = override_pass.or(settings.password.clone());
    text_view.buffer().set_text(&format!("Connecting to {}...\n", settings.name));

    std::thread::spawn(move || {
        match connect_ssh(&s_clone.host, s_clone.port, &s_clone.username, final_pass.as_deref().unwrap_or(""), s_clone.private_key.as_deref(), s_clone.keepalive, s_clone.agent_forwarding) {
            Ok((session, mut channel)) => {
                let _ = output_tx.send(b"Connection established.\r\n".to_vec());
                let _ = session.set_blocking(false);
                let mut buffer = [0; 8192];
                loop {
                    match channel.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(size) => { let _ = output_tx.send(buffer[..size].to_vec()); }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            while let Ok(ctrl) = input_rx.try_recv() {
                                match ctrl {
                                    ConnectionControl::Input(data) => {
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
                                    },
                                    ConnectionControl::Resize(cols, rows, width_px, height_px) => {
                                        let _ = channel.request_pty_size(cols, rows, Some(width_px), Some(height_px));
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

fn populate_list(list: &ListBox, sessions: &[ConnectionSettings], host_e: &Entry, port_e: &Entry, user_e: &Entry, pass_e: &Entry, save_p: &CheckButton, fg_b: &ColorButton, bg_b: &ColorButton, font_d: &DropDown, cur_d: &DropDown, blink_c: &CheckButton, scroll_e: &Entry, palette_btns: &[ColorButton], sessions_arc: Arc<Mutex<Vec<ConnectionSettings>>>, key_e: &Entry, theme_d: &DropDown, ka_e: &Entry, ag_c: &CheckButton, method_d: &DropDown) {
    while let Some(child) = list.first_child() { list.remove(&child); }
    for (index, s) in sessions.iter().enumerate() {
        let row_box = GtkBox::new(Orientation::Horizontal, 10);
        row_box.add_css_class("session-row");
        let label = Label::builder().label(&s.name).halign(gtk::Align::Start).hexpand(true).build();
        row_box.append(&label);
        
        let save_row_btn = Button::builder().icon_name("document-save-symbolic").css_classes(["secondary-action"]).tooltip_text("Save current settings to this session").build();
        row_box.append(&save_row_btn);
        
        let delete_btn = Button::builder().icon_name("user-trash-symbolic").css_classes(["destructive-action"]).tooltip_text("Delete session").build();
        row_box.append(&delete_btn);
        let row = gtk::ListBoxRow::builder().child(&row_box).build();
        list.append(&row);
        
        let s_arc_clone = sessions_arc.clone();
        let list_weak = list.downgrade();
        let h_e_weak = host_e.downgrade();
        let p_e_weak = port_e.downgrade();
        let u_e_weak = user_e.downgrade();
        let ps_e_weak = pass_e.downgrade();
        let save_p_weak = save_p.downgrade();
        let fg_weak = fg_b.downgrade();
        let bg_weak = bg_b.downgrade();
        let font_weak = font_d.downgrade();
        let cur_weak = cur_d.downgrade();
        let blink_weak = blink_c.downgrade();
        let scroll_weak = scroll_e.downgrade();
        let key_e_weak = key_e.downgrade();
        let theme_d_weak = theme_d.downgrade();
        let ka_e_weak = ka_e.downgrade();
        let ag_c_weak = ag_c.downgrade();
        let method_d_weak = method_d.downgrade();
        let p_buttons = palette_btns.to_vec();
        let name_clone = s.name.clone();

        let s_arc_for_save = sessions_arc.clone();
        let h_e_w2 = h_e_weak.clone();
        let p_e_w2 = p_e_weak.clone();
        let u_e_w2 = u_e_weak.clone();
        let ps_e_w2 = ps_e_weak.clone();
        let sp_w2 = save_p_weak.clone();
        let th_w2 = theme_d_weak.clone();
        let fg_w2 = fg_weak.clone();
        let bg_w2 = bg_weak.clone();
        let f_d_w2 = font_weak.clone();
        let c_d_w2 = cur_weak.clone();
        let bc_w2 = blink_weak.clone();
        let sc_w2 = scroll_weak.clone();
        let ka_w2 = ka_e_weak.clone();
        let ac_w2 = ag_c_weak.clone();
        let ls_w2 = list.downgrade();
        let ke_w2 = key_e_weak.clone();
        let pb_w2 = p_buttons.clone();
        let method_d_w2 = method_d_weak.clone();

        save_row_btn.connect_clicked(move |_| {
            let h_e = match h_e_w2.upgrade() { Some(v) => v, None => return };
            let p_e = match p_e_w2.upgrade() { Some(v) => v, None => return };
            let u_e = match u_e_w2.upgrade() { Some(v) => v, None => return };
            let ps_e = match ps_e_w2.upgrade() { Some(v) => v, None => return };
            let save_p_c = match sp_w2.upgrade() { Some(v) => v, None => return };
            let theme_d = match th_w2.upgrade() { Some(v) => v, None => return };
            let fg_b = match fg_w2.upgrade() { Some(v) => v, None => return };
            let bg_b = match bg_w2.upgrade() { Some(v) => v, None => return };
            let font_d = match f_d_w2.upgrade() { Some(v) => v, None => return };
            let cur_d = match c_d_w2.upgrade() { Some(v) => v, None => return };
            let blink_c = match bc_w2.upgrade() { Some(v) => v, None => return };
            let scroll_e = match sc_w2.upgrade() { Some(v) => v, None => return };
            let ka_e = match ka_w2.upgrade() { Some(v) => v, None => return };
            let ag_c = match ac_w2.upgrade() { Some(v) => v, None => return };
            let list = match ls_w2.upgrade() { Some(v) => v, None => return };
            let key_e_up = match ke_w2.upgrade() { Some(v) => v, None => return };
            let method_d_up = match method_d_w2.upgrade() { Some(v) => v, None => return };
            
            let mut pal = Vec::new();
            for btn in &pb_w2 { pal.push(rgba_to_hex(btn.rgba())); }

            let settings = ConnectionSettings {
                name: name_clone.clone(),
                host: h_e.text().to_string(),
                port: p_e.text().parse().unwrap_or(22),
                username: u_e.text().to_string(),
                password: if save_p_c.is_active() && method_d_up.selected() == 0 { Some(ps_e.text().to_string()) } else { None },
                fg_color: rgba_to_hex(fg_b.rgba()),
                bg_color: rgba_to_hex(bg_b.rgba()),
                font_size: font_d.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().parse().unwrap_or(14)).unwrap_or(14),
                palette: pal,
                cursor_style: cur_d.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_else(|| "Block".to_string()),
                cursor_blink: blink_c.is_active(),
                scrollback: scroll_e.text().parse().unwrap_or(1000),
                private_key: if method_d_up.selected() == 1 { Some(key_e_up.text().to_string()) } else { None },
                keepalive: ka_e.text().parse().unwrap_or(0),
                agent_forwarding: ag_c.is_active(),
                theme: theme_d.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_else(|| "Custom".to_string()),
                method: method_d_up.selected(),
            };

            let mut s_vec = s_arc_for_save.lock().unwrap();
            if index < s_vec.len() {
                s_vec[index] = settings.clone();
                save_sessions(&s_vec);
                populate_list(&list, &s_vec, &h_e, &p_e, &u_e, &ps_e, &save_p_c, &fg_b, &bg_b, &font_d, &cur_d, &blink_c, &scroll_e, &pb_w2, s_arc_for_save.clone(), &key_e_up, &theme_d, &ka_e, &ag_c, &method_d_up);
                update_active_terminals(&settings.name, &settings);
            }
        });

        delete_btn.connect_clicked(move |_| {
            let list_up = match list_weak.upgrade() { Some(v) => v, None => return };
            let h_e = match h_e_weak.upgrade() { Some(v) => v, None => return };
            let p_e = match p_e_weak.upgrade() { Some(v) => v, None => return };
            let u_e = match u_e_weak.upgrade() { Some(v) => v, None => return };
            let ps_e = match ps_e_weak.upgrade() { Some(v) => v, None => return };
            let save_p_up = match save_p_weak.upgrade() { Some(v) => v, None => return };
            let fg = match fg_weak.upgrade() { Some(v) => v, None => return };
            let bg = match bg_weak.upgrade() { Some(v) => v, None => return };
            let font = match font_weak.upgrade() { Some(v) => v, None => return };
            let cur = match cur_weak.upgrade() { Some(v) => v, None => return };
            let blink = match blink_weak.upgrade() { Some(v) => v, None => return };
            let scroll = match scroll_weak.upgrade() { Some(v) => v, None => return };
            let key_up = match key_e_weak.upgrade() { Some(v) => v, None => return };
            let theme_up = match theme_d_weak.upgrade() { Some(v) => v, None => return };
            let ka_up = match ka_e_weak.upgrade() { Some(v) => v, None => return };
            let ag_up = match ag_c_weak.upgrade() { Some(v) => v, None => return };
            let method_up = match method_d_weak.upgrade() { Some(v) => v, None => return };
            
            let mut s = s_arc_clone.lock().unwrap();
            if index < s.len() {
                s.remove(index);
                save_sessions(&s);
                populate_list(&list_up, &s, &h_e, &p_e, &u_e, &ps_e, &save_p_up, &fg, &bg, &font, &cur, &blink, &scroll, &p_buttons, s_arc_clone.clone(), &key_up, &theme_up, &ka_up, &ag_up, &method_up);
            }
        });
    }
    
    let sessions_vec = sessions.to_vec();
    let h_e_weak = host_e.downgrade();
    let p_e_weak = port_e.downgrade();
    let u_e_weak = user_entry_downgrade(user_e); 
    let ps_e_weak = pass_entry_downgrade(pass_e);
    let save_p_weak = save_p.downgrade();
    let fg_weak = fg_button_downgrade(fg_b);
    let bg_weak = bg_button_downgrade(bg_b);
    let font_weak = font_dropdown_downgrade(font_d);
    let cur_weak = cur_dropdown_downgrade(cur_d);
    let blink_weak = blink_check_downgrade(blink_c);
    let scroll_weak = scroll_entry_downgrade(scroll_e);
    let key_e_weak = key_e.downgrade();
    let theme_d_weak = theme_d.downgrade();
    let ka_e_weak = ka_e.downgrade();
    let ag_c_weak = ag_c.downgrade();
    let method_d_weak = method_d.downgrade();
    let pal_buttons = palette_btns.to_vec();
    
    list.connect_row_activated(move |_, row| {
        let h_e = match h_e_weak.upgrade() { Some(v) => v, None => return };
        let p_e = match p_e_weak.upgrade() { Some(v) => v, None => return };
        let u_e = match upgrade_user_e(&u_e_weak) { Some(v) => v, None => return };
        let ps_e = match upgrade_pass_e(&ps_e_weak) { Some(v) => v, None => return };
        let save_p = match save_p_weak.upgrade() { Some(v) => v, None => return };
        let fg = match upgrade_fg_b(&fg_weak) { Some(v) => v, None => return };
        let bg = match upgrade_bg_b(&bg_weak) { Some(v) => v, None => return };
        let font = match upgrade_font_d(&font_weak) { Some(v) => v, None => return };
        let cur = match upgrade_cur_d(&cur_weak) { Some(v) => v, None => return };
        let blink = match upgrade_blink_c(&blink_weak) { Some(v) => v, None => return };
        let scroll = match upgrade_scroll_e(&scroll_weak) { Some(v) => v, None => return };
        let key_up = match key_e_weak.upgrade() { Some(v) => v, None => return };
        let theme_up = match theme_d_weak.upgrade() { Some(v) => v, None => return };
        let ka_up = match ka_e_weak.upgrade() { Some(v) => v, None => return };
        let ag_up = match ag_c_weak.upgrade() { Some(v) => v, None => return };
        let method_up = match method_d_weak.upgrade() { Some(v) => v, None => return };

        if let Some(s) = sessions_vec.get(row.index() as usize) {
            h_e.set_text(&s.host); p_e.set_text(&s.port.to_string()); u_e.set_text(&s.username);
            ps_e.set_text(s.password.as_deref().unwrap_or(""));
            key_up.set_text(s.private_key.as_deref().unwrap_or(""));
            save_p.set_active(s.password.is_some());
            method_up.set_selected(s.method);
            fg.set_rgba(&hex_to_rgba(&s.fg_color));
            bg.set_rgba(&hex_to_rgba(&s.bg_color));
            
            if let Some(model) = font.model().and_then(|m| m.downcast::<StringList>().ok()) {
                for i in 0..model.n_items() {
                    if let Some(str_obj) = model.string(i) {
                        if str_obj == s.font_size.to_string() { font.set_selected(i); break; }
                    }
                }
            }
            if let Some(model) = cur.model().and_then(|m| m.downcast::<StringList>().ok()) {
                for i in 0..model.n_items() {
                    if let Some(str_obj) = model.string(i) {
                        if str_obj == s.cursor_style { cur.set_selected(i); break; }
                    }
                }
            }
            if let Some(model) = theme_up.model().and_then(|m| m.downcast::<StringList>().ok()) {
                for i in 0..model.n_items() {
                    if let Some(str_obj) = model.string(i) {
                        if str_obj == s.theme { theme_up.set_selected(i); break; }
                    }
                }
            }
            blink.set_active(s.cursor_blink);
            scroll.set_text(&s.scrollback.to_string());
            ka_up.set_text(&s.keepalive.to_string());
            ag_up.set_active(s.agent_forwarding);
            for i in 0..16 {
                if i < s.palette.len() && i < pal_buttons.len() {
                    pal_buttons[i].set_rgba(&hex_to_rgba(&s.palette[i]));
                }
            }
        }
    });
}

fn user_entry_downgrade(e: &Entry) -> glib::object::WeakRef<Entry> { e.downgrade() }
fn pass_entry_downgrade(e: &Entry) -> glib::object::WeakRef<Entry> { e.downgrade() }
fn fg_button_downgrade(b: &ColorButton) -> glib::object::WeakRef<ColorButton> { b.downgrade() }
fn bg_button_downgrade(b: &ColorButton) -> glib::object::WeakRef<ColorButton> { b.downgrade() }
fn font_dropdown_downgrade(d: &DropDown) -> glib::object::WeakRef<DropDown> { d.downgrade() }
fn cur_dropdown_downgrade(d: &DropDown) -> glib::object::WeakRef<DropDown> { d.downgrade() }
fn blink_check_downgrade(c: &CheckButton) -> glib::object::WeakRef<CheckButton> { c.downgrade() }
fn scroll_entry_downgrade(e: &Entry) -> glib::object::WeakRef<Entry> { e.downgrade() }

fn upgrade_user_e(w: &glib::object::WeakRef<Entry>) -> Option<Entry> { w.upgrade() }
fn upgrade_pass_e(w: &glib::object::WeakRef<Entry>) -> Option<Entry> { w.upgrade() }
fn upgrade_fg_b(w: &glib::object::WeakRef<ColorButton>) -> Option<ColorButton> { w.upgrade() }
fn upgrade_bg_b(w: &glib::object::WeakRef<ColorButton>) -> Option<ColorButton> { w.upgrade() }
fn upgrade_font_d(w: &glib::object::WeakRef<DropDown>) -> Option<DropDown> { w.upgrade() }
fn upgrade_cur_d(w: &glib::object::WeakRef<DropDown>) -> Option<DropDown> { w.upgrade() }
fn upgrade_blink_c(w: &glib::object::WeakRef<CheckButton>) -> Option<CheckButton> { w.upgrade() }
fn upgrade_scroll_e(w: &glib::object::WeakRef<Entry>) -> Option<Entry> { w.upgrade() }

fn rgba_to_hex(rgba: gtk::gdk::RGBA) -> String {
    format!("#{:02x}{:02x}{:02x}", 
        (rgba.red() * 255.0) as u8, 
        (rgba.green() * 255.0) as u8, 
        (rgba.blue() * 255.0) as u8)
}

fn hex_to_rgba(hex: &str) -> gtk::gdk::RGBA {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
    gtk::gdk::RGBA::builder().red(r).green(g).blue(b).alpha(1.0).build()
}

enum ConnectionControl {
    Input(Vec<u8>),
    Resize(u32, u32, u32, u32),
}

fn keyval_to_bytes(keyval: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> Option<Vec<u8>> {
    use gtk::gdk::Key;
    let is_ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
    match keyval {
        Key::Return | Key::KP_Enter => Some(b"\r".to_vec()),
        Key::BackSpace => Some(b"\x7f".to_vec()),
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
                let val = keyval.to_unicode().unwrap_or('\0');
                if val >= 'a' && val <= 'z' { return Some(vec![(val as u8) - b'a' + 1]); }
                if val >= 'A' && val <= 'Z' { return Some(vec![(val as u8) - b'A' + 1]); }
            }
            if let Some(c) = keyval.to_unicode() { if c >= ' ' { return Some(c.to_string().into_bytes()); } }
            None
        }
    }
}

fn connect_ssh(host: &str, port: u16, user: &str, pass: &str, key_path: Option<&str>, keepalive: u32, agent_forwarding: bool) -> Result<(ssh2::Session, ssh2::Channel), Box<dyn std::error::Error + Send + Sync>> {
    let tcp = TcpStream::connect(format!("{}:{}", host, port))?;
    let mut sess = SshSession::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;
    
    if let Some(path) = key_path {
        sess.userauth_pubkey_file(user, None, std::path::Path::new(path), None)?;
    } else if !pass.is_empty() {
        sess.userauth_password(user, pass)?;
    } else {
        return Err("Authentication failed: No password or key specified".into());
    }

    if keepalive > 0 {
        sess.set_keepalive(true, keepalive);
    }
    
    let mut channel = sess.channel_session()?;
    if agent_forwarding {
        let _ = channel.request_auth_agent_forwarding();
    }
    channel.request_pty("xterm-256color", None, Some((80, 24, 0, 0)))?;
    channel.shell()?;
    Ok((sess, channel))
}

fn update_active_terminals(session_id: &str, settings: &ConnectionSettings) {
    ACTIVE_TERMINALS.with(|at| {
        let mut list = at.borrow_mut();
        // Clear dead ones while we are at it
        list.retain(|t| t.text_view.upgrade().is_some());
        
        for t in list.iter() {
            if t.session_id == session_id {
                t.css_provider.load_from_data(&format!(
                    "textview, textview text {{ background-color: {}; color: {}; font-size: {}pt; }}",
                    settings.bg_color, settings.fg_color, settings.font_size
                ));
                if let Ok(mut state) = t.term_state.lock() {
                    state.update_palette(&settings.palette);
                    state.primary_buffer.tag_table().foreach(|tag| {
                        if tag.name().map(|n| n == "bold").unwrap_or(false) {
                            tag.set_foreground(None); 
                        }
                    });
                    state.alternate_buffer.tag_table().foreach(|tag| {
                        if tag.name().map(|n| n == "bold").unwrap_or(false) {
                            tag.set_foreground(None); 
                        }
                    });
                }
            }
        }
    });
}
