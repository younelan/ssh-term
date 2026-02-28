use gtk4 as gtk;
use gtk::gio;
use gtk::prelude::*;
use glib::prelude::IsA; // Added this line as per instruction
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
    #[serde(default = "default_term_type")]
    pub term_type: String,
}

fn default_keepalive() -> u32 { 0 }
fn default_theme() -> String { "Default".to_string() }
fn default_method() -> u32 { 0 }
fn default_term_type() -> String { "xterm-256color".to_string() }

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

fn get_256_color(idx: u8, palette: &[String]) -> String {
    if (idx as usize) < palette.len() {
        return palette[idx as usize].clone();
    }
    if idx >= 16 && idx <= 231 {
        let n = idx - 16;
        let b = n % 6;
        let g = (n / 6) % 6;
        let r = (n / 36) % 6;
        let val = |x| if x == 0 { 0 } else { 55 + x * 40 };
        return format!("#{:02x}{:02x}{:02x}", val(r), val(g), val(b));
    }
    if idx >= 232 {
        let gray = 8 + (idx - 232) * 10;
        return format!("#{:02x}{:02x}{:02x}", gray, gray, gray);
    }
    "#ffffff".to_string()
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
    tab_label: glib::WeakRef<Label>,
}

impl TerminalState {
    fn new(view: glib::WeakRef<TextView>, tab_label: glib::WeakRef<Label>, palette: Vec<String>) -> Self {
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
                
                let tag_bg = TextTag::new(Some(&format!("bg-{}", code.parse::<u32>().unwrap() + 10)));
                tag_bg.set_background(Some(color));
                tag_table.add(&tag_bg);
            }
        }
        
        for i in 0..=255 {
            let color = get_256_color(i, &palette);
            let tag_fg = TextTag::new(Some(&format!("fg-256-{}", i)));
            tag_fg.set_foreground(Some(&color));
            tag_table.add(&tag_fg);
            
            let tag_bg = TextTag::new(Some(&format!("bg-256-{}", i)));
            tag_bg.set_background(Some(&color));
            tag_table.add(&tag_bg);
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
            tab_label,
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
                
                let bg_name = format!("bg-{}", code.parse::<u32>().unwrap() + 10);
                if let Some(tag) = tag_table.lookup(&bg_name) {
                    tag.set_background(Some(color));
                }
            }
        }
        
        for i in 0..=255 {
            let color = get_256_color(i, palette);
            if let Some(tag) = tag_table.lookup(&format!("fg-256-{}", i)) {
                tag.set_foreground(Some(&color));
            }
            if let Some(tag) = tag_table.lookup(&format!("bg-256-{}", i)) {
                tag.set_background(Some(&color));
            }
        }
    }

    fn apply_sgr(&mut self, params: &[i64]) {
        if params.is_empty() || params[0] == 0 {
            self.current_tags.clear();
            return;
        }
        let mut i = 0;
        while i < params.len() {
            let param = params[i];
            i += 1;
            match param {
                0 => self.current_tags.clear(),
                1 => if !self.current_tags.contains(&"bold".to_string()) { self.current_tags.push("bold".to_string()); },
                30..=37 | 90..=97 => {
                    self.current_tags.retain(|t| !t.starts_with("fg-"));
                    self.current_tags.push(format!("fg-{}", param));
                }
                38 => {
                    if i + 1 < params.len() && params[i] == 5 {
                        let color_idx = params[i + 1];
                        i += 2;
                        self.current_tags.retain(|t| !t.starts_with("fg-"));
                        self.current_tags.push(format!("fg-256-{}", color_idx));
                    }
                }
                40..=47 | 100..=107 => {
                    self.current_tags.retain(|t| !t.starts_with("bg-"));
                    self.current_tags.push(format!("bg-{}", param));
                }
                48 => {
                    if i + 1 < params.len() && params[i] == 5 {
                        let color_idx = params[i + 1];
                        i += 2;
                        self.current_tags.retain(|t| !t.starts_with("bg-"));
                        self.current_tags.push(format!("bg-256-{}", color_idx));
                    }
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
            '@' => {
                let count = arg0.max(1);
                let buffer = self.active_buffer();
                let mut iter = self.ensure_cursor_position(cx, cy);
                let spaces = " ".repeat(count);
                buffer.insert(&mut iter, &spaces);
            }
            'P' => {
                let count = arg0.max(1);
                let buffer = self.active_buffer();
                let mut start = self.ensure_cursor_position(cx, cy);
                let mut end = start.clone();
                for _ in 0..count {
                    if !end.ends_line() { end.forward_char(); }
                }
                buffer.delete(&mut start, &mut end);
            }
            'L' => {
                let count = arg0.max(1);
                let buffer = self.active_buffer();
                let mut iter = self.ensure_cursor_position(0, cy);
                let newlines = "\n".repeat(count);
                buffer.insert(&mut iter, &newlines);
            }
            'M' => {
                let count = arg0.max(1);
                let buffer = self.active_buffer();
                if let Some(mut start) = buffer.iter_at_line(cy as i32) {
                    let mut end = start.clone();
                    for _ in 0..count {
                        if !end.is_end() { end.forward_visible_line(); }
                    }
                    buffer.delete(&mut start, &mut end);
                }
            }
            'X' => {
                let count = arg0.max(1);
                let buffer = self.active_buffer();
                let mut start = self.ensure_cursor_position(cx, cy);
                let mut end = start.clone();
                for _ in 0..count {
                    if !end.ends_line() { end.forward_char(); }
                }
                buffer.delete(&mut start, &mut end);
                let spaces = " ".repeat(count);
                let mut insert_iter = self.ensure_cursor_position(cx, cy);
                buffer.insert(&mut insert_iter, &spaces);
            }
            'S' => {
                let count = arg0.max(1);
                let buffer = self.active_buffer();
                if let Some(mut start) = buffer.iter_at_line(0) {
                    let mut end = start.clone();
                    for _ in 0..count {
                        if !end.is_end() { end.forward_visible_line(); }
                    }
                    buffer.delete(&mut start, &mut end);
                }
            }
            'T' => {
                let count = arg0.max(1);
                let buffer = self.active_buffer();
                let mut start = buffer.start_iter();
                let newlines = "\n".repeat(count);
                buffer.insert(&mut start, &newlines);
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

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.len() >= 2 {
            if params[0] == b"0" || params[0] == b"2" {
                if let Ok(title) = std::str::from_utf8(params[1]) {
                    if let Some(lbl) = self.tab_label.upgrade() {
                        lbl.set_text(title);
                    }
                }
            }
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

    let name_row = GtkBox::new(Orientation::Horizontal, 10);
    let name_entry = Entry::builder().placeholder_text("Session Name (e.g. Prod Server)").hexpand(true).build();
    let term_model = StringList::new(&["xterm-256color", "xterm", "vt100", "linux", "rxvt-unicode-256color", "tmux-256color"]);
    let term_dropdown = DropDown::builder().model(&term_model).build();
    name_row.append(&name_entry);
    name_row.append(&term_dropdown);
    conn_page.append(&name_row);

    // Row 1: Host, Port, Method
    let row1 = GtkBox::new(Orientation::Horizontal, 10);
    let host_entry = Entry::builder().placeholder_text("Host Address (e.g. 1.2.3.4)").hexpand(true).build();
    let port_entry = Entry::builder().placeholder_text("Port").max_length(5).width_chars(5).max_width_chars(5).text("22").halign(gtk::Align::Start).build();
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
    let name_e_clone = name_entry.clone();
    let host_e_clone = host_entry.clone();
    let port_e_clone = port_entry.clone();
    let user_e_clone = user_entry.clone();
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
                    term_type: "xterm-256color".to_string(),
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
    populate_list(&sessions_list, &sessions_vec, &name_entry, &host_entry, &port_entry, &user_entry, &pass_entry, &save_pass_check, &fg_btn, &bg_btn, &font_dropdown, &cursor_dropdown, &blink_check, &scroll_entry, &palette_buttons, sessions_arc.clone(), &key_entry, &theme_dropdown, &ka_entry, &agent_check, &method_dropdown);

    let name_e_weak = name_entry.downgrade();
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
    let term_weak = term_dropdown.downgrade();
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
        let term_d = match term_weak.upgrade() { Some(v) => v, None => return };
        let name_e = match name_e_weak.upgrade() { Some(v) => v, None => return };
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
        let name_val = name_e.text().to_string();
        let name = if name_val.trim().is_empty() { format!("{}@{}", user, host) } else { name_val };

        if !host.is_empty() && !user.is_empty() {
            let mut s = sess_clone_for_save.lock().unwrap();
            let save_pass = save_p_c.is_active();
            let settings = ConnectionSettings { 
                name: name.clone(), host, port, username: user,
                password: if save_pass && !pass.is_empty() { Some(pass) } else { None },
                fg_color: fg, bg_color: bg, font_size,
                palette: pal,
                cursor_style: cur_style, cursor_blink: blink, scrollback: scroll,
                private_key: if method_d.selected() == 1 { Some(key) } else { None },
                keepalive, agent_forwarding: agent, theme: theme_name,
                method: method_d.selected(),
                term_type: term_d.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_else(|| "xterm-256color".to_string()),
            };
            let s_name = settings.name.clone();
            if let Some(pos) = s.iter().position(|x| x.host == settings.host && x.username == settings.username) {
                s[pos] = settings.clone();
            } else {
                s.push(settings.clone());
            }
            save_sessions(&s);
            populate_list(&list, &s, &name_e, &host_e, &port_e, &user_e, &pass_e, &save_p_c, &fg_b, &bg_b, &font_d, &cur_d, &blink_c, &scroll_e, &pal_buttons_clone, sess_clone_for_save.clone(), &key_e, &theme_d, &ka_e, &ag_c, &method_d);
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
    let term_weak = term_dropdown.downgrade();
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
        let term_d = match term_weak.upgrade() { Some(v) => v, None => return };
        let name_val = name_e_clone.text().to_string();
        let name = if name_val.trim().is_empty() { format!("{}@{}", user_e_clone.text(), host_e_clone.text()) } else { name_val };
        
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
            name: name, host, port, username: user,
            password: if pass.is_empty() { None } else { Some(pass.clone()) },
            fg_color: fg, bg_color: bg, font_size,
            palette: if pal.len() == 16 { pal } else { default_palette() },
            cursor_style: cur_style, cursor_blink: blink, scrollback: scroll,
            private_key: if method_d.selected() == 1 { Some(key) } else { None },
            keepalive, agent_forwarding: agent, theme: theme_name,
            method: method_d.selected(),
            term_type: term_d.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_else(|| "xterm-256color".to_string()),
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
    let label_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    label_box.append(&label);
    
    let index = notebook.append_page(&scrolled, Some(&label_box));
    notebook.set_current_page(Some(index));
    text_view.grab_focus();

    // Right-Click Context Menu for active Terminal View
    let context_menu = gio::Menu::new();
    let theme_submenu = gio::Menu::new();
    let action_group = gio::SimpleActionGroup::new();
    label_box.insert_action_group("term", Some(&action_group));

    let sid_for_menu = settings.name.clone();
    
    for (theme_idx, theme) in THEMES.iter().enumerate() {
        let action_name = format!("set_theme_{}", theme_idx);
        theme_submenu.append(Some(theme.name), Some(&format!("term.{}", action_name)));

        let action_theme = gio::SimpleAction::new(&action_name, None);
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
    gesture.set_button(3); // Right click
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

    let ts_weak_loop = ts_weak.clone();
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
            clipboard.read_text_async(None::<&gio::Cancellable>, move |result| {
                if let Ok(Some(text)) = result {
                    let _ = itx_clone.send(ConnectionControl::Input(text.into_bytes()));
                }
            });
            return glib::Propagation::Stop;
        } else if is_clear {
            let mut state = s_arc_key.lock().unwrap();
            
            // Native GTK clear - we clear both primary and alternate
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
                    // Extract current setting from JSON to have base knowledge
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
                        
                        term.css_provider.load_from_data(&format!(
                            "textview, textview text {{ background-color: {}; color: {}; font-size: {}pt; }}",
                            s.bg_color, s.fg_color, new_size
                        ));
                        
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

    let s_clone = settings.clone();
    let final_pass = override_pass.or(settings.password.clone());
    text_view.buffer().set_text(&format!("Connecting to {}...\n", settings.name));

    std::thread::spawn(move || {
        match connect_ssh(&s_clone.host, s_clone.port, &s_clone.username, final_pass.as_deref().unwrap_or(""), s_clone.private_key.as_deref(), s_clone.keepalive, s_clone.agent_forwarding, &s_clone.term_type) {
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

fn populate_list(list: &ListBox, sessions: &[ConnectionSettings], name_e: &Entry, host_e: &Entry, port_e: &Entry, user_e: &Entry, pass_e: &Entry, save_p: &CheckButton, fg_b: &ColorButton, bg_b: &ColorButton, font_d: &DropDown, cur_d: &DropDown, blink_c: &CheckButton, scroll_e: &Entry, palette_btns: &[ColorButton], sessions_arc: Arc<Mutex<Vec<ConnectionSettings>>>, key_e: &Entry, theme_d: &DropDown, ka_e: &Entry, ag_c: &CheckButton, method_d: &DropDown) {
    while let Some(child) = list.first_child() { list.remove(&child); }
    for (index, s) in sessions.iter().enumerate() {
        let row_box = GtkBox::new(Orientation::Horizontal, 10);
        row_box.add_css_class("session-row");
        let label = Label::builder().label(&s.name).halign(gtk::Align::Start).hexpand(true).build();
        row_box.append(&label);
        
        let menu_btn = MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("Session Options")
            .css_classes(["flat"])
            .halign(gtk::Align::End)
            .build();
        row_box.append(&menu_btn);

        let menu = gio::Menu::new();
        let action_group = gio::SimpleActionGroup::new();
        menu_btn.insert_action_group("row", Some(&action_group));
        menu_btn.set_menu_model(Some(&menu));

        menu.append(Some("Save Current Settings"), Some("row.save"));
        menu.append(Some("Rename Session"), Some("row.rename"));
        menu.append(Some("Clone Session"), Some("row.clone"));
        menu.append(Some("Delete"), Some("row.delete"));

        let row = gtk::ListBoxRow::builder().child(&row_box).build();
        list.append(&row);
        
        let s_arc_clone = sessions_arc.clone();
        let list_weak = list.downgrade();
        let name_e_weak = name_e.downgrade();
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
        let n_e_w2 = name_e_weak.clone();
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

        let action_save = gio::SimpleAction::new("save", None);
        action_save.connect_activate(move |_, _| {
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
            let name_e_up = match n_e_w2.upgrade() { Some(v) => v, None => return };
            
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
                term_type: s_arc_for_save.lock().unwrap().get(index).map(|s| s.term_type.clone()).unwrap_or_else(|| "xterm-256color".to_string()),
            };

            let mut s_vec = s_arc_for_save.lock().unwrap();
            if index < s_vec.len() {
                s_vec[index] = settings.clone();
                save_sessions(&s_vec);
                populate_list(&list, &s_vec, &name_e_up, &h_e, &p_e, &u_e, &ps_e, &save_p_c, &fg_b, &bg_b, &font_d, &cur_d, &blink_c, &scroll_e, &pb_w2, s_arc_for_save.clone(), &key_e_up, &theme_d, &ka_e, &ag_c, &method_d_up);
                update_active_terminals(&settings.name, &settings);
            }
        });
        action_group.add_action(&action_save);

        let action_delete = gio::SimpleAction::new("delete", None);
        let list_w3 = list.downgrade();
        let n_e_w_del = name_e_weak.clone();
        let h_e_w_del = h_e_weak.clone();
        let p_e_w_del = p_e_weak.clone();
        let u_e_w_del = u_e_weak.clone();
        let ps_e_w_del = ps_e_weak.clone();
        let sp_w_del = save_p_weak.clone();
        let fg_w_del = fg_weak.clone();
        let bg_w_del = bg_weak.clone();
        let f_d_w_del = font_weak.clone();
        let c_d_w_del = cur_weak.clone();
        let bc_w_del = blink_weak.clone();
        let sc_w_del = scroll_weak.clone();
        let ke_w_del = key_e_weak.clone();
        let th_w_del = theme_d_weak.clone();
        let ka_w_del = ka_e_weak.clone();
        let ac_w_del = ag_c_weak.clone();
        let md_w_del = method_d_weak.clone();
        let s_arc_for_del = sessions_arc.clone();
        let pb_w_del = p_buttons.clone();
        action_delete.connect_activate(move |_, _| {
            let list_up = match list_w3.upgrade() { Some(v) => v, None => return };
            let name_up = match n_e_w_del.upgrade() { Some(v) => v, None => return };
            let h_e = match h_e_w_del.upgrade() { Some(v) => v, None => return };
            let p_e = match p_e_w_del.upgrade() { Some(v) => v, None => return };
            let u_e = match u_e_w_del.upgrade() { Some(v) => v, None => return };
            let ps_e = match ps_e_w_del.upgrade() { Some(v) => v, None => return };
            let save_p_up = match sp_w_del.upgrade() { Some(v) => v, None => return };
            let fg = match fg_w_del.upgrade() { Some(v) => v, None => return };
            let bg = match bg_w_del.upgrade() { Some(v) => v, None => return };
            let font = match f_d_w_del.upgrade() { Some(v) => v, None => return };
            let cur = match c_d_w_del.upgrade() { Some(v) => v, None => return };
            let blink = match bc_w_del.upgrade() { Some(v) => v, None => return };
            let scroll = match sc_w_del.upgrade() { Some(v) => v, None => return };
            let key_up = match ke_w_del.upgrade() { Some(v) => v, None => return };
            let theme_up = match th_w_del.upgrade() { Some(v) => v, None => return };
            let ka_up = match ka_w_del.upgrade() { Some(v) => v, None => return };
            let ag_up = match ac_w_del.upgrade() { Some(v) => v, None => return };
            let method_up = match md_w_del.upgrade() { Some(v) => v, None => return };
            
            let mut s = s_arc_for_del.lock().unwrap();
            if index < s.len() {
                s.remove(index);
                save_sessions(&s);
                populate_list(&list_up, &s, &name_up, &h_e, &p_e, &u_e, &ps_e, &save_p_up, &fg, &bg, &font, &cur, &blink, &scroll, &pb_w_del, s_arc_for_del.clone(), &key_up, &theme_up, &ka_up, &ag_up, &method_up);
            }
        });
        action_group.add_action(&action_delete);

        // Rename Action (Dialog Prompt)
        let s_arc_ren = sessions_arc.clone();
        let list_w_ren = list.downgrade();
        let n_e_w_ren = name_e_weak.clone();
        let win_weak_ren = find_parent_window(list); 
        let action_rename = gio::SimpleAction::new("rename", None);
        let current_name = s.name.clone();
        
        let h_e_ren = h_e_weak.clone();
        let p_e_ren = p_e_weak.clone();
        let u_e_ren = u_e_weak.clone();
        let ps_e_ren = ps_e_weak.clone();
        let sp_ren = save_p_weak.clone();
        let fg_ren = fg_weak.clone();
        let bg_ren = bg_weak.clone();
        let f_d_ren = font_weak.clone();
        let c_d_ren = cur_weak.clone();
        let bc_ren = blink_weak.clone();
        let sc_ren = scroll_weak.clone();
        let pb_ren = p_buttons.clone();
        let ke_ren = key_e_weak.clone();
        let th_ren = theme_d_weak.clone();
        let ka_ren = ka_e_weak.clone();
        let ac_ren = ag_c_weak.clone();
        let md_ren = method_d_weak.clone();

        action_rename.connect_activate(move |_, _| {
            let win = match win_weak_ren.upgrade() { Some(w) => w, None => return };
            let dialog = gtk::MessageDialog::builder()
                .transient_for(&win)
                .modal(true)
                .message_type(gtk::MessageType::Question)
                .buttons(gtk::ButtonsType::OkCancel)
                .text("Rename Session")
                .secondary_text("Enter the new name for this session:")
                .build();
                
            let entry = Entry::builder().text(&current_name).margin_top(10).margin_bottom(10).margin_start(10).margin_end(10).build();
            dialog.content_area().append(&entry);
            entry.grab_focus();
            
            let s_arc_ren_inner = s_arc_ren.clone();
            let n_e_weak_ren_inner = n_e_w_ren.clone();
            let l_w_ren_inner = list_w_ren.clone();
            
            // Re-clone UI references for the dialog closure
            let h_e_ren_i = h_e_ren.clone();
            let p_e_ren_i = p_e_ren.clone();
            let u_e_ren_i = u_e_ren.clone();
            let ps_e_ren_i = ps_e_ren.clone();
            let sp_ren_i = sp_ren.clone();
            let fg_ren_i = fg_ren.clone();
            let bg_ren_i = bg_ren.clone();
            let f_d_ren_i = f_d_ren.clone();
            let c_d_ren_i = c_d_ren.clone();
            let bc_ren_i = bc_ren.clone();
            let sc_ren_i = sc_ren.clone();
            let pb_ren_i = pb_ren.clone();
            let ke_ren_i = ke_ren.clone();
            let th_ren_i = th_ren.clone();
            let ka_ren_i = ka_ren.clone();
            let ac_ren_i = ac_ren.clone();
            let md_ren_i = md_ren.clone();

            dialog.connect_response(move |d, res| {
                if res == gtk::ResponseType::Ok {
                    let new_name = entry.text().to_string();
                    if !new_name.trim().is_empty() {
                        let mut s = s_arc_ren_inner.lock().unwrap();
                        if index < s.len() {
                            s[index].name = new_name;
                            save_sessions(&s);
                            
                            if let (Some(ls_up), Some(ne_up), Some(he_up), Some(pe_up), Some(ue_up), Some(pse_up), Some(sp_up), Some(fg_up), Some(bg_up), Some(fd_up), Some(cd_up), Some(bc_up), Some(sc_up), Some(ke_up), Some(th_up), Some(ka_up), Some(ac_up), Some(md_up)) = (
                                l_w_ren_inner.upgrade(), n_e_weak_ren_inner.upgrade(), h_e_ren_i.upgrade(), p_e_ren_i.upgrade(), u_e_ren_i.upgrade(), ps_e_ren_i.upgrade(), sp_ren_i.upgrade(), fg_ren_i.upgrade(), bg_ren_i.upgrade(), f_d_ren_i.upgrade(), c_d_ren_i.upgrade(), bc_ren_i.upgrade(), sc_ren_i.upgrade(), ke_ren_i.upgrade(), th_ren_i.upgrade(), ka_ren_i.upgrade(), ac_ren_i.upgrade(), md_ren_i.upgrade()
                            ) {
                                populate_list(&ls_up, &s, &ne_up, &he_up, &pe_up, &ue_up, &pse_up, &sp_up, &fg_up, &bg_up, &fd_up, &cd_up, &bc_up, &sc_up, &pb_ren_i, s_arc_ren_inner.clone(), &ke_up, &th_up, &ka_up, &ac_up, &md_up);
                            }
                        }
                    }
                }
                d.destroy();
            });
            dialog.show();
        });
        action_group.add_action(&action_rename);

        // Clone Action
        let s_arc_cl = sessions_arc.clone();
        let list_w_cl = list.downgrade();
        let n_e_w_cl = name_e_weak.clone();
        let win_weak_cl = find_parent_window(list); 
        let action_clone = gio::SimpleAction::new("clone", None);
        let s_to_clone = s.clone();
        
        let h_e_cl = h_e_weak.clone();
        let p_e_cl = p_e_weak.clone();
        let u_e_cl = u_e_weak.clone();
        let ps_e_cl = ps_e_weak.clone();
        let sp_cl = save_p_weak.clone();
        let fg_cl = fg_weak.clone();
        let bg_cl = bg_weak.clone();
        let f_d_cl = font_weak.clone();
        let c_d_cl = cur_weak.clone();
        let bc_cl = blink_weak.clone();
        let sc_cl = scroll_weak.clone();
        let pb_cl = p_buttons.clone();
        let ke_cl = key_e_weak.clone();
        let th_cl = theme_d_weak.clone();
        let ka_cl = ka_e_weak.clone();
        let ac_cl = ag_c_weak.clone();
        let md_cl = method_d_weak.clone();

        action_clone.connect_activate(move |_, _| {
            let win = match win_weak_cl.upgrade() { Some(w) => w, None => return };
            let dialog = gtk::MessageDialog::builder()
                .transient_for(&win)
                .modal(true)
                .message_type(gtk::MessageType::Question)
                .buttons(gtk::ButtonsType::OkCancel)
                .text("Clone Session")
                .secondary_text("Enter a name for the new cloned session:")
                .build();
                
            let default_clone_name = format!("{} (Copy)", s_to_clone.name);
            let entry = Entry::builder().text(&default_clone_name).margin_top(10).margin_bottom(10).margin_start(10).margin_end(10).build();
            dialog.content_area().append(&entry);
            entry.grab_focus();
            
            let s_arc_cl_inner = s_arc_cl.clone();
            let stc_inner = s_to_clone.clone();
            let l_w_cl_inner = list_w_cl.clone();
            let n_e_w_cl_inner = n_e_w_cl.clone();
            
            let h_e_cl_i = h_e_cl.clone();
            let p_e_cl_i = p_e_cl.clone();
            let u_e_cl_i = u_e_cl.clone();
            let ps_e_cl_i = ps_e_cl.clone();
            let sp_cl_i = sp_cl.clone();
            let fg_cl_i = fg_cl.clone();
            let bg_cl_i = bg_cl.clone();
            let f_d_cl_i = f_d_cl.clone();
            let c_d_cl_i = c_d_cl.clone();
            let bc_cl_i = bc_cl.clone();
            let sc_cl_i = sc_cl.clone();
            let pb_cl_i = pb_cl.clone();
            let ke_cl_i = ke_cl.clone();
            let th_cl_i = th_cl.clone();
            let ka_cl_i = ka_cl.clone();
            let ac_cl_i = ac_cl.clone();
            let md_cl_i = md_cl.clone();

            dialog.connect_response(move |d, res| {
                if res == gtk::ResponseType::Ok {
                    let new_name = entry.text().to_string();
                    if !new_name.trim().is_empty() {
                        let mut cloned_settings = stc_inner.clone();
                        cloned_settings.name = new_name;
                        
                        let mut s = s_arc_cl_inner.lock().unwrap();
                        s.push(cloned_settings);
                        save_sessions(&s);
                        
                        if let (Some(ls_up), Some(ne_up), Some(he_up), Some(pe_up), Some(ue_up), Some(pse_up), Some(sp_up), Some(fg_up), Some(bg_up), Some(fd_up), Some(cd_up), Some(bc_up), Some(sc_up), Some(ke_up), Some(th_up), Some(ka_up), Some(ac_up), Some(md_up)) = (
                            l_w_cl_inner.upgrade(), n_e_w_cl_inner.upgrade(), h_e_cl_i.upgrade(), p_e_cl_i.upgrade(), u_e_cl_i.upgrade(), ps_e_cl_i.upgrade(), sp_cl_i.upgrade(), fg_cl_i.upgrade(), bg_cl_i.upgrade(), f_d_cl_i.upgrade(), c_d_cl_i.upgrade(), bc_cl_i.upgrade(), sc_cl_i.upgrade(), ke_cl_i.upgrade(), th_cl_i.upgrade(), ka_cl_i.upgrade(), ac_cl_i.upgrade(), md_cl_i.upgrade()
                        ) {
                            populate_list(&ls_up, &s, &ne_up, &he_up, &pe_up, &ue_up, &pse_up, &sp_up, &fg_up, &bg_up, &fd_up, &cd_up, &bc_up, &sc_up, &pb_cl_i, s_arc_cl_inner.clone(), &ke_up, &th_up, &ka_up, &ac_up, &md_up);
                        }
                    }
                }
                d.destroy();
            });
            dialog.show();
        });
        action_group.add_action(&action_clone);

        // Right-Click Context Menu for Theme
        row_box.insert_action_group("row", Some(&action_group));
        let context_menu = gio::Menu::new();
        let theme_submenu = gio::Menu::new();
        
        for (theme_idx, theme) in THEMES.iter().enumerate() {
            let action_name = format!("set_theme_{}", theme_idx);
            theme_submenu.append(Some(theme.name), Some(&format!("row.{}", action_name)));

            let action_theme = gio::SimpleAction::new(&action_name, None);
            let s_arc_th = sessions_arc.clone();
            let theme_name = theme.name.to_string();
            let theme_fg = theme.fg.to_string();
            let theme_bg = theme.bg.to_string();
            let theme_pal = theme.palette.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            
            let l_w_th = list.downgrade();
            let n_e_w_th = name_e_weak.clone();
            let h_e_w_th = h_e_weak.clone();
            let p_e_w_th = p_e_weak.clone();
            let u_e_w_th = u_e_weak.clone();
            let ps_e_w_th = ps_e_weak.clone();
            let sp_w_th = save_p_weak.clone();
            let fg_w_th = fg_weak.clone();
            let bg_w_th = bg_weak.clone();
            let f_d_w_th = font_weak.clone();
            let c_d_w_th = cur_weak.clone();
            let bc_w_th = blink_weak.clone();
            let sc_w_th = scroll_weak.clone();
            let pb_w_th = p_buttons.clone();
            let ke_w_th = key_e_weak.clone();
            let th_w_th = theme_d_weak.clone();
            let ka_w_th = ka_e_weak.clone();
            let ac_w_th = ag_c_weak.clone();
            let md_w_th = method_d_weak.clone();

            action_theme.connect_activate(move |_, _| {
                let mut s = s_arc_th.lock().unwrap();
                if index < s.len() {
                    s[index].theme = theme_name.clone();
                    s[index].fg_color = theme_fg.clone();
                    s[index].bg_color = theme_bg.clone();
                    s[index].palette = theme_pal.clone();
                    save_sessions(&s);
                    
                    if let (Some(ls_up), Some(ne_up), Some(he_up), Some(pe_up), Some(ue_up), Some(pse_up), Some(sp_up), Some(fg_up), Some(bg_up), Some(fd_up), Some(cd_up), Some(bc_up), Some(sc_up), Some(ke_up), Some(th_up), Some(ka_up), Some(ac_up), Some(md_up)) = (
                        l_w_th.upgrade(), n_e_w_th.upgrade(), h_e_w_th.upgrade(), p_e_w_th.upgrade(), u_e_w_th.upgrade(), ps_e_w_th.upgrade(), sp_w_th.upgrade(), fg_w_th.upgrade(), bg_w_th.upgrade(), f_d_w_th.upgrade(), c_d_w_th.upgrade(), bc_w_th.upgrade(), sc_w_th.upgrade(), ke_w_th.upgrade(), th_w_th.upgrade(), ka_w_th.upgrade(), ac_w_th.upgrade(), md_w_th.upgrade()
                    ) {
                        populate_list(&ls_up, &s, &ne_up, &he_up, &pe_up, &ue_up, &pse_up, &sp_up, &fg_up, &bg_up, &fd_up, &cd_up, &bc_up, &sc_up, &pb_w_th, s_arc_th.clone(), &ke_up, &th_up, &ka_up, &ac_up, &md_up);
                    }
                }
            });
            action_group.add_action(&action_theme);
        }
        context_menu.append_submenu(Some("Quick Set Theme"), &theme_submenu);

        let popover = gtk::PopoverMenu::from_model(Some(&context_menu));
        popover.set_parent(&row_box);
        popover.set_has_arrow(false);
        
        let gesture = gtk::GestureClick::new();
        gesture.set_button(3); // Right click
        let p_weak = popover.downgrade();
        gesture.connect_pressed(move |_, _, _, _| {
            if let Some(p) = p_weak.upgrade() {
                p.popup();
            }
        });
        row_box.add_controller(gesture);
    }
    
    let sessions_vec = sessions.to_vec();
    let name_e_weak = name_e.downgrade();
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
            let n_e = match name_e_weak.upgrade() { Some(v) => v, None => return };
            n_e.set_text(&s.name);
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

fn find_parent_window<W: glib::prelude::IsA<gtk::Widget>>(widget: &W) -> glib::object::WeakRef<gtk::ApplicationWindow> {
    let mut current = widget.parent();
    while let Some(parent) = current {
        if let Ok(win) = parent.clone().downcast::<gtk::ApplicationWindow>() {
            return win.downgrade();
        }
        current = parent.parent();
    }
    glib::object::WeakRef::new()
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

fn connect_ssh(host: &str, port: u16, user: &str, pass: &str, key_path: Option<&str>, keepalive: u32, agent_forwarding: bool, term_type: &str) -> Result<(ssh2::Session, ssh2::Channel), Box<dyn std::error::Error + Send + Sync>> {
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
    channel.request_pty(term_type, None, Some((80, 24, 0, 0)))?;
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
