use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnectionSettings {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default = "default_fg")]
    pub fg_color: String,
    #[serde(default = "default_bg")]
    pub bg_color: String,
    #[serde(default = "default_font_size")]
    pub font_size: i32,
    #[serde(default = "default_palette")]
    pub palette: Vec<String>,
    #[serde(default = "default_cursor_style")]
    pub cursor_style: String,
    #[serde(default = "default_cursor_blink")]
    pub cursor_blink: bool,
    #[serde(default = "default_scrollback")]
    pub scrollback: i32,
    #[serde(default)]
    pub private_key: Option<String>,
    #[serde(default = "default_keepalive")]
    pub keepalive: u32,
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

pub struct Theme {
    pub name: &'static str,
    pub fg: &'static str,
    pub bg: &'static str,
    pub palette: [&'static str; 16],
}

pub const THEMES: [Theme; 6] = [
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

pub fn get_256_color(n: u8, cur_palette: &[String]) -> String {
    if n < 16 && (n as usize) < cur_palette.len() {
        cur_palette[n as usize].clone()
    } else if n < 232 {
        let n = n - 16;
        let r = (n / 36) * 51;
        let g = ((n % 36) / 6) * 51;
        let b = (n % 6) * 51;
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    } else {
        let gray = (n - 232) * 10 + 8;
        format!("#{:02x}{:02x}{:02x}", gray, gray, gray)
    }
}

pub fn get_config_path() -> std::path::PathBuf {
    let mut path = dirs_next::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push(".terminal_ssh_sessions.json");
    path
}

pub fn load_sessions() -> Vec<ConnectionSettings> {
    let path = get_config_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    }
}

pub fn save_sessions(sessions: &[ConnectionSettings]) {
    let path = get_config_path();
    if let Ok(content) = serde_json::to_string_pretty(sessions) {
        let _ = std::fs::write(path, content);
    }
}
