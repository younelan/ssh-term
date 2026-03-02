use base64::Engine as _;
const BASE64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::STANDARD;
use gtk4 as gtk;
use gtk::{glib, gio, Label, TextBuffer, TextTag, TextView};
use gtk::prelude::*;
use vte::Perform;

// ── Graph widget ──────────────────────────────────────────────────────────────

const GRAPH_PALETTE: &[&str] = &[
    "#5b8dee", "#ff6b6b", "#51cf66", "#fcc419",
    "#cc5de8", "#ff922b", "#20c997", "#f06595",
];

pub struct GraphData {
    pub kind:       String,        // "bar" | "line" | "pie"
    pub values:     Vec<Vec<f64>>, // outer = series, inner = data points
    pub labels:     Vec<String>,   // x-axis / pie-slice labels
    pub series_labels: Vec<String>,
    pub color_strs: Vec<String>,
    pub title:      String,
    pub bg:         String,        // background hex, "" = default
}

fn parse_hex(hex: &str) -> (f64, f64, f64) {
    let s = hex.trim_start_matches('#');
    if s.len() < 6 { return (0.5, 0.5, 0.5); }
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(128) as f64 / 255.0;
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(128) as f64 / 255.0;
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(128) as f64 / 255.0;
    (r, g, b)
}

fn series_color(gd: &GraphData, s: usize) -> (f64, f64, f64) {
    parse_hex(gd.color_strs.get(s).map(|c| c.as_str())
        .unwrap_or(GRAPH_PALETTE[s % GRAPH_PALETTE.len()]))
}

/// Returns (width, height) of a text string on the given cairo context, or (0,0) on error.
fn text_size(cr: &gtk::cairo::Context, s: &str) -> (f64, f64) {
    match cr.text_extents(s) {
        Ok(e) => (e.width(), e.height()),
        Err(_) => (0.0, 0.0),
    }
}

fn set_fg(cr: &gtk::cairo::Context) {
    cr.set_source_rgb(0.886, 0.910, 0.941);
}
fn set_dim(cr: &gtk::cairo::Context) {
    cr.set_source_rgb(0.58, 0.635, 0.722);
}

fn draw_graph(cr: &gtk::cairo::Context, w: i32, h: i32, gd: &GraphData) {
    // Background
    if gd.bg.is_empty() {
        cr.set_source_rgb(0.137, 0.118, 0.176);
    } else {
        let (r, g, b) = parse_hex(&gd.bg);
        cr.set_source_rgb(r, g, b);
    }
    let _ = cr.paint();

    let flat: Vec<f64> = gd.values.iter().flat_map(|s| s.iter().copied()).collect();
    if flat.is_empty() { return; }

    match gd.kind.as_str() {
        "pie"  => draw_pie(cr, w, h, gd),
        "line" => draw_line(cr, w, h, gd),
        _      => draw_bar(cr, w, h, gd),
    }

    // Title
    if !gd.title.is_empty() {
        set_fg(cr);
        cr.set_font_size(13.0);
        let (tw, _) = text_size(cr, &gd.title);
        cr.move_to((w as f64 - tw) / 2.0, 15.0);
        let _ = cr.show_text(&gd.title);
    }
}

fn grid_and_yaxis(cr: &gtk::cairo::Context, w: f64, margin_l: f64, margin_r: f64,
                  margin_t: f64, ph: f64, h: f64, min_val: f64, range: f64) {
    for i in 0..=4 {
        let y = margin_t + ph * (1.0 - i as f64 / 4.0);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
        cr.set_line_width(0.5);
        cr.move_to(margin_l, y);
        cr.line_to(w - margin_r, y);
        let _ = cr.stroke();
        set_dim(cr);
        cr.set_font_size(9.0);
        let val = min_val + range * i as f64 / 4.0;
        let label = if range < 10.0 { format!("{:.1}", val) } else { format!("{:.0}", val) };
        let (tw, th) = text_size(cr, &label);
        cr.move_to((margin_l - tw - 4.0).max(0.0), y + th / 2.0);
        let _ = cr.show_text(&label);
        let _ = h; // suppress unused warning
    }
}

fn draw_bar(cr: &gtk::cairo::Context, w: i32, h: i32, gd: &GraphData) {
    let n_series = gd.values.len();
    let n_groups = gd.values.iter().map(|s| s.len()).max().unwrap_or(0);
    if n_groups == 0 { return; }

    let margin_l = 42.0f64;
    let margin_b = if gd.labels.is_empty() { 10.0 } else { 28.0 };
    let margin_t = if gd.title.is_empty() { 10.0 } else { 26.0 };
    let margin_r = 10.0f64;
    let pw = w as f64 - margin_l - margin_r;
    let ph = h as f64 - margin_t - margin_b;

    let max_val: f64 = gd.values.iter().flat_map(|s| s.iter()).cloned().fold(0.0f64, f64::max).max(1.0);

    grid_and_yaxis(cr, w as f64, margin_l, margin_r, margin_t, ph, h as f64, 0.0, max_val);

    let group_w = pw / n_groups as f64;
    let bar_gap  = 2.0f64;
    let group_pad = 4.0f64;
    let bar_w = ((group_w - group_pad * 2.0 - bar_gap * (n_series.saturating_sub(1)) as f64)
                  / n_series as f64).max(2.0);

    for g in 0..n_groups {
        for s in 0..n_series {
            let val = gd.values[s].get(g).copied().unwrap_or(0.0);
            let bh = (val / max_val * ph).max(0.0);
            let x  = margin_l + g as f64 * group_w + group_pad
                     + s as f64 * (bar_w + bar_gap);
            let y  = margin_t + ph - bh;

            let (r, cg, b) = series_color(gd, s);
            cr.set_source_rgb(r, cg, b);
            cr.rectangle(x, y, bar_w, bh);
            let _ = cr.fill();

            // Value label on top (if room)
            if bh > 14.0 {
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.6);
                cr.set_font_size(8.0);
                let label = if val == val.floor() { format!("{:.0}", val) } else { format!("{:.1}", val) };
                let (tw, _) = text_size(cr, &label);
                cr.move_to(x + bar_w / 2.0 - tw / 2.0, y + 10.0);
                let _ = cr.show_text(&label);
            }
        }

        // Group label
        if let Some(lbl) = gd.labels.get(g) {
            set_dim(cr);
            cr.set_font_size(9.0);
            let gx = margin_l + g as f64 * group_w + group_w / 2.0;
            let (lw, _) = text_size(cr, lbl);
            cr.move_to(gx - lw / 2.0, h as f64 - margin_b + 12.0);
            let _ = cr.show_text(lbl);
        }
    }

    // Series legend (top-right)
    if n_series > 1 {
        for s in 0..n_series {
            let (r, cg, b) = series_color(gd, s);
            let lx = margin_l + s as f64 * 80.0;
            let ly = margin_t - 6.0;
            cr.set_source_rgb(r, cg, b);
            cr.rectangle(lx, ly, 10.0, 8.0);
            let _ = cr.fill();
            set_dim(cr);
            cr.set_font_size(9.0);
            let name = gd.series_labels.get(s).map(|s| s.as_str()).unwrap_or("");
            cr.move_to(lx + 13.0, ly + 8.0);
            let _ = cr.show_text(name);
        }
    }
}

fn draw_line(cr: &gtk::cairo::Context, w: i32, h: i32, gd: &GraphData) {
    let n_series = gd.values.len();
    let n_pts    = gd.values.iter().map(|s| s.len()).max().unwrap_or(0);
    if n_pts == 0 { return; }

    let margin_l = 42.0f64;
    let margin_b = if gd.labels.is_empty() { 10.0 } else { 28.0 };
    let margin_t = if gd.title.is_empty() { 10.0 } else { 26.0 };
    let margin_r = 10.0f64;
    let pw = w as f64 - margin_l - margin_r;
    let ph = h as f64 - margin_t - margin_b;

    let all: Vec<f64> = gd.values.iter().flat_map(|s| s.iter().copied()).collect();
    let min_val = all.iter().cloned().fold(f64::MAX, f64::min);
    let max_val = all.iter().cloned().fold(f64::MIN, f64::max);
    let range   = (max_val - min_val).max(1.0);

    grid_and_yaxis(cr, w as f64, margin_l, margin_r, margin_t, ph, h as f64, min_val, range);

    let pt = |i: usize, v: f64| -> (f64, f64) {
        let x = margin_l + if n_pts > 1 { i as f64 * pw / (n_pts - 1) as f64 } else { pw / 2.0 };
        let y = margin_t + ph * (1.0 - (v - min_val) / range);
        (x, y)
    };

    for s in 0..n_series {
        let series = &gd.values[s];
        if series.is_empty() { continue; }
        let (r, cg, b) = series_color(gd, s);

        // Fill under line
        cr.new_path();
        let (x0, y0) = pt(0, series[0]);
        cr.move_to(x0, margin_t + ph);
        cr.line_to(x0, y0);
        for i in 1..series.len() {
            let (xi, yi) = pt(i, series[i]);
            cr.line_to(xi, yi);
        }
        let (xn, _) = pt(series.len() - 1, series[series.len() - 1]);
        cr.line_to(xn, margin_t + ph);
        cr.close_path();
        cr.set_source_rgba(r, cg, b, 0.18);
        let _ = cr.fill();

        // Line
        cr.set_line_width(2.0);
        cr.set_source_rgb(r, cg, b);
        let (x0, y0) = pt(0, series[0]);
        cr.move_to(x0, y0);
        for i in 1..series.len() {
            let (xi, yi) = pt(i, series[i]);
            cr.line_to(xi, yi);
        }
        let _ = cr.stroke();

        // Dots
        for i in 0..series.len() {
            let (xi, yi) = pt(i, series[i]);
            cr.arc(xi, yi, 3.5, 0.0, std::f64::consts::TAU);
            cr.set_source_rgb(r, cg, b);
            let _ = cr.fill();
        }
    }

    // X labels
    set_dim(cr);
    cr.set_font_size(9.0);
    for i in 0..n_pts {
        if let Some(lbl) = gd.labels.get(i) {
            let (xi, _) = pt(i, 0.0);
            let (lw, _) = text_size(cr, lbl);
            cr.move_to(xi - lw / 2.0, h as f64 - margin_b + 12.0);
            let _ = cr.show_text(lbl);
        }
    }

    // Series legend
    if n_series > 1 {
        for s in 0..n_series {
            let (r, cg, b) = series_color(gd, s);
            let lx = margin_l + s as f64 * 80.0;
            let ly = margin_t - 6.0;
            cr.set_source_rgb(r, cg, b);
            cr.rectangle(lx, ly, 10.0, 8.0);
            let _ = cr.fill();
            set_dim(cr);
            cr.set_font_size(9.0);
            let name = gd.series_labels.get(s).map(|s| s.as_str()).unwrap_or("");
            cr.move_to(lx + 13.0, ly + 8.0);
            let _ = cr.show_text(name);
        }
    }
}

fn draw_pie(cr: &gtk::cairo::Context, w: i32, h: i32, gd: &GraphData) {
    let empty = Vec::new();
    let vals: &Vec<f64> = gd.values.first().unwrap_or(&empty);
    let n = vals.len();
    if n == 0 { return; }
    let total: f64 = vals.iter().sum();
    if total == 0.0 { return; }

    let title_h  = if gd.title.is_empty() { 4.0 } else { 28.0 };
    let legend_rows = (n + 1) / 2;
    let legend_h = legend_rows as f64 * 18.0 + 6.0;
    let pie_h    = (h as f64 - title_h - legend_h - 8.0).max(20.0);

    let cx     = w as f64 / 2.0;
    let cy     = title_h + pie_h / 2.0;
    let radius = (w.min(pie_h as i32) as f64 / 2.0 - 10.0).max(10.0);

    let mut angle = -std::f64::consts::FRAC_PI_2;
    for (i, &val) in vals.iter().enumerate() {
        let sweep = std::f64::consts::TAU * val / total;
        let (r, cg, b) = series_color(gd, i);

        cr.set_source_rgb(r, cg, b);
        cr.move_to(cx, cy);
        cr.arc(cx, cy, radius, angle, angle + sweep);
        cr.close_path();
        let _ = cr.fill();

        // Thin border
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.25);
        cr.set_line_width(1.0);
        cr.move_to(cx, cy);
        cr.arc(cx, cy, radius, angle, angle + sweep);
        cr.close_path();
        let _ = cr.stroke();

        // Percentage label inside slice (only if sweep big enough)
        if sweep > 0.2 {
            let mid = angle + sweep / 2.0;
            let lx  = cx + radius * 0.65 * mid.cos();
            let ly  = cy + radius * 0.65 * mid.sin();
            let pct = format!("{:.0}%", 100.0 * val / total);
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.set_font_size(10.0);
            let (pw, ph2) = text_size(cr, &pct);
            cr.move_to(lx - pw / 2.0, ly + ph2 / 2.0);
            let _ = cr.show_text(&pct);
        }

        angle += sweep;
    }

    // Legend (2 columns)
    let legend_y = title_h + pie_h + 8.0;
    let col_w = w as f64 / 2.0;
    for (i, &val) in vals.iter().enumerate() {
        let col = i % 2;
        let row = i / 2;
        let lx  = col as f64 * col_w + 10.0;
        let ly  = legend_y + row as f64 * 18.0;

        let (r, cg, b) = series_color(gd, i);
        cr.set_source_rgb(r, cg, b);
        cr.rectangle(lx, ly + 2.0, 10.0, 10.0);
        let _ = cr.fill();

        set_fg(cr);
        cr.set_font_size(10.0);
        let name = gd.labels.get(i).map(|s| s.as_str()).unwrap_or("");
        let pct  = 100.0 * val / total;
        let txt  = if name.is_empty() { format!("{:.1}%", pct) } else { format!("{} – {:.1}%", name, pct) };
        cr.move_to(lx + 14.0, ly + 11.0);
        let _ = cr.show_text(&txt);
    }
}

fn parse_graph_data(spec: &str) -> GraphData {
    let mut kind        = "bar".to_string();
    let mut all_values: Vec<Vec<f64>> = Vec::new();
    let mut labels      = Vec::new();
    let mut series_labels = Vec::new();
    let mut color_strs  = Vec::new();
    let mut title       = String::new();
    let mut bg          = String::new();

    for part in spec.split(';') {
        if let Some((k, v)) = part.split_once(':') {
            match k.trim() {
                "kind"   => kind = v.to_string(),
                "title"  => title = v.to_string(),
                "bg"     => bg = v.to_string(),
                "labels" => labels = v.split('|').map(|s| s.to_string()).collect(),
                "series" => series_labels = v.split('|').map(|s| s.to_string()).collect(),
                "colors" => color_strs = v.split('|').map(|s| s.to_string()).collect(),
                // data can be multi-series: "data:10|20|30,15|25|35"
                "data" => {
                    all_values = v.split(',')
                        .map(|s| s.split('|').filter_map(|n| n.parse().ok()).collect())
                        .collect();
                }
                _ => {}
            }
        }
    }
    if all_values.is_empty() {
        all_values.push(Vec::new());
    }
    GraphData { kind, values: all_values, labels, series_labels, color_strs, title, bg }
}

pub fn keyval_to_bytes(keyval: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> Option<Vec<u8>> {
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
            }
            if let Some(c) = keyval.to_unicode() {
                let mut buf = [0u8; 4];
                return Some(c.encode_utf8(&mut buf).as_bytes().to_vec());
            }
            None
        }
    }
}

pub struct TerminalState {
    pub primary_buffer: TextBuffer,
    pub alternate_buffer: TextBuffer,
    pub is_alternate: bool,
    pub current_tags: Vec<String>,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub alt_cursor_x: usize,
    pub alt_cursor_y: usize,
    pub mouse_tracking_mode: u32,
    pub view: glib::WeakRef<TextView>,
    pub tab_label: glib::WeakRef<Label>,
    pub scroll_top: usize,
    pub scroll_bottom: usize,
    pub saved_cursor_x: usize,
    pub saved_cursor_y: usize,
    pub saved_tags: Vec<String>,
    pub bracketed_paste_mode: bool,
    pub char_width: f32,
    pub char_height: f32,
    pub image_buffer: Vec<u8>,
    pub is_sixel: bool,
    pub cols: usize,
    pub rows: usize,
    /// Sender to push bytes back into the PTY (widget events go here).
    pub pty_input_tx: Option<flume::Sender<Vec<u8>>>,
    /// Named panels (containers) that widgets can be added to.
    pub panels: std::collections::HashMap<String, gtk::Box>,
    /// Named grid panels for row/col positioning.
    pub grids: std::collections::HashMap<String, gtk::Grid>,
    /// Radio button groups — first button per group name is the leader.
    pub radio_groups: std::collections::HashMap<String, gtk::CheckButton>,
    /// All created widgets by ID, for later updates via WidgetUpdate.
    pub widgets: std::collections::HashMap<String, gtk::Widget>,
    /// Menu popover inner boxes — items are appended here.
    pub menu_boxes: std::collections::HashMap<String, gtk::Box>,
    /// Menu popover widgets — for closing after item click.
    pub menu_popovers: std::collections::HashMap<String, gtk::Popover>,
    /// Map from menu ID to parent menu/menubar ID (for closing chain).
    pub menu_parents: std::collections::HashMap<String, String>,
    /// TreeStore models for treeview widgets (id -> store).
    pub tree_stores: std::collections::HashMap<String, gtk::TreeStore>,
    /// ListStore models for listview widgets (id -> store).
    pub list_stores: std::collections::HashMap<String, gtk::ListStore>,
    /// Named TreeIter rows: widget_id -> (row_id -> iter), for addrow with parent.
    pub tree_row_iters: std::collections::HashMap<String, std::collections::HashMap<String, gtk::TreeIter>>,
    /// Tracks the last dynamically-applied CSS class per widget so we can remove it before adding a new one.
    pub widget_css_classes: std::collections::HashMap<String, String>,
    /// Per-widget CssProvider for dynamic background/foreground colours.
    pub widget_css_providers: std::collections::HashMap<String, gtk::CssProvider>,
    /// Last-set background colour per widget (for merge on partial update).
    pub widget_bg_colors: std::collections::HashMap<String, String>,
    /// Last-set foreground/text colour per widget (for merge on partial update).
    pub widget_fg_colors: std::collections::HashMap<String, String>,
    /// Floating GTK windows created by Window= OSC command (id -> window).
    pub windows: std::collections::HashMap<String, gtk::Window>,
    /// Number of data columns per table/listview (excludes the 2 hidden fg/bg colour columns).
    pub table_data_cols: std::collections::HashMap<String, usize>,
    /// Live data backing each graph widget (id -> GraphData).  Shared with the draw closure.
    pub graph_data: std::collections::HashMap<String, std::rc::Rc<std::cell::RefCell<GraphData>>>,
    /// Per-widget font family override.
    pub widget_fonts: std::collections::HashMap<String, String>,
    /// Per-widget font size override (px).
    pub widget_sizes: std::collections::HashMap<String, u32>,
    /// FlowBox containers for flowbox panels (id -> FlowBox).
    pub flow_panels: std::collections::HashMap<String, gtk::FlowBox>,
}

impl TerminalState {
    pub fn new(view: glib::WeakRef<TextView>, tab_label: glib::WeakRef<Label>, palette: Vec<String>) -> Self {
        let primary_buffer = view.upgrade().unwrap().buffer();
        let tag_table = primary_buffer.tag_table();
        let alternate_buffer = TextBuffer::new(Some(&tag_table));

        // Create standard tags
        let bold = TextTag::new(Some("bold"));
        bold.set_weight(700); // Pango Bold
        tag_table.add(&bold);

        let italic = TextTag::new(Some("italic"));
        italic.set_style(gtk::pango::Style::Italic);
        tag_table.add(&italic);

        let underline = TextTag::new(Some("underline"));
        underline.set_underline(gtk::pango::Underline::Single);
        tag_table.add(&underline);

        let inverse = TextTag::new(Some("inverse"));
        // Inverse is tricky with CSS tags, but we'll try to swap colors if we knew them.
        // For now, just use a distinct highlight.
        inverse.set_background(Some("#555555"));
        tag_table.add(&inverse);

        let codes = [
            "30", "31", "32", "33", "34", "35", "36", "37",
            "90", "91", "92", "93", "94", "95", "96", "97",
        ];
        for (i, &code) in codes.iter().enumerate() {
            if let Some(color) = palette.get(i) {
                let tag = TextTag::new(Some(&format!("fg-{}", code)));
                tag.set_foreground(Some(color));
                tag_table.add(&tag);
                
                let bg_val = code.parse::<u32>().unwrap() + 10;
                let tag_bg = TextTag::new(Some(&format!("bg-{}", bg_val)));
                tag_bg.set_background(Some(color));
                tag_table.add(&tag_bg);
            }
        }

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
            scroll_top: 0,
            scroll_bottom: 23,
            saved_cursor_x: 0,
            saved_cursor_y: 0,
            saved_tags: Vec::new(),
            bracketed_paste_mode: false,
            char_width: 8.0,
            char_height: 16.0,
            image_buffer: Vec::new(),
            is_sixel: false,
            cols: 80,
            rows: 24,
            pty_input_tx: None,
            panels: std::collections::HashMap::new(),
            grids: std::collections::HashMap::new(),
            radio_groups: std::collections::HashMap::new(),
            widgets: std::collections::HashMap::new(),
            menu_boxes: std::collections::HashMap::new(),
            menu_popovers: std::collections::HashMap::new(),
            menu_parents: std::collections::HashMap::new(),
            tree_stores: std::collections::HashMap::new(),
            list_stores: std::collections::HashMap::new(),
            tree_row_iters: std::collections::HashMap::new(),
            widget_css_classes: std::collections::HashMap::new(),
            widget_css_providers: std::collections::HashMap::new(),
            widget_bg_colors: std::collections::HashMap::new(),
            widget_fg_colors: std::collections::HashMap::new(),
            windows: std::collections::HashMap::new(),
            table_data_cols: std::collections::HashMap::new(),
            graph_data: std::collections::HashMap::new(),
            widget_fonts: std::collections::HashMap::new(),
            widget_sizes: std::collections::HashMap::new(),
            flow_panels: std::collections::HashMap::new(),
        }
    }

    /// Apply background/foreground colours to a widget via a per-widget CssProvider.
    /// Merges new values with any already-stored value so a partial update (bg only,
    /// fg only) doesn't erase the other channel.
    fn apply_widget_colors(&mut self, id: &str, widget: &gtk::Widget, bg: Option<&str>, fg: Option<&str>) {
        if let Some(b) = bg { self.widget_bg_colors.insert(id.to_string(), b.to_string()); }
        if let Some(f) = fg { self.widget_fg_colors.insert(id.to_string(), f.to_string()); }
        self.rebuild_widget_css(id, widget);
    }

    fn apply_widget_font(&mut self, id: &str, widget: &gtk::Widget, font: Option<&str>, size: Option<u32>) {
        if let Some(f) = font { self.widget_fonts.insert(id.to_string(), f.to_string()); }
        if let Some(s) = size  { self.widget_sizes.insert(id.to_string(), s); }
        self.rebuild_widget_css(id, widget);
    }

    fn rebuild_widget_css(&mut self, id: &str, widget: &gtk::Widget) {
        let eff_bg   = self.widget_bg_colors.get(id).cloned();
        let eff_fg   = self.widget_fg_colors.get(id).cloned();
        let eff_font = self.widget_fonts.get(id).cloned();
        let eff_size = self.widget_sizes.get(id).copied();
        if eff_bg.is_none() && eff_fg.is_none() && eff_font.is_none() && eff_size.is_none() { return; }
        widget.set_widget_name(&format!("wgt-{}", id));
        let mut widget_props = String::new();
        let mut label_props  = String::new();
        if let Some(ref b) = eff_bg {
            widget_props.push_str(&format!("background-color:{};background-image:none;background:{};", b, b));
        }
        if let Some(ref f) = eff_fg {
            widget_props.push_str(&format!("color:{};", f));
            label_props.push_str(&format!("color:{};", f));
        }
        if let Some(ref font) = eff_font {
            let fs = format!("font-family:\"{}\";", font);
            widget_props.push_str(&fs);
            label_props.push_str(&fs);
        }
        if let Some(sz) = eff_size {
            let ss = format!("font-size:{}px;", sz);
            widget_props.push_str(&ss);
            label_props.push_str(&ss);
        }
        let mut css = format!("#wgt-{} {{ {} }}", id, widget_props);
        if !label_props.is_empty() {
            css.push_str(&format!(" #wgt-{} label {{ {} }}", id, label_props));
        }
        if let Some(provider) = self.widget_css_providers.get(id) {
            provider.load_from_data(&css);
        } else {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(&css);
            gtk::style_context_add_provider_for_display(
                &widget.display(),
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
            self.widget_css_providers.insert(id.to_string(), provider);
        }
    }


    /// Ensure buffer has at least `n+1` lines (lines 0..=n exist).
    fn ensure_lines(&self, n: usize) {
        let buffer = self.active_buffer();
        while (buffer.line_count() as usize) <= n {
            let mut end = buffer.end_iter();
            buffer.insert(&mut end, "\n");
        }
    }

    /// Scroll the scroll region [scroll_top..scroll_bottom] up by `count` lines.
    /// Deletes `count` lines from the top of the region, inserts `count` blank
    /// lines at the bottom.  Cursor position is NOT changed.
    fn scroll_region_up(&self, count: usize) {
        // Guarantee every line through scroll_bottom exists before we start
        // indexing.  If the buffer is short, iter_at_line() returns None and
        // the fallback to end_iter inserts the blank line at the wrong row,
        // shifting all subsequent row indices and pulling the status bar into
        // the scroll region.
        self.ensure_lines(self.scroll_bottom);
        let buffer = self.active_buffer();
        for _ in 0..count {
            // Delete line scroll_top (text + its trailing \n) so everything
            // above scroll_top shifts up by one.
            let mut line_start = buffer.iter_at_line(self.scroll_top as i32).unwrap();
            let mut line_end = line_start.clone();
            if line_end.forward_line() {
                buffer.delete(&mut line_start, &mut line_end);
            } else {
                // scroll_top is the very last line — clear its content.
                line_end.forward_to_line_end();
                buffer.delete(&mut line_start, &mut line_end);
                return;
            }
            // After the deletion every line from scroll_top+1 shifted up one.
            // The old scroll_bottom is now at (scroll_bottom-1).
            // Insert a blank line AFTER it to restore scroll_bottom.
            let mut ins = buffer.iter_at_line(self.scroll_bottom as i32 - 1).unwrap();
            ins.forward_to_line_end();
            buffer.insert(&mut ins, "\n");
        }
    }

    /// Scroll the scroll region [scroll_top..scroll_bottom] down by `count` lines.
    /// Inserts `count` blank lines at the top of the region, deletes `count` lines
    /// from the bottom.  Cursor position is NOT changed.
    fn scroll_region_down(&self, count: usize) {
        self.ensure_lines(self.scroll_bottom);
        let buffer = self.active_buffer();
        for _ in 0..count {
            // Insert blank line at scroll_top — everything in the region shifts down.
            let mut ins = buffer.iter_at_line(self.scroll_top as i32).unwrap();
            buffer.insert(&mut ins, "\n");
            // The old scroll_bottom is now at (scroll_bottom+1).  Delete it.
            let del_idx = self.scroll_bottom as i32 + 1;
            if let Some(mut del_start) = buffer.iter_at_line(del_idx) {
                let mut del_end = del_start.clone();
                if del_end.forward_line() {
                    buffer.delete(&mut del_start, &mut del_end);
                } else {
                    del_end.forward_to_line_end();
                    buffer.delete(&mut del_start, &mut del_end);
                }
            }
        }
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        let rows_changed = rows != self.rows;
        self.cols = cols;
        self.rows = rows;
        // Only reset the scroll region when the row count actually changes.
        // If rows is the same, an app like vi may have already sent CSI r to
        // define a restricted region (e.g. scroll_bottom = rows-2 so the status
        // bar is excluded). Wiping that on every identical resize call would
        // continuously stomp the custom region and drag the status line into the
        // scroll area.  When rows DO change, the PTY receives SIGWINCH and the
        // app will re-send CSI r, so resetting here is safe and necessary.
        if rows_changed {
            self.scroll_top = 0;
            self.scroll_bottom = rows.saturating_sub(1);
        }
    }

    pub fn active_buffer(&self) -> TextBuffer {
        if self.is_alternate {
            self.alternate_buffer.clone()
        } else {
            self.primary_buffer.clone()
        }
    }

    pub fn ensure_cursor_position(&self, cx: usize, cy: usize) -> gtk::TextIter {
        let buffer = self.active_buffer();
        while (buffer.line_count() as usize) <= cy {
            let mut end = buffer.end_iter();
            buffer.insert(&mut end, "\n");
        }
        
        let mut iter = buffer.iter_at_line(cy as i32).expect("Line must exist");
        iter.forward_to_line_end();
        let current_len = iter.line_offset() as usize;
        if cx > current_len {
            buffer.insert(&mut iter, &" ".repeat(cx - current_len));
        }
        buffer.iter_at_line_offset(cy as i32, cx as i32).unwrap_or_else(|| buffer.end_iter())
    }

    pub fn update_visual_cursor(&self) {
        if let Some(tv) = self.view.upgrade() {
            let buffer = tv.buffer();
            if buffer.selection_bounds().is_some() {
                return;
            }
            let (cx, cy) = if self.is_alternate { (self.alt_cursor_x, self.alt_cursor_y) } else { (self.cursor_x, self.cursor_y) };
            let mut iter = self.ensure_cursor_position(cx, cy);
            buffer.place_cursor(&iter);
            // use_align=false: scroll the minimum amount to make the cursor line
            // visible anywhere in the viewport.  This keeps vi's status bar (which
            // sits one line below scroll_bottom) in view — yalign=1.0 would pin the
            // cursor to the very bottom edge and push the status line off-screen.
            tv.scroll_to_iter(&mut iter, 0.0, false, 0.0, 0.0);
            // Always reset horizontal scroll to 0 — scroll_to_iter scrolls both axes
            // and can drift right when the cursor approaches cols.
            if let Some(hadj) = tv.hadjustment() {
                hadj.set_value(0.0);
            }
        }
    }



    pub fn tab(&mut self) {
        if self.is_alternate {
            self.alt_cursor_x = (self.alt_cursor_x / 8 + 1) * 8;
        } else {
            self.cursor_x = (self.cursor_x / 8 + 1) * 8;
        }
        self.update_visual_cursor();
    }

    pub fn update_palette(&mut self, palette: &[String]) {
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
    }

    pub fn apply_sgr(&mut self, params: &[i64]) {
        if params.contains(&0) { self.current_tags.clear(); }
        for &p in params {
            match p {
                1 => self.current_tags.push("bold".to_string()),
                3 => self.current_tags.push("italic".to_string()),
                4 => self.current_tags.push("underline".to_string()),
                7 => self.current_tags.push("inverse".to_string()),
                30..=37 | 90..=97 => self.current_tags.push(format!("fg-{}", p)),
                40..=47 | 100..=107 => self.current_tags.push(format!("bg-{}", p)),
                _ => {}
            }
        }
    }

    /// Parse semicolon-separated key:value pairs from a widget spec string.
    fn parse_widget_props(spec: &str) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        // Split on unescaped ';' and unescape values in one pass.
        // Supported escapes inside any value: \n \r \\ \;
        let mut parts: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut chars = spec.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                match chars.next() {
                    Some('n')  => cur.push('\n'),
                    Some('r')  => cur.push('\r'),
                    Some(';')  => cur.push(';'),
                    Some('\\') => cur.push('\\'),
                    Some(c)    => { cur.push('\\'); cur.push(c); }
                    None       => cur.push('\\'),
                }
            } else if ch == ';' {
                parts.push(std::mem::take(&mut cur));
            } else {
                cur.push(ch);
            }
        }
        parts.push(cur);
        for part in parts {
            if let Some((k, v)) = part.split_once(':') {
                map.insert(k.to_string(), v.to_string());
            }
        }
        map
    }

    /// Send a widget event response back to the PTY.
    #[allow(dead_code)]
    fn send_widget_event(&self, id: &str, action: &str, value: &str) {
        if let Some(ref tx) = self.pty_input_tx {
            // Format: ESC ] 1337 ; WidgetEvent=id:ID;action:ACTION;value:VALUE ST
            let msg = format!("\x1b]1337;WidgetEvent=id:{};action:{};value:{}\x07", id, action, value);
            let _ = tx.send(msg.into_bytes());
        }
    }

    /// Create a panel (container). If `panel` property is set, nest inside that parent;
    /// otherwise embed inline in the text buffer.
    /// `spec` is e.g. "id:panel1;layout:vertical;title:Settings;width:300;spacing:8"
    pub fn insert_panel(&mut self, spec: &str) {
        let props = Self::parse_widget_props(spec);
        let id = props.get("id").cloned().unwrap_or_else(|| "panel".into());
        let layout = props.get("layout").cloned().unwrap_or_else(|| "vertical".into());
        let title = props.get("title").cloned();
        let parent_id = props.get("panel").cloned();
        let width: i32 = props.get("width").and_then(|w| w.parse().ok()).unwrap_or(-1);
        let height: i32 = props.get("height").and_then(|h| h.parse().ok()).unwrap_or(-1);
        let spacing: i32 = props.get("spacing").and_then(|s| s.parse().ok()).unwrap_or(6);
        let margin: i32 = props.get("margin").and_then(|m| m.parse().ok()).unwrap_or(8);
        let expand: bool = props.get("expand").map(|v| v == "true" || v == "1").unwrap_or(false);

        // Build the inner container
        let (inner_widget, panel_box, grid_opt) = if layout == "grid" {
            let grid = gtk::Grid::new();
            grid.set_row_spacing(spacing as u32);
            grid.set_column_spacing(spacing as u32);
            grid.set_margin_start(margin);
            grid.set_margin_end(margin);
            grid.set_margin_top(margin);
            grid.set_margin_bottom(margin);
            if width > 0 || height > 0 { grid.set_size_request(width, height); }
            let outer: gtk::Widget = if let Some(ref t) = title {
                let frame = gtk::Frame::new(Some(t));
                frame.set_child(Some(&grid));
                frame.upcast()
            } else {
                grid.clone().upcast()
            };
            (outer, None, Some(grid))
        } else {
            let orientation = if layout == "horizontal" || layout == "hbox" {
                gtk::Orientation::Horizontal
            } else {
                gtk::Orientation::Vertical
            };
            let bx = gtk::Box::new(orientation, spacing);
            bx.set_margin_start(margin);
            bx.set_margin_end(margin);
            bx.set_margin_top(margin);
            bx.set_margin_bottom(margin);
            if width > 0 || height > 0 { bx.set_size_request(width, height); }
            let outer: gtk::Widget = if let Some(ref t) = title {
                let frame = gtk::Frame::new(Some(t));
                frame.set_child(Some(&bx));
                frame.upcast()
            } else {
                bx.clone().upcast()
            };
            (outer, Some(bx), None)
        };
        inner_widget.set_visible(true);
        // Apply per-panel colours
        let bg = props.get("bg_color").map(|s| s.as_str());
        let fg = props.get("fg_color").or(props.get("color")).map(|s| s.as_str());
        if bg.is_some() || fg.is_some() {
            self.apply_widget_colors(&id, &inner_widget, bg, fg);
        }
        if expand {
            inner_widget.set_hexpand(true);
            inner_widget.set_vexpand(true);
        }

        // Place: into parent panel, or inline in buffer
        if let Some(ref pid) = parent_id {
            let mut placed = false;
            if let Some(parent_grid) = self.grids.get(pid) {
                let row: i32 = props.get("row").and_then(|r| r.parse().ok()).unwrap_or(0);
                let col: i32 = props.get("col").and_then(|c| c.parse().ok()).unwrap_or(0);
                let colspan: i32 = props.get("colspan").and_then(|c| c.parse().ok()).unwrap_or(1);
                let rowspan: i32 = props.get("rowspan").and_then(|r| r.parse().ok()).unwrap_or(1);
                parent_grid.attach(&inner_widget, col, row, colspan, rowspan);
                placed = true;
            }
            if !placed {
                if let Some(parent_box) = self.panels.get(pid) {
                    parent_box.append(&inner_widget);
                    placed = true;
                }
            }
            if !placed {
                eprintln!("[PANEL] parent '{}' not found, inserting inline", pid);
            } else {
                // Register and return early
                if let Some(g) = grid_opt { self.grids.insert(id.clone(), g); }
                if let Some(b) = panel_box { self.panels.insert(id.clone(), b); }
                eprintln!("[PANEL] inserted panel id={} layout={} into parent={}", id, layout, pid);
                return;
            }
        }

        // Inline: embed in text buffer
        let tv = match self.view.upgrade() {
            Some(tv) => tv,
            None => return,
        };
        let buffer = self.active_buffer();
        let cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
        let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
        let mut iter = self.ensure_cursor_position(cx, cy);
        let anchor = buffer.create_child_anchor(&mut iter);
        tv.add_child_at_anchor(&inner_widget, &anchor);

        if let Some(g) = grid_opt { self.grids.insert(id.clone(), g); }
        if let Some(b) = panel_box { self.panels.insert(id.clone(), b); }
        if self.is_alternate { self.alt_cursor_x += 1; } else { self.cursor_x += 1; }
        eprintln!("[PANEL] inserted panel id={} layout={} inline", id, layout);
    }

    /// Read the current value of a widget and send it back as a WidgetEvent.
    /// Spec: "id:editor"  →  WidgetEvent=id:editor;action:value;value:<text>
    fn get_widget_value(&self, spec: &str) {
        let props = Self::parse_widget_props(spec);
        let id = match props.get("id") {
            Some(v) => v.clone(),
            None => return,
        };
        let value = if let Some(widget) = self.widgets.get(&id) {
            if let Some(tv) = widget.downcast_ref::<gtk::TextView>() {
                let buf = tv.buffer();
                let start = buf.start_iter();
                let end   = buf.end_iter();
                buf.text(&start, &end, false).to_string()
            } else if let Some(entry) = widget.downcast_ref::<gtk::Entry>() {
                entry.text().to_string()
            } else if let Some(lbl) = widget.downcast_ref::<gtk::Label>() {
                lbl.text().to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        // Escape so the single OSC sequence stays intact:
        // \ → \\, newline → \n, ; → \; (semicolons would break Python's event parser)
        let escaped = value.replace('\\', "\\\\").replace('\n', "\\n").replace(';', "\\;");
        if let Some(ref tx) = self.pty_input_tx {
            let msg = format!("\x1b]1337;WidgetEvent=id:{};action:value;value:{}\x07", id, escaped);
            let _ = tx.send(msg.into_bytes());
        }
    }

    /// Show a native file open/save dialog and send the result back as a WidgetEvent.
    /// Spec format (first segment is mode): "open;id:fd1;title:Open;filter:*.py"
    ///                                  or: "save;id:fd2;title:Save;default:out.py"
    fn show_file_dialog(&self, spec: &str) {
        let mode = spec.split(';').next().unwrap_or("open").trim().to_lowercase();
        let props = Self::parse_widget_props(spec);
        let id = props.get("id").cloned().unwrap_or_else(|| "fd".to_string());
        let title = props.get("title").cloned().unwrap_or_else(|| {
            if mode == "save" { "Save File".to_string() } else { "Open File".to_string() }
        });
        let default_name = props.get("default").cloned().unwrap_or_default();
        let filter_pattern = props.get("filter").cloned();

        let pty_tx = self.pty_input_tx.clone();
        let id_save = id.clone();
        let id_open = id.clone();

        let dialog = gtk::FileDialog::builder()
            .title(&title)
            .modal(true)
            .build();

        if let Some(pattern) = filter_pattern {
            let filter = gtk::FileFilter::new();
            filter.add_pattern(&pattern);
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            dialog.set_filters(Some(&filters));
        }

        if !default_name.is_empty() {
            dialog.set_initial_name(Some(&default_name));
        }

        if mode == "save" {
            dialog.save(None::<&gtk::Window>, None::<&gio::Cancellable>, move |result| {
                let msg = match result {
                    Ok(file) => {
                        let path = file.path()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        format!("\x1b]1337;WidgetEvent=id:{};action:file;value:{}\x07", id_save, path)
                    }
                    Err(_) => format!("\x1b]1337;WidgetEvent=id:{};action:cancel;value:\x07", id_save),
                };
                if let Some(ref tx) = pty_tx {
                    let _ = tx.send(msg.into_bytes());
                }
            });
        } else {
            dialog.open(None::<&gtk::Window>, None::<&gio::Cancellable>, move |result| {
                let msg = match result {
                    Ok(file) => {
                        let path = file.path()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        format!("\x1b]1337;WidgetEvent=id:{};action:file;value:{}\x07", id_open, path)
                    }
                    Err(_) => format!("\x1b]1337;WidgetEvent=id:{};action:cancel;value:\x07", id_open),
                };
                if let Some(ref tx) = pty_tx {
                    let _ = tx.send(msg.into_bytes());
                }
            });
        }
    }

    /// Build a widget from parsed properties and return it (without placing it).
    fn build_widget(&mut self, props: &std::collections::HashMap<String, String>) -> Option<gtk::Widget> {
        let widget_type = props.get("type")?.as_str();
        let id = props.get("id").cloned().unwrap_or_else(|| "unnamed".to_string());
        let pty_tx = self.pty_input_tx.clone();

        match widget_type {
            "button" => {
                let label_raw = props.get("label").cloned().unwrap_or_else(|| "Button".into());
                let label = label_raw.replace("\\n", "\n");
                let btn = gtk::Button::with_label(&label);
                let bw: i32 = props.get("width").and_then(|v| v.parse().ok()).unwrap_or(-1);
                let bh: i32 = props.get("height").and_then(|v| v.parse().ok()).unwrap_or(-1);
                if bw > 0 || bh > 0 { btn.set_size_request(bw, bh); }
                if let Some(cls) = props.get("css_class") {
                    btn.add_css_class(cls);
                    self.widget_css_classes.insert(id.clone(), cls.clone());
                    // Pre-apply per-widget provider for ttt-* classes so the
                    // GTK theme gradient cannot override the button background.
                    let wname = format!("wgt-{}", id);
                    btn.set_widget_name(&wname);
                    let ttt_css: Option<String> = match cls.as_str() {
                        "ttt-x"        => Some(format!("#{} {{ background-color:#3498db; background-image:none; }} #{} label {{ color:white; font-size:26px; font-weight:bold; }}", wname, wname)),
                        "ttt-o"        => Some(format!("#{} {{ background-color:#e74c3c; background-image:none; }} #{} label {{ color:white; font-size:26px; font-weight:bold; }}", wname, wname)),
                        "ttt-win"      => Some(format!("#{} {{ background-color:#2ecc71; background-image:none; }} #{} label {{ color:white; font-size:26px; font-weight:bold; }}", wname, wname)),
                        "ttt-empty"    => Some(format!("#{} {{ background-color:#44475a; background-image:none; }} #{} label {{ color:#aaa;   font-size:26px; font-weight:bold; }}", wname, wname)),
                        "card-hidden"  => Some(format!("#{} {{ background-color:#2c3e50; background-image:none; border-radius:8px; }} #{} label {{ color:#566573; font-size:20px; font-weight:bold; }}", wname, wname)),
                        "card-red"     => Some(format!("#{} {{ background-color:#c0392b; background-image:none; border-radius:8px; }} #{} label {{ color:white;   font-size:20px; font-weight:bold; }}", wname, wname)),
                        "card-black"   => Some(format!("#{} {{ background-color:#1a252f; background-image:none; border-radius:8px; }} #{} label {{ color:#3498db; font-size:20px; font-weight:bold; }}", wname, wname)),
                        "card-matched" => Some(format!("#{} {{ background-color:#1e8449; background-image:none; border-radius:8px; }} #{} label {{ color:white;   font-size:20px; font-weight:bold; }}", wname, wname)),
                        "card-wrong"   => Some(format!("#{} {{ background-color:#922b21; background-image:none; border-radius:8px; }} #{} label {{ color:white;   font-size:20px; font-weight:bold; }}", wname, wname)),
                        _ => None,
                    };
                    if let Some(css) = ttt_css {
                        let provider = gtk::CssProvider::new();
                        provider.load_from_data(&css);
                        gtk::style_context_add_provider_for_display(
                            &btn.display(),
                            &provider,
                            gtk::STYLE_PROVIDER_PRIORITY_USER,
                        );
                        self.widget_css_providers.insert(id.clone(), provider);
                    }
                }
                let wid = id.clone();
                let tx = pty_tx.clone();
                btn.connect_clicked(move |_| {
                    if let Some(ref tx) = tx {
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:clicked;value:\x07", wid);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(btn.upcast())
            }
            "entry" | "textbox" => {
                let placeholder = props.get("placeholder").cloned().unwrap_or_default();
                let width: i32 = props.get("width").and_then(|w| w.parse().ok()).unwrap_or(20);
                let entry = gtk::Entry::new();
                entry.set_placeholder_text(Some(&placeholder));
                entry.set_width_chars(width);
                let wid = id.clone();
                let tx = pty_tx.clone();
                entry.connect_activate(move |e| {
                    if let Some(ref tx) = tx {
                        let text = e.text().to_string();
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:submit;value:{}\x07", wid, text);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(entry.upcast())
            }
            "password" | "passwordentry" => {
                let placeholder = props.get("placeholder").cloned().unwrap_or_else(|| "Password".into());
                let pe = gtk::PasswordEntry::new();
                pe.set_placeholder_text(Some(&placeholder));
                pe.set_show_peek_icon(true);
                let wid = id.clone();
                let tx = pty_tx.clone();
                pe.connect_activate(move |e| {
                    if let Some(ref tx) = tx {
                        let text = e.text().to_string();
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:submit;value:{}\x07", wid, text);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(pe.upcast())
            }
            "checkbox" | "check" => {
                let label = props.get("label").cloned().unwrap_or_else(|| "Check".into());
                let checked: bool = props.get("checked").map(|v| v == "true" || v == "1").unwrap_or(false);
                let cb = gtk::CheckButton::with_label(&label);
                cb.set_active(checked);
                let wid = id.clone();
                let tx = pty_tx.clone();
                cb.connect_toggled(move |c| {
                    if let Some(ref tx) = tx {
                        let val = if c.is_active() { "true" } else { "false" };
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:toggled;value:{}\x07", wid, val);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(cb.upcast())
            }
            "radio" => {
                let label = props.get("label").cloned().unwrap_or_else(|| "Option".into());
                let group = props.get("group").cloned().unwrap_or_else(|| "default".into());
                let rb = gtk::CheckButton::with_label(&label);
                // Link to existing group leader so only one can be active
                if let Some(leader) = self.radio_groups.get(&group) {
                    rb.set_group(Some(leader));
                } else {
                    self.radio_groups.insert(group.clone(), rb.clone());
                }
                let wid = id.clone();
                let tx = pty_tx.clone();
                let grp = group.clone();
                rb.connect_toggled(move |c| {
                    if c.is_active() {
                        if let Some(ref tx) = tx {
                            let msg = format!("\x1b]1337;WidgetEvent=id:{};action:selected;value:true;group:{}\x07", wid, grp);
                            let _ = tx.send(msg.into_bytes());
                        }
                    }
                });
                Some(rb.upcast())
            }
            "dropdown" | "combo" | "select" => {
                let items_str = props.get("items").cloned().unwrap_or_default();
                let items: Vec<&str> = items_str.split('|').collect();
                let selected: u32 = props.get("selected").and_then(|v| v.parse().ok()).unwrap_or(0);
                let combo = gtk::ComboBoxText::new();
                for item in &items {
                    combo.append_text(item);
                }
                combo.set_active(Some(selected));
                let wid = id.clone();
                let tx = pty_tx.clone();
                combo.connect_changed(move |c| {
                    if let Some(ref tx) = tx {
                        let val = c.active_text().map(|s| s.to_string()).unwrap_or_default();
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:selected;value:{}\x07", wid, val);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(combo.upcast())
            }
            "slider" | "scale" => {
                let min: f64 = props.get("min").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let max: f64 = props.get("max").and_then(|v| v.parse().ok()).unwrap_or(100.0);
                let value: f64 = props.get("value").and_then(|v| v.parse().ok()).unwrap_or(min);
                let width: i32 = props.get("width").and_then(|w| w.parse().ok()).unwrap_or(200);
                let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, min, max, 1.0);
                scale.set_value(value);
                scale.set_size_request(width, -1);
                scale.set_draw_value(true);
                let wid = id.clone();
                let tx = pty_tx.clone();
                scale.connect_value_changed(move |s| {
                    if let Some(ref tx) = tx {
                        let val = s.value();
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:changed;value:{:.0}\x07", wid, val);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(scale.upcast())
            }
            "switch" | "toggle" => {
                let active: bool = props.get("active").map(|v| v == "true" || v == "1").unwrap_or(false);
                let sw = gtk::Switch::new();
                sw.set_active(active);
                let wid = id.clone();
                let tx = pty_tx.clone();
                sw.connect_state_set(move |_, state| {
                    if let Some(ref tx) = tx {
                        let val = if state { "true" } else { "false" };
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:toggled;value:{}\x07", wid, val);
                        let _ = tx.send(msg.into_bytes());
                    }
                    glib::Propagation::Proceed
                });
                Some(sw.upcast())
            }
            "progress" | "progressbar" => {
                let fraction: f64 = props.get("value").and_then(|v| v.parse().ok()).unwrap_or(0.0) / 100.0;
                let width: i32 = props.get("width").and_then(|w| w.parse().ok()).unwrap_or(200);
                let text = props.get("text").cloned();
                let pb = gtk::ProgressBar::new();
                pb.set_fraction(fraction.clamp(0.0, 1.0));
                pb.set_size_request(width, -1);
                if let Some(t) = text {
                    pb.set_text(Some(&t));
                    pb.set_show_text(true);
                }
                Some(pb.upcast())
            }
            "graph" | "chart" => {
                let width: i32  = props.get("width").and_then(|v| v.parse().ok()).unwrap_or(320);
                let height: i32 = props.get("height").and_then(|v| v.parse().ok()).unwrap_or(220);
                // Rebuild a spec string from all remaining props so parse_graph_data can handle them
                let spec = props.iter()
                    .map(|(k, v)| format!("{}:{}", k, v))
                    .collect::<Vec<_>>()
                    .join(";");
                let gd = std::rc::Rc::new(std::cell::RefCell::new(parse_graph_data(&spec)));
                let da = gtk::DrawingArea::new();
                da.set_content_width(width);
                da.set_content_height(height);
                da.set_size_request(width, height);
                let gd_draw = gd.clone();
                da.set_draw_func(move |_, cr, w, h| {
                    draw_graph(cr, w, h, &gd_draw.borrow());
                });
                self.graph_data.insert(id.clone(), gd);
                Some(da.upcast())
            }
            "badge" => {
                let text = props.get("text").or(props.get("label")).cloned().unwrap_or_default();
                let lbl = gtk::Label::new(Some(&text));
                lbl.add_css_class("badge");
                // Support inline colour override via color: and fg: props
                if props.contains_key("color") || props.contains_key("fg") {
                    let bg = props.get("color").or(props.get("bg")).cloned();
                    let fg = props.get("fg").cloned();
                    self.apply_widget_colors(&id, &lbl.clone().upcast(), bg.as_deref(), fg.as_deref());
                }
                Some(lbl.upcast())
            }
            "spinner" => {
                let sp = gtk::Spinner::new();
                let spinning = props.get("spinning").map(|v| v != "false" && v != "0").unwrap_or(true);
                if spinning { sp.start(); }
                let sz: i32 = props.get("size").and_then(|v| v.parse().ok()).unwrap_or(32);
                sp.set_size_request(sz, sz);
                Some(sp.upcast())
            }
            "colorpicker" | "colorbutton" => {
                let dialog = gtk::ColorDialog::new();
                dialog.set_with_alpha(
                    props.get("alpha").map(|v| v == "true" || v == "1").unwrap_or(false)
                );
                let btn = gtk::ColorDialogButton::new(Some(dialog));
                if let Some(hex) = props.get("value") {
                    if let Ok(rgba) = hex.parse::<gtk::gdk::RGBA>() {
                        btn.set_rgba(&rgba);
                    }
                }
                let wid = id.clone();
                let tx = pty_tx.clone();
                btn.connect_rgba_notify(move |b| {
                    let rgba = b.rgba();
                    let hex = format!("#{:02x}{:02x}{:02x}",
                        (rgba.red()   * 255.0) as u8,
                        (rgba.green() * 255.0) as u8,
                        (rgba.blue()  * 255.0) as u8);
                    if let Some(ref tx) = tx {
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:changed;value:{}\x07", wid, hex);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(btn.upcast())
            }
            "flowbox" | "flowpanel" => {
                let width: i32  = props.get("width").and_then(|v| v.parse().ok()).unwrap_or(-1);
                let height: i32 = props.get("height").and_then(|v| v.parse().ok()).unwrap_or(200);
                let col_min: u32 = props.get("min_cols").and_then(|v| v.parse().ok()).unwrap_or(1);
                let col_max: u32 = props.get("max_cols").and_then(|v| v.parse().ok()).unwrap_or(10);
                let row_sp: u32 = props.get("row_spacing").and_then(|v| v.parse().ok()).unwrap_or(6);
                let col_sp: u32 = props.get("col_spacing").and_then(|v| v.parse().ok()).unwrap_or(6);
                let fb = gtk::FlowBox::new();
                fb.set_min_children_per_line(col_min);
                fb.set_max_children_per_line(col_max);
                fb.set_row_spacing(row_sp);
                fb.set_column_spacing(col_sp);
                fb.set_selection_mode(gtk::SelectionMode::None);
                fb.set_hexpand(true);
                let sw = gtk::ScrolledWindow::new();
                sw.set_child(Some(&fb));
                sw.set_size_request(width, height);
                sw.set_hexpand(true);
                sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
                // Register FlowBox in its own map so insert_widget can route children to it
                self.flow_panels.insert(id.clone(), fb);
                Some(sw.upcast())
            }
            "scrollarea" => {
                let width: i32  = props.get("width").and_then(|v| v.parse().ok()).unwrap_or(-1);
                let height: i32 = props.get("height").and_then(|v| v.parse().ok()).unwrap_or(200);
                let hpol = match props.get("hscroll").map(|s| s.as_str()) {
                    Some("never")    => gtk::PolicyType::Never,
                    Some("always")   => gtk::PolicyType::Always,
                    _                => gtk::PolicyType::Automatic,
                };
                let vpol = match props.get("vscroll").map(|s| s.as_str()) {
                    Some("never")    => gtk::PolicyType::Never,
                    Some("always")   => gtk::PolicyType::Always,
                    _                => gtk::PolicyType::Automatic,
                };
                let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
                inner.set_hexpand(true);
                let sw = gtk::ScrolledWindow::new();
                sw.set_child(Some(&inner));
                sw.set_size_request(width, height);
                sw.set_hexpand(true);
                sw.set_vexpand(height < 0);
                sw.set_policy(hpol, vpol);
                self.panels.insert(id.clone(), inner);
                Some(sw.upcast())
            }
            "label" => {
                let text = props.get("label").or(props.get("text")).cloned().unwrap_or_default();
                let lbl = gtk::Label::new(Some(&text));
                if let Some(css_class) = props.get("class") {
                    lbl.add_css_class(css_class);
                }
                Some(lbl.upcast())
            }
            "spinbutton" | "spin" | "spinbox" => {
                let min: f64 = props.get("min").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let max: f64 = props.get("max").and_then(|v| v.parse().ok()).unwrap_or(100.0);
                let value: f64 = props.get("value").and_then(|v| v.parse().ok()).unwrap_or(min);
                let step: f64 = props.get("step").and_then(|v| v.parse().ok()).unwrap_or(1.0);
                let adj = gtk::Adjustment::new(value, min, max, step, step * 10.0, 0.0);
                let spin = gtk::SpinButton::new(Some(&adj), step, 0);
                let wid = id.clone();
                let tx = pty_tx.clone();
                spin.connect_value_changed(move |s| {
                    if let Some(ref tx) = tx {
                        let val = s.value();
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:changed;value:{:.0}\x07", wid, val);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(spin.upcast())
            }
            "separator" | "sep" => {
                let orient = if props.get("orient").map(|o| o == "vertical" || o == "v").unwrap_or(false) {
                    gtk::Orientation::Vertical
                } else {
                    gtk::Orientation::Horizontal
                };
                let sep = gtk::Separator::new(orient);
                Some(sep.upcast())
            }
            "calendar" | "date" | "datepicker" => {
                let cal = gtk::Calendar::new();
                let wid = id.clone();
                let tx = pty_tx.clone();
                cal.connect_day_selected(move |c| {
                    if let Some(ref tx) = tx {
                        let dt = c.date();
                        let val = format!("{:04}-{:02}-{:02}", dt.year(), dt.month(), dt.day_of_month());
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:selected;value:{}\x07", wid, val);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(cal.upcast())
            }
            "color" | "colorbutton" | "colorpicker" => {
                let btn = gtk::ColorButton::new();
                if let Some(c) = props.get("value") {
                    let rgba = gtk::gdk::RGBA::parse(c).unwrap_or(gtk::gdk::RGBA::BLACK);
                    btn.set_rgba(&rgba);
                }
                btn.set_use_alpha(props.get("alpha").map(|v| v == "true" || v == "1").unwrap_or(false));
                if let Some(t) = props.get("title") {
                    btn.set_title(t);
                }
                let wid = id.clone();
                let tx = pty_tx.clone();
                btn.connect_color_set(move |b| {
                    if let Some(ref tx) = tx {
                        let c = b.rgba();
                        let hex = format!("#{:02x}{:02x}{:02x}",
                            (c.red() * 255.0) as u8,
                            (c.green() * 255.0) as u8,
                            (c.blue() * 255.0) as u8);
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:selected;value:{}\x07", wid, hex);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(btn.upcast())
            }
            "link" | "linkbutton" => {
                let uri = props.get("uri").cloned().unwrap_or_else(|| "https://example.com".into());
                let label = props.get("label").cloned().unwrap_or_else(|| uri.clone());
                let lb = gtk::LinkButton::with_label(&uri, &label);
                Some(lb.upcast())
            }
            "togglebutton" => {
                let label = props.get("label").cloned().unwrap_or_else(|| "Toggle".into());
                let active: bool = props.get("active").map(|v| v == "true" || v == "1").unwrap_or(false);
                let tb = gtk::ToggleButton::with_label(&label);
                tb.set_active(active);
                let wid = id.clone();
                let tx = pty_tx.clone();
                tb.connect_toggled(move |b| {
                    if let Some(ref tx) = tx {
                        let val = if b.is_active() { "true" } else { "false" };
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:toggled;value:{}\x07", wid, val);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(tb.upcast())
            }
            "levelbar" | "level" => {
                let min: f64 = props.get("min").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let max: f64 = props.get("max").and_then(|v| v.parse().ok()).unwrap_or(100.0);
                let value: f64 = props.get("value").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let width: i32 = props.get("width").and_then(|w| w.parse().ok()).unwrap_or(200);
                let lb = gtk::LevelBar::for_interval(min, max);
                lb.set_value(value);
                lb.set_size_request(width, -1);
                Some(lb.upcast())
            }
            "image" | "picture" => {
                // Inline image from a file path
                let path = props.get("path").cloned().unwrap_or_default();
                let w: i32 = props.get("width").and_then(|v| v.parse().ok()).unwrap_or(-1);
                let h: i32 = props.get("height").and_then(|v| v.parse().ok()).unwrap_or(-1);
                let img = gtk::Image::from_file(&path);
                if w > 0 || h > 0 { img.set_size_request(w, h); }
                Some(img.upcast())
            }
            "picturebox" | "img" => {
                // Updatable picture widget — supports setting image via path or base64 data.
                // Use WidgetUpdate with path: or data: (base64) to change the image.
                let w: i32 = props.get("width").and_then(|v| v.parse().ok()).unwrap_or(200);
                let h: i32 = props.get("height").and_then(|v| v.parse().ok()).unwrap_or(200);
                let pic = gtk::Picture::new();
                pic.set_can_shrink(true);
                pic.set_size_request(w, h);
                // If path provided at creation time, load it
                if let Some(path) = props.get("path") {
                    let file = gtk::gio::File::for_path(path);
                    pic.set_file(Some(&file));
                }
                // If base64 data provided at creation time, decode and set
                if let Some(b64) = props.get("data") {
                    if let Ok(bytes) = BASE64.decode(b64.as_bytes()) {
                        let pixbuf_loader = gtk::gdk_pixbuf::PixbufLoader::new();
                        let _ = pixbuf_loader.write(&bytes);
                        let _ = pixbuf_loader.close();
                        if let Some(pixbuf) = pixbuf_loader.pixbuf() {
                            let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
                            pic.set_paintable(Some(&texture));
                        }
                    }
                }
                // Store the Picture itself for updates
                self.widgets.insert(id.clone(), pic.clone().upcast());
                Some(pic.upcast())
            }
            "treeview" => {
                let width: i32 = props.get("width").and_then(|v| v.parse().ok()).unwrap_or(200);
                let height: i32 = props.get("height").and_then(|v| v.parse().ok()).unwrap_or(300);
                let store = gtk::TreeStore::new(&[glib::Type::STRING]);
                let tv = gtk::TreeView::with_model(&store);
                tv.set_headers_visible(false);
                tv.set_activate_on_single_click(true);
                let renderer = gtk::CellRendererText::new();
                let col = gtk::TreeViewColumn::new();
                gtk::prelude::CellLayoutExt::pack_start(&col, &renderer, true);
                gtk::prelude::CellLayoutExt::add_attribute(&col, &renderer, "text", 0);
                tv.append_column(&col);
                let wid = id.clone();
                let tx = pty_tx.clone();
                tv.selection().connect_changed(move |sel| {
                    if let Some((model, iter)) = sel.selected() {
                        if let Ok(val) = model.get_value(&iter, 0).get::<String>() {
                            if let Some(ref tx) = tx {
                                let msg = format!("\x1b]1337;WidgetEvent=id:{};action:selected;value:{}\x07", wid, val);
                                let _ = tx.send(msg.into_bytes());
                            }
                        }
                    }
                });
                {
                    let wid2 = id.clone();
                    let tx2 = pty_tx.clone();
                    tv.connect_row_expanded(move |tv_inner, iter, _path| {
                        if let Some(model) = tv_inner.model() {
                            if let Ok(val) = model.get_value(iter, 0).get::<String>() {
                                if let Some(ref tx) = tx2 {
                                    let msg = format!("\x1b]1337;WidgetEvent=id:{};action:expanded;value:{}\x07", wid2, val);
                                    let _ = tx.send(msg.into_bytes());
                                }
                            }
                        }
                    });
                }
                {
                    let wid3 = id.clone();
                    let tx3 = pty_tx.clone();
                    tv.connect_row_collapsed(move |tv_inner, iter, _path| {
                        if let Some(model) = tv_inner.model() {
                            if let Ok(val) = model.get_value(iter, 0).get::<String>() {
                                if let Some(ref tx) = tx3 {
                                    let msg = format!("\x1b]1337;WidgetEvent=id:{};action:collapsed;value:{}\x07", wid3, val);
                                    let _ = tx.send(msg.into_bytes());
                                }
                            }
                        }
                    });
                }
                let sw = gtk::ScrolledWindow::new();
                sw.set_child(Some(&tv));
                sw.set_size_request(width, height);
                sw.set_vexpand(true);
                sw.set_hexpand(true);
                sw.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
                self.tree_stores.insert(id.clone(), store);
                self.widgets.insert(id.clone(), tv.clone().upcast());
                Some(sw.upcast())
            }
            "listview" | "listbox" | "table" => {
                let width: i32 = props.get("width").and_then(|v| v.parse().ok()).unwrap_or(300);
                let height: i32 = props.get("height").and_then(|v| v.parse().ok()).unwrap_or(300);
                let col_names: Vec<String> = props.get("cols")
                    .map(|s| s.split('|').map(|c| c.to_string()).collect())
                    .unwrap_or_else(|| vec!["Column".to_string()]);
                let col_widths: Vec<i32> = props.get("widths")
                    .map(|s| s.split('|').filter_map(|w| w.parse().ok()).collect())
                    .unwrap_or_default();
                let n = col_names.len();
                // n+2 cols: n data + 1 fg colour + 1 bg colour (hidden, bound to renderers)
                let types: Vec<glib::Type> = vec![glib::Type::STRING; n + 2];
                let store = gtk::ListStore::new(&types);
                let tv = gtk::TreeView::with_model(&store);
                tv.set_headers_visible(true);
                tv.set_activate_on_single_click(true);
                let sortable = props.get("sortable").map(|v| v != "false" && v != "0").unwrap_or(true);
                // Per-column ascending/descending toggle state
                let sort_dirs = std::rc::Rc::new(std::cell::RefCell::new(vec![true; n]));
                for (i, name) in col_names.iter().enumerate() {
                    let renderer = gtk::CellRendererText::new();
                    let col = gtk::TreeViewColumn::new();
                    col.set_title(name);
                    gtk::prelude::CellLayoutExt::pack_start(&col, &renderer, true);
                    gtk::prelude::CellLayoutExt::add_attribute(&col, &renderer, "text", i as i32);
                    // Bind hidden fg/bg colour columns to every renderer
                    gtk::prelude::CellLayoutExt::add_attribute(&col, &renderer, "foreground", n as i32);
                    gtk::prelude::CellLayoutExt::add_attribute(&col, &renderer, "background", (n + 1) as i32);
                    col.set_resizable(true);
                    col.set_expand(i == 0);
                    if let Some(&cw) = col_widths.get(i) {
                        if cw > 0 {
                            col.set_sizing(gtk::TreeViewColumnSizing::Fixed);
                            col.set_fixed_width(cw);
                        }
                    }
                    // Column-header click → sort event
                    if sortable {
                        let col_name = name.clone();
                        let wid_sort = id.clone();
                        let tx_sort  = pty_tx.clone();
                        let dirs     = sort_dirs.clone();
                        let col_idx  = i;
                        col.set_clickable(true);
                        col.connect_clicked(move |c| {
                            let mut d = dirs.borrow_mut();
                            let asc = d[col_idx];
                            d[col_idx] = !asc;
                            let dir_str = if asc { "asc" } else { "desc" };
                            c.set_sort_indicator(true);
                            c.set_sort_order(if asc {
                                gtk::SortType::Ascending
                            } else {
                                gtk::SortType::Descending
                            });
                            if let Some(ref tx) = tx_sort {
                                let msg = format!(
                                    "\x1b]1337;WidgetEvent=id:{};action:sort;col:{};dir:{}\x07",
                                    wid_sort, col_name, dir_str
                                );
                                let _ = tx.send(msg.into_bytes());
                            }
                        });
                    }
                    tv.append_column(&col);
                }
                self.table_data_cols.insert(id.clone(), n);
                // Single click = selection changed → action:selected
                tv.set_activate_on_single_click(false);
                let wid = id.clone();
                let tx = pty_tx.clone();
                let ncols = n;
                tv.selection().connect_changed(move |sel| {
                    if let Some((model, iter)) = sel.selected() {
                        let vals: Vec<String> = (0..ncols)
                            .filter_map(|i| model.get_value(&iter, i as i32).get::<String>().ok())
                            .collect();
                        if let Some(ref tx) = tx {
                            let msg = format!("\x1b]1337;WidgetEvent=id:{};action:selected;value:{}\x07", wid, vals.join("|"));
                            let _ = tx.send(msg.into_bytes());
                        }
                    }
                });
                // Double click = row activated → action:activated
                let wid2 = id.clone();
                let tx2 = pty_tx.clone();
                let ncols2 = n;
                tv.connect_row_activated(move |tv, path, _col| {
                    if let Some(model) = tv.model() {
                        if let Some(iter) = model.iter(path) {
                            let vals: Vec<String> = (0..ncols2)
                                .filter_map(|i| model.get_value(&iter, i as i32).get::<String>().ok())
                                .collect();
                            if let Some(ref tx) = tx2 {
                                let msg = format!("\x1b]1337;WidgetEvent=id:{};action:activated;value:{}\x07", wid2, vals.join("|"));
                                let _ = tx.send(msg.into_bytes());
                            }
                        }
                    }
                });
                // Row action columns — inline click targets, not backed by the model store.
                // Each renderer has a fixed label set directly; no add_attribute binding is
                // made so GTK never overwrites the text from the model.
                let action_names: Vec<String> = props.get("actions")
                    .map(|s| s.split('|').map(|a| a.trim().to_string()).collect())
                    .unwrap_or_default();
                let n_data = n;
                for act_name in &action_names {
                    let renderer = gtk::CellRendererText::new();
                    renderer.set_property("text", act_name.as_str());
                    renderer.set_property("foreground", "#5599ff");
                    let col = gtk::TreeViewColumn::new();
                    gtk::prelude::CellLayoutExt::pack_start(&col, &renderer, false);
                    col.set_expand(false);
                    tv.append_column(&col);
                }
                if !action_names.is_empty() {
                    let tv_click = tv.clone();
                    let wid_click = id.clone();
                    let tx_click = pty_tx.clone();
                    let actions_click = action_names.clone();
                    let gesture = gtk::GestureClick::new();
                    gesture.connect_pressed(move |_, _, x, y| {
                        if let Some((Some(path), Some(clicked_col), _, _)) =
                            tv_click.path_at_pos(x as i32, y as i32)
                        {
                            let all_cols = tv_click.columns();
                            if let Some(col_idx) = all_cols.iter().position(|c| c == &clicked_col) {
                                if col_idx >= n_data {
                                    let act = &actions_click[col_idx - n_data];
                                    let row_idx = path.indices().first().cloned().unwrap_or(0);
                                    if let Some(model) = tv_click.model() {
                                        if let Some(iter) = model.iter(&path) {
                                            let rowid = model.get_value(&iter, 0)
                                                .get::<String>().unwrap_or_default();
                                            if let Some(ref tx) = tx_click {
                                                let msg = format!(
                                                    "\x1b]1337;WidgetEvent=id:{};action:row_action;value:{};rowid:{};row:{}\x07",
                                                    wid_click, act, rowid, row_idx
                                                );
                                                let _ = tx.send(msg.into_bytes());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });
                    tv.add_controller(gesture);
                }
                let sw = gtk::ScrolledWindow::new();
                sw.set_child(Some(&tv));
                sw.set_size_request(width, height);
                sw.set_vexpand(true);
                sw.set_hexpand(true);
                sw.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
                self.list_stores.insert(id.clone(), store);
                self.widgets.insert(id.clone(), tv.clone().upcast());
                Some(sw.upcast())
            }
            "splitview" | "paned" => {
                let orientation = match props.get("layout").map(|s| s.as_str()) {
                    Some("vertical") => gtk::Orientation::Vertical,
                    _ => gtk::Orientation::Horizontal,
                };
                let pos: i32 = props.get("pos").and_then(|p| p.parse().ok()).unwrap_or(250);
                let width: i32 = props.get("width").and_then(|v| v.parse().ok()).unwrap_or(-1);
                let height: i32 = props.get("height").and_then(|v| v.parse().ok()).unwrap_or(-1);
                let paned = gtk::Paned::new(orientation);
                paned.set_hexpand(true);
                paned.set_vexpand(true);
                // Default height so handle is actually visible and draggable
                let h = if height > 0 { height } else { 400 };
                if width > 0 { paned.set_size_request(width, h); } else { paned.set_size_request(-1, h); }
                paned.set_position(pos);
                // resize_start/end_child=true means dragging the handle actually
                // redistributes space between both sides instead of just moving
                // the divider position.
                paned.set_resize_start_child(true);
                paned.set_resize_end_child(true);
                // Minimum sizes prevent the pane from shrinking below a usable
                // width; without these GTK shifts the divider off-screen instead
                // of stopping the resize at the window edge.
                let min_start: i32 = props.get("min_start").and_then(|v| v.parse().ok()).unwrap_or(80);
                let min_end:   i32 = props.get("min_end").and_then(|v| v.parse().ok()).unwrap_or(120);
                let (ms_w, ms_h, me_w, me_h) = if orientation == gtk::Orientation::Horizontal {
                    (min_start, -1, min_end, -1)
                } else {
                    (-1, min_start, -1, min_end)
                };
                let start = gtk::Box::new(gtk::Orientation::Vertical, 0);
                let end = gtk::Box::new(gtk::Orientation::Vertical, 0);
                start.set_size_request(ms_w, ms_h);
                end.set_size_request(me_w, me_h);
                start.set_hexpand(true); start.set_vexpand(true);
                end.set_hexpand(true);   end.set_vexpand(true);
                paned.set_start_child(Some(&start));
                paned.set_end_child(Some(&end));
                // set_shrink=false enforces the size_request minimums during
                // drag; without this GTK ignores the child minimum and lets the
                // handle travel all the way to both edges.
                paned.set_shrink_start_child(false);
                paned.set_shrink_end_child(false);
                self.panels.insert(format!("{}-start", id), start);
                self.panels.insert(format!("{}-end", id), end);
                Some(paned.upcast())
            }
            "expander" => {
                let label = props.get("label").cloned().unwrap_or_else(|| "Details".into());
                let expanded: bool = props.get("expanded").map(|v| v == "true" || v == "1").unwrap_or(false);
                let exp = gtk::Expander::new(Some(&label));
                exp.set_expanded(expanded);
                // The expander is a container — register it as a panel so children use panel:<id>
                let inner = gtk::Box::new(gtk::Orientation::Vertical, 4);
                exp.set_child(Some(&inner));
                self.panels.insert(id.clone(), inner);
                Some(exp.upcast())
            }
            "frame" => {
                let label = props.get("label").cloned().unwrap_or_default();
                let spacing: i32 = props.get("spacing").and_then(|s| s.parse().ok()).unwrap_or(6);
                let margin: i32  = props.get("margin").and_then(|m| m.parse().ok()).unwrap_or(6);
                let width: i32   = props.get("width").and_then(|v| v.parse().ok()).unwrap_or(-1);
                let height: i32  = props.get("height").and_then(|v| v.parse().ok()).unwrap_or(-1);
                let frame = if label.is_empty() { gtk::Frame::new(None) }
                            else { gtk::Frame::new(Some(&label)) };
                if width > 0 || height > 0 { frame.set_size_request(width, height); }
                let inner = gtk::Box::new(gtk::Orientation::Vertical, spacing);
                inner.set_margin_start(margin); inner.set_margin_end(margin);
                inner.set_margin_top(margin);   inner.set_margin_bottom(margin);
                frame.set_child(Some(&inner));
                self.panels.insert(id.clone(), inner);
                Some(frame.upcast())
            }
            "textview" | "textarea" => {
                let text = props.get("text").cloned().unwrap_or_default();
                let width: i32 = props.get("width").and_then(|w| w.parse().ok()).unwrap_or(300);
                let height: i32 = props.get("height").and_then(|h| h.parse().ok()).unwrap_or(200);
                let editable: bool = props.get("editable").map(|v| v == "true" || v == "1").unwrap_or(false);
                let wrap = props.get("wrap").map(|w| w.as_str()).unwrap_or("word");
                let tv = gtk::TextView::new();
                tv.set_editable(editable);
                tv.set_cursor_visible(editable);
                tv.buffer().set_text(&text);
                tv.set_wrap_mode(match wrap {
                    "none" => gtk::WrapMode::None,
                    "char" => gtk::WrapMode::Char,
                    "wordchar" => gtk::WrapMode::WordChar,
                    _ => gtk::WrapMode::Word,
                });
                tv.set_left_margin(4);
                tv.set_right_margin(4);
                tv.set_top_margin(4);
                tv.set_bottom_margin(4);
                // Wrap in a scrolled window
                let sw = gtk::ScrolledWindow::new();
                sw.set_child(Some(&tv));
                sw.set_size_request(width, height);
                sw.set_vexpand(true);
                sw.set_hexpand(true);
                // Store the inner TextView as the widget so we can update its buffer
                self.widgets.insert(id.clone(), tv.clone().upcast());
                Some(sw.upcast())
            }
            "notebook" | "tabs" => {
                let width: i32 = props.get("width").and_then(|w| w.parse().ok()).unwrap_or(-1);
                let height: i32 = props.get("height").and_then(|h| h.parse().ok()).unwrap_or(-1);
                let pos = match props.get("tabpos").map(|s| s.as_str()) {
                    Some("bottom") => gtk::PositionType::Bottom,
                    Some("left") => gtk::PositionType::Left,
                    Some("right") => gtk::PositionType::Right,
                    _ => gtk::PositionType::Top,
                };
                let nb = gtk::Notebook::new();
                nb.set_tab_pos(pos);
                nb.set_scrollable(true);
                if width > 0 || height > 0 { nb.set_size_request(width, height); }
                // Store notebook so tabs can be added later
                self.widgets.insert(id.clone(), nb.clone().upcast());
                Some(nb.upcast())
            }
            "tab" => {
                // Add a page to an existing notebook widget.
                // Props: notebook:<id>, label:<tab title>
                // The tab content is a vertical box registered as a panel.
                let nb_id = props.get("notebook").cloned().unwrap_or_default();
                let label = props.get("label").cloned().unwrap_or_else(|| "Tab".into());
                let nb_widget = self.widgets.get(&nb_id).cloned();
                if let Some(ref nbw) = nb_widget {
                    if let Some(nb) = nbw.downcast_ref::<gtk::Notebook>() {
                        let inner = gtk::Box::new(gtk::Orientation::Vertical, 4);
                        inner.set_margin_start(4);
                        inner.set_margin_end(4);
                        inner.set_margin_top(4);
                        inner.set_margin_bottom(4);
                        let tab_label = gtk::Label::new(Some(&label));
                        nb.append_page(&inner, Some(&tab_label));
                        inner.set_visible(true);
                        // Register the inner box as a panel
                        self.panels.insert(id.clone(), inner);
                        let wid = id.clone();
                        let tx = pty_tx.clone();
                        nb.connect_switch_page(move |_, _, page_num| {
                            if let Some(ref tx) = tx {
                                let msg = format!("\x1b]1337;WidgetEvent=id:{};action:switched;value:{}\x07", wid, page_num);
                                let _ = tx.send(msg.into_bytes());
                            }
                        });
                        return None; // Already placed inside notebook
                    }
                }
                eprintln!("[WIDGET] notebook '{}' not found for tab", nb_id);
                None
            }
            "close" | "closebutton" => {
                // A button that sends Ctrl+C (ETX) + exit command to the PTY
                let label = props.get("label").cloned().unwrap_or_else(|| "✕ Close".into());
                let btn = gtk::Button::with_label(&label);
                btn.add_css_class("destructive-action");
                let wid = id.clone();
                let tx = pty_tx.clone();
                btn.connect_clicked(move |_| {
                    if let Some(ref tx) = tx {
                        // Send event first so demo can handle it
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:close;value:true\x07", wid);
                        let _ = tx.send(msg.into_bytes());
                        // Then send Ctrl+C (ETX byte 0x03) to kill the running script
                        let _ = tx.send(vec![0x03]);
                    }
                });
                Some(btn.upcast())
            }
            "titlebar" => {
                let title = props.get("title").cloned().unwrap_or_else(|| "App".into());
                let icon  = props.get("icon").cloned();
                let show_close: bool = props.get("close").map(|v| v != "false" && v != "0").unwrap_or(true);

                let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                bar.add_css_class("app-titlebar");
                bar.set_hexpand(true);

                // Left: icon + title
                let left = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                left.set_hexpand(true);
                left.set_valign(gtk::Align::Center);
                left.set_margin_start(10);
                left.set_margin_top(6);
                left.set_margin_bottom(6);
                if let Some(icon_name) = icon {
                    let img = gtk::Image::from_icon_name(&icon_name);
                    img.set_pixel_size(16);
                    left.append(&img);
                }
                let lbl = gtk::Label::new(Some(&title));
                lbl.add_css_class("app-titlebar-title");
                left.append(&lbl);
                bar.append(&left);

                // Right: window controls + custom panel
                let right = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                right.set_valign(gtk::Align::Center);
                right.set_margin_end(8);
                right.set_margin_top(4);
                right.set_margin_bottom(4);
                bar.append(&right);

                if show_close {
                    // Minimize
                    let min_btn = gtk::Button::new();
                    min_btn.set_child(Some(&gtk::Image::from_icon_name("window-minimize-symbolic")));
                    min_btn.add_css_class("app-wm-btn");
                    min_btn.set_has_frame(false);
                    right.append(&min_btn);
                    // Maximize
                    let max_btn = gtk::Button::new();
                    max_btn.set_child(Some(&gtk::Image::from_icon_name("window-maximize-symbolic")));
                    max_btn.add_css_class("app-wm-btn");
                    max_btn.set_has_frame(false);
                    right.append(&max_btn);
                    // Close
                    let close_btn = gtk::Button::new();
                    close_btn.set_child(Some(&gtk::Image::from_icon_name("window-close-symbolic")));
                    close_btn.add_css_class("app-wm-btn");
                    close_btn.add_css_class("app-wm-close");
                    close_btn.set_has_frame(false);
                    let wid = id.clone();
                    let tx = pty_tx.clone();
                    close_btn.connect_clicked(move |_| {
                        if let Some(ref tx) = tx {
                            let msg = format!("\x1b]1337;WidgetEvent=id:{};action:close;value:\x07", wid);
                            let _ = tx.send(msg.into_bytes());
                            let _ = tx.send(vec![0x03]);
                        }
                    });
                    right.append(&close_btn);
                }

                self.panels.insert(format!("{}-right", id), right);
                self.panels.insert(id.clone(), left.clone()); // panel:<id> → left area
                self.widgets.insert(id.clone(), lbl.upcast());
                Some(bar.upcast())
            }
            "statusbar" => {
                let text = props.get("text").cloned().unwrap_or_else(|| "Ready".into());

                let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                bar.add_css_class("app-statusbar");
                bar.set_hexpand(true);

                let lbl = gtk::Label::new(Some(&text));
                lbl.add_css_class("app-statusbar-text");
                lbl.set_halign(gtk::Align::Start);
                lbl.set_hexpand(true);
                lbl.set_margin_start(8);
                lbl.set_margin_top(3);
                lbl.set_margin_bottom(3);
                bar.append(&lbl);

                // Right panel for extra status items
                let right = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                right.set_margin_end(8);
                right.set_valign(gtk::Align::Center);
                bar.append(&right);

                self.panels.insert(format!("{}-right", id), right);
                self.panels.insert(id.clone(), bar.clone());
                self.widgets.insert(id.clone(), lbl.upcast());
                Some(bar.upcast())
            }
            "menubar" => {
                // Horizontal box holding MenuButtons — no gio models, purely manual.
                let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                bx.set_hexpand(true);
                bx.add_css_class("menubar-box");
                self.panels.insert(id.clone(), bx.clone());
                eprintln!("[MENU] created menubar id={}", id);
                Some(bx.upcast())
            }
            "menu" | "submenu" => {
                let label = props.get("label").cloned().unwrap_or_else(|| "Menu".into());
                let menubar_id = props.get("menubar").cloned();
                let parent_menu_id = props.get("menu").cloned();

                // Build popover content box
                let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
                inner.set_margin_top(4);
                inner.set_margin_bottom(4);

                let popover = gtk::Popover::new();
                popover.set_child(Some(&inner));
                popover.set_has_arrow(false);
                popover.set_autohide(true);
                popover.add_css_class("menu-popover");

                // CRITICAL: Parent the popover on the TextView — NOT on any
                // widget embedded inside a TextBuffer child anchor.  Child
                // anchors break GTK's parent chain for active-state
                // propagation, causing "Broken accounting of active state"
                // warnings and preventing the popover from opening.
                if let Some(tv) = self.view.upgrade() {
                    popover.set_parent(&tv);
                }

                // Top-level: attach a label to the menubar box
                if let Some(ref mbar_id) = menubar_id {
                    let lbl = gtk::Label::new(Some(&label));
                    lbl.add_css_class("menubar-item");
                    // Hover tracking via contains-pointer (works across children)
                    let motion = gtk::EventControllerMotion::new();
                    let lbl_ref = lbl.clone();
                    motion.connect_notify_local(Some("contains-pointer"), move |ctrl, _| {
                        if ctrl.contains_pointer() {
                            lbl_ref.add_css_class("hovered");
                        } else {
                            lbl_ref.remove_css_class("hovered");
                        }
                    });
                    lbl.add_controller(motion);
                    let click = gtk::GestureClick::new();
                    let pop = popover.clone();
                    click.connect_pressed(move |gesture, _, _, _| {
                        gesture.set_state(gtk::EventSequenceState::Claimed);
                    });
                    let pop2 = pop.clone();
                    click.connect_released(move |gesture, _, _, _| {
                        gesture.set_state(gtk::EventSequenceState::Claimed);
                        if let Some(widget) = gesture.widget() {
                            if let Some(parent) = pop2.parent() {
                                if let Some(bounds) = widget.compute_bounds(&parent) {
                                    pop2.set_pointing_to(Some(&gtk::gdk::Rectangle::new(
                                        bounds.x() as i32,
                                        (bounds.y() + bounds.height()) as i32,
                                        bounds.width() as i32,
                                        1,
                                    )));
                                }
                            }
                        }
                        if pop2.is_visible() { pop2.popdown(); } else { pop2.popup(); }
                    });
                    lbl.add_controller(click);
                    if let Some(mbar_box) = self.panels.get(mbar_id) {
                        mbar_box.append(&lbl);
                    }
                } else if let Some(ref pid) = parent_menu_id {
                    // Nested submenu: add a box with GestureClick for explicit hover/background control
                    if let Some(parent_box) = self.menu_boxes.get(pid) {
                        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                        row.add_css_class("menu-row");
                        row.set_halign(gtk::Align::Fill);
                        row.set_hexpand(true);
                        
                        // Hover tracking via EVENT CONTROLLER using motion
                        let motion = gtk::EventControllerMotion::new();
                        let r1 = row.clone();
                        motion.connect_enter(move |_, _, _| { r1.add_css_class("hovered"); });
                        let r2 = row.clone();
                        motion.connect_leave(move |_| { r2.remove_css_class("hovered"); });
                        row.add_controller(motion);

                        let lbl = gtk::Label::new(Some(&label));
                        lbl.set_hexpand(true);
                        lbl.set_halign(gtk::Align::Start);
                        let arrow = gtk::Image::from_icon_name("go-next-symbolic");
                        arrow.set_halign(gtk::Align::End);
                        arrow.set_margin_start(12);
                        row.append(&lbl);
                        row.append(&arrow);
                        popover.set_position(gtk::PositionType::Right);
                        let pop = popover.clone();
                        
                        let click = gtk::GestureClick::new();
                        click.connect_released(move |gesture, _, _, _| {
                            gesture.set_state(gtk::EventSequenceState::Claimed);
                            if let Some(widget) = gesture.widget() {
                                if let Some(parent) = pop.parent() {
                                    if let Some(bounds) = widget.compute_bounds(&parent) {
                                        pop.set_pointing_to(Some(&gtk::gdk::Rectangle::new(
                                            (bounds.x() + bounds.width()) as i32,
                                            bounds.y() as i32,
                                            1,
                                            bounds.height() as i32,
                                        )));
                                    }
                                }
                            }
                            pop.popup();
                        });
                        row.add_controller(click);
                        parent_box.append(&row);
                    }
                }

                // Track parent relationship for close chain
                if let Some(ref mbar_id) = menubar_id {
                    self.menu_parents.insert(id.clone(), mbar_id.clone());
                } else if let Some(ref pid) = parent_menu_id {
                    self.menu_parents.insert(id.clone(), pid.clone());
                }

                self.menu_boxes.insert(id.clone(), inner);
                self.menu_popovers.insert(id.clone(), popover);
                eprintln!("[MENU] created menu id={} label={}", id, label);
                None
            }
            "menuitem" => {
                let label = props.get("label").cloned().unwrap_or_else(|| "Item".into());
                let menu_id = props.get("menu").cloned().unwrap_or_default();
                
                // Regular menu item using Box + GestureClick to avoid all button styling
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                row.add_css_class("menu-row");
                row.set_halign(gtk::Align::Fill);
                row.set_hexpand(true);
                
                let motion = gtk::EventControllerMotion::new();
                let r1 = row.clone();
                motion.connect_enter(move |_, _, _| { r1.add_css_class("hovered"); });
                let r2 = row.clone();
                motion.connect_leave(move |_| { r2.remove_css_class("hovered"); });
                row.add_controller(motion);

                let lbl = gtk::Label::new(Some(&label));
                lbl.set_halign(gtk::Align::Start);
                lbl.set_hexpand(true);
                row.append(&lbl);

                let wid = id.clone();
                let tx = pty_tx.clone();
                // Collect all ancestor popovers so clicking closes the entire chain
                let mut close_pops: Vec<gtk::Popover> = Vec::new();
                let mut cur = menu_id.clone();
                loop {
                    if let Some(p) = self.menu_popovers.get(&cur) {
                        close_pops.push(p.clone());
                    }
                    if let Some(parent) = self.menu_parents.get(&cur) {
                        cur = parent.clone();
                    } else {
                        break;
                    }
                }
                
                let click = gtk::GestureClick::new();
                click.connect_released(move |gesture, _, _, _| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    for p in &close_pops { p.popdown(); }
                    if let Some(ref tx) = tx {
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:activated;value:\x07", wid);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                row.add_controller(click);
                
                if let Some(menu_box) = self.menu_boxes.get(&menu_id) {
                    menu_box.append(&row);
                }
                self.widgets.insert(id.clone(), row.clone().upcast());
                eprintln!("[MENU] menuitem id={} in menu={}", id, menu_id);
                None
            }
            "menusep" | "menuseparator" => {
                let menu_id = props.get("menu").cloned().unwrap_or_default();
                if let Some(menu_box) = self.menu_boxes.get(&menu_id) {
                    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
                    sep.add_css_class("menu-sep");
                    menu_box.append(&sep);
                }
                eprintln!("[MENU] separator in menu={}", menu_id);
                None
            }
            "menucheck" | "menutoggle" => {
                let label = props.get("label").cloned().unwrap_or_else(|| "Check".into());
                let menu_id = props.get("menu").cloned().unwrap_or_default();
                let checked = props.get("checked").map(|v| v == "true" || v == "1").unwrap_or(false);
                
                // Create a row container to have uniform hover over the full width
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                row.add_css_class("menu-row");
                row.set_halign(gtk::Align::Fill);
                row.set_hexpand(true);
                
                let motion = gtk::EventControllerMotion::new();
                let r1 = row.clone();
                motion.connect_enter(move |_, _, _| { r1.add_css_class("hovered"); });
                let r2 = row.clone();
                motion.connect_leave(move |_| { r2.remove_css_class("hovered"); });
                row.add_controller(motion);

                let cb = gtk::CheckButton::new();
                cb.set_active(checked);
                cb.set_can_focus(false); // remove focus ring from clicking it
                
                let lbl = gtk::Label::new(Some(&label));
                lbl.set_halign(gtk::Align::Start);
                lbl.set_margin_start(6);
                lbl.set_hexpand(true);
                
                row.append(&cb);
                row.append(&lbl);

                let wid = id.clone();
                let tx = pty_tx.clone();
                
                let click = gtk::GestureClick::new();
                let cb_clone = cb.clone();
                click.connect_released(move |gesture, _, _, _| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    cb_clone.set_active(!cb_clone.is_active());
                });
                row.add_controller(click);

                cb.connect_toggled(move |c| {
                    if let Some(ref tx) = tx {
                        let val = if c.is_active() { "true" } else { "false" };
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:toggled;value:{}\x07", wid, val);
                        let _ = tx.send(msg.into_bytes());
                    }
                });

                if let Some(menu_box) = self.menu_boxes.get(&menu_id) {
                    menu_box.append(&row);
                }
                self.widgets.insert(id.clone(), row.clone().upcast());
                eprintln!("[MENU] menucheck id={} in menu={}", id, menu_id);
                None
            }
            "toolbar" => {
                let spacing: i32 = props.get("spacing").and_then(|s| s.parse().ok()).unwrap_or(2);
                let width: i32 = props.get("width").and_then(|w| w.parse().ok()).unwrap_or(-1);
                let bx = gtk::Box::new(gtk::Orientation::Horizontal, spacing);
                bx.add_css_class("toolbar");
                bx.set_hexpand(true);
                bx.set_margin_start(2);
                bx.set_margin_end(2);
                bx.set_margin_top(2);
                bx.set_margin_bottom(2);
                if width > 0 { bx.set_size_request(width, -1); }
                self.panels.insert(id.clone(), bx.clone());
                eprintln!("[TOOLBAR] created toolbar id={}", id);
                Some(bx.upcast())
            }
            "toolbutton" => {
                let label = props.get("label").cloned().unwrap_or_default();
                let icon = props.get("icon").cloned();
                let tooltip = props.get("tooltip").cloned();
                let btn = gtk::Button::new();
                match (icon.as_deref(), label.is_empty()) {
                    (Some(icon_name), false) => {
                        let content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                        let img = gtk::Image::from_icon_name(icon_name);
                        let lbl = gtk::Label::new(Some(&label));
                        content.append(&img);
                        content.append(&lbl);
                        btn.set_child(Some(&content));
                    }
                    (Some(icon_name), true) => {
                        btn.set_icon_name(icon_name);
                    }
                    (None, false) => {
                        btn.set_label(&label);
                    }
                    _ => {
                        btn.set_label("•");
                    }
                }
                btn.add_css_class("flat");
                if let Some(ref tip) = tooltip {
                    btn.set_tooltip_text(Some(tip));
                }
                let wid = id.clone();
                let tx = pty_tx.clone();
                btn.connect_clicked(move |_| {
                    if let Some(ref tx) = tx {
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:clicked;value:\x07", wid);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(btn.upcast())
            }
            "tooltoggle" => {
                let label = props.get("label").cloned().unwrap_or_default();
                let icon = props.get("icon").cloned();
                let active = props.get("active").map(|v| v == "true" || v == "1").unwrap_or(false);
                let tooltip = props.get("tooltip").cloned();
                let btn = gtk::ToggleButton::new();
                match (icon.as_deref(), label.is_empty()) {
                    (Some(icon_name), false) => {
                        let content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                        let img = gtk::Image::from_icon_name(icon_name);
                        let lbl = gtk::Label::new(Some(&label));
                        content.append(&img);
                        content.append(&lbl);
                        btn.set_child(Some(&content));
                    }
                    (Some(icon_name), true) => {
                        let img = gtk::Image::from_icon_name(icon_name);
                        btn.set_child(Some(&img));
                    }
                    (None, false) => {
                        btn.set_label(&label);
                    }
                    _ => {
                        btn.set_label("•");
                    }
                }
                btn.set_active(active);
                btn.add_css_class("flat");
                if let Some(ref tip) = tooltip {
                    btn.set_tooltip_text(Some(tip));
                }
                let wid = id.clone();
                let tx = pty_tx.clone();
                btn.connect_toggled(move |b| {
                    if let Some(ref tx) = tx {
                        let val = if b.is_active() { "true" } else { "false" };
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:toggled;value:{}\x07", wid, val);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(btn.upcast())
            }
            _ => {
                eprintln!("[WIDGET] unknown type: {}", widget_type);
                None
            }
        }
    }

    /// Insert a GTK widget. If `panel` property is set, add to that panel;
    /// otherwise embed directly in the text buffer at cursor.
    pub fn insert_widget(&mut self, spec: &str) {
        let props = Self::parse_widget_props(spec);
        let widget_type = match props.get("type") {
            Some(t) => t.clone(),
            None => { eprintln!("[WIDGET] no type specified"); return; }
        };
        let id = props.get("id").cloned().unwrap_or_else(|| "unnamed".to_string());
        let panel_id = props.get("panel").or(props.get("toolbar")).cloned();
        let row: i32 = props.get("row").and_then(|r| r.parse().ok()).unwrap_or(0);
        let col: i32 = props.get("col").and_then(|c| c.parse().ok()).unwrap_or(0);
        let colspan: i32 = props.get("colspan").and_then(|c| c.parse().ok()).unwrap_or(1);
        let rowspan: i32 = props.get("rowspan").and_then(|r| r.parse().ok()).unwrap_or(1);
        let expand: bool = props.get("expand").map(|v| v == "true" || v == "1").unwrap_or(false);

        let widget = match self.build_widget(&props) {
            Some(w) => w,
            None => return,
        };
        widget.set_visible(true);
        // Apply per-widget colours if provided
        let bg = props.get("bg_color").or(props.get("bg")).map(|s| s.as_str());
        let fg = props.get("fg_color").or(props.get("fg")).or(props.get("color")).map(|s| s.as_str());
        if bg.is_some() || fg.is_some() {
            self.apply_widget_colors(&id, &widget, bg, fg);
        }
        // Font / size
        let font = props.get("font").cloned();
        let fsize: Option<u32> = props.get("size").and_then(|v| v.parse().ok());
        if font.is_some() || fsize.is_some() {
            self.apply_widget_font(&id, &widget, font.as_deref(), fsize);
        }
        // Tooltip
        if let Some(tip) = props.get("tooltip") {
            widget.set_tooltip_text(Some(tip));
        }
        // Visible / enabled overrides (default is shown + enabled)
        if let Some(vis) = props.get("visible") {
            widget.set_visible(vis == "true" || vis == "1");
        }
        if let Some(ena) = props.get("enabled") {
            widget.set_sensitive(ena != "false" && ena != "0");
        }
        if let Some(sens) = props.get("sensitive") {
            widget.set_sensitive(sens == "true" || sens == "1");
        }
        // Block-level widgets always fill horizontal space when placed inline.
        let default_expand = matches!(widget_type.as_str(),
            "menubar" | "toolbar" | "splitview" | "paned" | "panel" | "titlebar" | "statusbar"
        );
        let h_expand = props.get("hexpand").map(|v| v == "true" || v == "1").unwrap_or(false);
        let v_expand = props.get("vexpand").map(|v| v == "true" || v == "1").unwrap_or(false);
        if expand || default_expand || h_expand { widget.set_hexpand(true); }
        if v_expand { widget.set_vexpand(true); }

        // Store widget by ID for later updates (textview stores itself inside build_widget)
        if !self.widgets.contains_key(&id) {
            self.widgets.insert(id.clone(), widget.clone());
        }

        // Route to panel, grid, or inline
        if let Some(ref pid) = panel_id {
            if let Some(grid) = self.grids.get(pid) {
                grid.attach(&widget, col, row, colspan, rowspan);
                eprintln!("[WIDGET] {}(id={}) -> grid {} at row={} col={}", widget_type, id, pid, row, col);
                return;
            }
            if let Some(fb) = self.flow_panels.get(pid).cloned() {
                fb.append(&widget);
                eprintln!("[WIDGET] {}(id={}) -> flowbox {}", widget_type, id, pid);
                return;
            }
            if let Some(bx) = self.panels.get(pid) {
                bx.append(&widget);
                eprintln!("[WIDGET] {}(id={}) -> panel {}", widget_type, id, pid);
                return;
            }
            eprintln!("[WIDGET] panel '{}' not found, inserting inline", pid);
        }

        // Inline: embed in text buffer via anchor
        let tv = match self.view.upgrade() {
            Some(tv) => tv,
            None => return,
        };
        let buffer = self.active_buffer();
        let cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
        let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
        let mut iter = self.ensure_cursor_position(cx, cy);
        let anchor = buffer.create_child_anchor(&mut iter);
        tv.add_child_at_anchor(&widget, &anchor);

        // If block:true is set (or implied by widget type), inject a newline
        // into the buffer after the anchor so the next widget/text starts on
        // a fresh line.  Widgets placed inside a parent panel are never block
        // (the parent controls layout).  Explicit block:false overrides type default.
        let default_block = matches!(widget_type.as_str(),
            "menubar" | "toolbar" | "splitview" | "paned" | "panel" | "titlebar" | "statusbar"
        );
        let block = props
            .get("block")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(default_block);

        if block {
            let end_of_anchor = buffer.iter_at_child_anchor(&anchor);
            let mut after = end_of_anchor;
            after.forward_char();
            if after.line_offset() != 0 || after.char() != '\n' {
                buffer.insert(&mut after, "\n");
            }
            if self.is_alternate {
                self.alt_cursor_x = 0;
                self.alt_cursor_y = cy + 1;
            } else {
                self.cursor_x = 0;
                self.cursor_y = cy + 1;
            }
        } else if self.is_alternate {
            self.alt_cursor_x += 1;
        } else {
            self.cursor_x += 1;
        }
        eprintln!("[WIDGET] inserted {}(id={}) inline at ({}, {})", widget_type, id, cx, cy);
    }

    /// Update an existing widget's properties.
    /// `spec` is e.g. "id:lbl1;text:New text" or "id:pb1;value:75" or "id:tv1;append:new line"
    pub fn update_widget(&mut self, spec: &str) {
        let props = Self::parse_widget_props(spec);
        let id = match props.get("id") {
            Some(id) => id.clone(),
            None => { eprintln!("[UPDATE] no id specified"); return; }
        };
        // Handle menu action updates (enable/disable, toggle state)
        // (Removed — menu items are now stored as regular widgets below)

        let widget = match self.widgets.get(&id) {
            Some(w) => w.clone(),
            None => { eprintln!("[UPDATE] widget '{}' not found", id); return; }
        };

        // Try each property update
        if let Some(text) = props.get("text") {
            // parse_widget_props already unescaped \n, \r, \;, \\
            if let Some(lbl) = widget.downcast_ref::<gtk::Label>() {
                lbl.set_text(text);
            } else if let Some(btn) = widget.downcast_ref::<gtk::Button>() {
                btn.set_label(text);
            } else if let Some(entry) = widget.downcast_ref::<gtk::Entry>() {
                entry.set_text(text);
            } else if let Some(tv) = widget.downcast_ref::<gtk::TextView>() {
                tv.buffer().set_text(text);
            } else if let Some(pb) = widget.downcast_ref::<gtk::ProgressBar>() {
                pb.set_text(Some(text));
                pb.set_show_text(true);
            }
        }
        if let Some(append_text) = props.get("append") {
            if let Some(tv) = widget.downcast_ref::<gtk::TextView>() {
                let buf = tv.buffer();
                let mut end = buf.end_iter();
                buf.insert(&mut end, append_text);
                buf.insert(&mut end, "\n");
                // Auto-scroll to bottom
                let mark = buf.create_mark(None, &buf.end_iter(), false);
                tv.scroll_mark_onscreen(&mark);
            }
        }
        if let Some(val_str) = props.get("value") {
            if let Ok(val) = val_str.parse::<f64>() {
                if let Some(pb) = widget.downcast_ref::<gtk::ProgressBar>() {
                    pb.set_fraction((val / 100.0).clamp(0.0, 1.0));
                } else if let Some(scale) = widget.downcast_ref::<gtk::Scale>() {
                    scale.set_value(val);
                } else if let Some(spin) = widget.downcast_ref::<gtk::SpinButton>() {
                    spin.set_value(val);
                } else if let Some(lb) = widget.downcast_ref::<gtk::LevelBar>() {
                    lb.set_value(val);
                }
            }
            // Also handle boolean value for switch/check
            if let Some(sw) = widget.downcast_ref::<gtk::Switch>() {
                sw.set_active(val_str == "true" || val_str == "1");
            } else if let Some(cb) = widget.downcast_ref::<gtk::CheckButton>() {
                cb.set_active(val_str == "true" || val_str == "1");
            }
        }
        if let Some(label) = props.get("label") {
            let label = &label.replace("\\n", "\n");
            if let Some(btn) = widget.downcast_ref::<gtk::Button>() {
                btn.set_label(label);
            } else if let Some(cb) = widget.downcast_ref::<gtk::CheckButton>() {
                cb.set_label(Some(label));
            } else if let Some(exp) = widget.downcast_ref::<gtk::Expander>() {
                exp.set_label(Some(label));
            }
        }
        if let Some(new_class) = props.get("css_class") {
            if let Some(old_class) = self.widget_css_classes.get(&id) {
                widget.remove_css_class(old_class);
            }
            widget.add_css_class(new_class);
            self.widget_css_classes.insert(id.clone(), new_class.clone());
            // If the caller registered explicit bg/fg colours for this class,
            // inject them via the high-priority per-widget provider so the
            // GTK theme cannot override them.
            let name = id.clone();
            let w2   = widget.clone();
            let nc   = new_class.clone();
            // Build inline CSS based on the new class so the correct colours
            // are applied immediately without needing a separate WU call.
            w2.set_widget_name(&format!("wgt-{}", name));
            let inline_css = match nc.as_str() {
                "ttt-x"     => Some(format!(
                    "#wgt-{} {{ background-color:#3498db; background-image:none; }}\
                     #wgt-{} label {{ color:white; font-size:26px; font-weight:bold; }}",
                    name, name)),
                "ttt-o"     => Some(format!(
                    "#wgt-{} {{ background-color:#e74c3c; background-image:none; }}\
                     #wgt-{} label {{ color:white; font-size:26px; font-weight:bold; }}",
                    name, name)),
                "ttt-win"   => Some(format!(
                    "#wgt-{} {{ background-color:#2ecc71; background-image:none; }}\
                     #wgt-{} label {{ color:white; font-size:26px; font-weight:bold; }}",
                    name, name)),
                "ttt-empty" => Some(format!(
                    "#wgt-{} {{ background-color:#44475a; background-image:none; }}\
                     #wgt-{} label {{ color:#aaa; font-size:26px; font-weight:bold; }}",
                    name, name)),
                "card-hidden"  => Some(format!(
                    "#wgt-{} {{ background-color:#2c3e50; background-image:none; border-radius:8px; }}\
                     #wgt-{} label {{ color:#566573; font-size:20px; font-weight:bold; }}",
                    name, name)),
                "card-red"     => Some(format!(
                    "#wgt-{} {{ background-color:#c0392b; background-image:none; border-radius:8px; }}\
                     #wgt-{} label {{ color:white; font-size:20px; font-weight:bold; }}",
                    name, name)),
                "card-black"   => Some(format!(
                    "#wgt-{} {{ background-color:#1a252f; background-image:none; border-radius:8px; }}\
                     #wgt-{} label {{ color:#3498db; font-size:20px; font-weight:bold; }}",
                    name, name)),
                "card-matched" => Some(format!(
                    "#wgt-{} {{ background-color:#1e8449; background-image:none; border-radius:8px; }}\
                     #wgt-{} label {{ color:white; font-size:20px; font-weight:bold; }}",
                    name, name)),
                "card-wrong"   => Some(format!(
                    "#wgt-{} {{ background-color:#922b21; background-image:none; border-radius:8px; }}\
                     #wgt-{} label {{ color:white; font-size:20px; font-weight:bold; }}",
                    name, name)),
                _ => None,
            };
            if let Some(css) = inline_css {
                if let Some(provider) = self.widget_css_providers.get(&id) {
                    provider.load_from_data(&css);
                } else {
                    let provider = gtk::CssProvider::new();
                    provider.load_from_data(&css);
                    gtk::style_context_add_provider_for_display(
                        &w2.display(),
                        &provider,
                        gtk::STYLE_PROVIDER_PRIORITY_USER,
                    );
                    self.widget_css_providers.insert(id.clone(), provider);
                }
            }
        }
        // Dynamic colour updates
        let bg = props.get("bg_color").or(props.get("bg")).map(|s| s.as_str());
        let fg = props.get("fg_color").or(props.get("fg")).or(props.get("color")).map(|s| s.as_str());
        if bg.is_some() || fg.is_some() {
            let w = widget.clone();
            self.apply_widget_colors(&id, &w, bg, fg);
        }
        // Font / size
        {
            let font = props.get("font").cloned();
            let fsize: Option<u32> = props.get("size").and_then(|v| v.parse().ok());
            if font.is_some() || fsize.is_some() {
                let w = widget.clone();
                self.apply_widget_font(&id, &w, font.as_deref(), fsize);
            }
        }
        // Tooltip
        if let Some(tip) = props.get("tooltip") {
            widget.set_tooltip_text(Some(tip));
        }
        if let Some(sensitive) = props.get("sensitive") {
            widget.set_sensitive(sensitive == "true" || sensitive == "1");
        }
        // enabled: is friend to sensitive (enabled:false = disable)
        if let Some(ena) = props.get("enabled") {
            widget.set_sensitive(ena != "false" && ena != "0");
        }
        if let Some(visible) = props.get("visible") {
            widget.set_visible(visible == "true" || visible == "1");
        }
        // Spinner control
        if let Some(spinning) = props.get("spinning") {
            if let Some(sp) = widget.downcast_ref::<gtk::Spinner>() {
                if spinning == "true" || spinning == "1" { sp.start(); } else { sp.stop(); }
            }
        }
        // Update picturebox: path or data (base64)
        if let Some(path) = props.get("path") {
            if let Some(pic) = widget.downcast_ref::<gtk::Picture>() {
                let file = gtk::gio::File::for_path(path);
                pic.set_file(Some(&file));
                eprintln!("[UPDATE] picturebox '{}' set path={}", id, path);
            }
        }
        if let Some(b64) = props.get("data") {
            if let Some(pic) = widget.downcast_ref::<gtk::Picture>() {
                if let Ok(bytes) = BASE64.decode(b64.as_bytes()) {
                    let pixbuf_loader = gtk::gdk_pixbuf::PixbufLoader::new();
                    let _ = pixbuf_loader.write(&bytes);
                    let _ = pixbuf_loader.close();
                    if let Some(pixbuf) = pixbuf_loader.pixbuf() {
                        let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
                        pic.set_paintable(Some(&texture));
                    }
                    eprintln!("[UPDATE] picturebox '{}' set from base64 ({} bytes)", id, bytes.len());
                }
            }
        }
        // TreeView / ListView row operations
        if let Some(action) = props.get("action").map(|s| s.as_str()) {
            match action {
                "clear" => {
                    if let Some(ts) = self.tree_stores.get(&id).cloned() {
                        ts.clear();
                        self.tree_row_iters.remove(&id);
                    } else if let Some(ls) = self.list_stores.get(&id).cloned() {
                        ls.clear();
                    }
                }
                "addrow" => {
                    if let Some(ts) = self.tree_stores.get(&id).cloned() {
                        let label = props.get("label").cloned().unwrap_or_default();
                        let row_id = props.get("rowid").cloned().unwrap_or_else(|| label.clone());
                        let parent_row_id = props.get("parent").cloned();
                        let parent_iter = parent_row_id.as_ref().and_then(|pid| {
                            self.tree_row_iters.get(&id).and_then(|m| m.get(pid)).cloned()
                        });
                        let iter = ts.append(parent_iter.as_ref());
                        ts.set_value(&iter, 0, &label.to_value());
                        self.tree_row_iters
                            .entry(id.clone())
                            .or_insert_with(std::collections::HashMap::new)
                            .insert(row_id, iter);
                    } else if let Some(ls) = self.list_stores.get(&id).cloned() {
                        if let Some(cols_str) = props.get("cols") {
                            let vals: Vec<&str> = cols_str.split('|').collect();
                            let iter = ls.append();
                            for (i, val) in vals.iter().enumerate() {
                                ls.set_value(&iter, i as u32, &val.to_value());
                            }
                            // Optional per-row foreground/background colours
                            let n_data = self.table_data_cols.get(&id).copied().unwrap_or(vals.len());
                            if let Some(fg) = props.get("fg") {
                                ls.set_value(&iter, n_data as u32, &fg.to_value());
                            }
                            if let Some(bg) = props.get("bg") {
                                ls.set_value(&iter, (n_data + 1) as u32, &bg.to_value());
                            }
                        }
                    }
                }
                // Change the colour of an existing row by index
                "rowcolor" => {
                    if let Some(ls) = self.list_stores.get(&id).cloned() {
                        if let Some(row_str) = props.get("row") {
                            if let Ok(row_idx) = row_str.parse::<i32>() {
                                if let Some(iter) = ls.iter_nth_child(None, row_idx) {
                                    let n_data = self.table_data_cols.get(&id).copied().unwrap_or(0);
                                    if let Some(fg) = props.get("fg") {
                                        ls.set_value(&iter, n_data as u32, &fg.to_value());
                                    }
                                    if let Some(bg) = props.get("bg") {
                                        ls.set_value(&iter, (n_data + 1) as u32, &bg.to_value());
                                    }
                                }
                            }
                        }
                    }
                }
                "expand_all" => {
                    if let Some(tv) = widget.downcast_ref::<gtk::TreeView>() {
                        tv.expand_all();
                    }
                }
                _ => {}
            }
        }
        // Graph data update
        if let Some(gd_rc) = self.graph_data.get(&id) {
            let mut gd = gd_rc.borrow_mut();
            let mut dirty = false;
            if let Some(v) = props.get("data") {
                gd.values = v.split(',')
                    .map(|s| s.split('|').filter_map(|n| n.parse().ok()).collect())
                    .collect();
                if gd.values.is_empty() { gd.values.push(Vec::new()); }
                dirty = true;
            }
            if let Some(v) = props.get("kind")   { gd.kind = v.clone();   dirty = true; }
            if let Some(v) = props.get("title")  { gd.title = v.clone();  dirty = true; }
            if let Some(v) = props.get("labels") {
                gd.labels = v.split('|').map(|s| s.to_string()).collect();
                dirty = true;
            }
            if let Some(v) = props.get("colors") {
                gd.color_strs = v.split('|').map(|s| s.to_string()).collect();
                dirty = true;
            }
            drop(gd);
            if dirty {
                if let Some(da) = widget.downcast_ref::<gtk::DrawingArea>() {
                    da.queue_draw();
                }
            }
        }
        eprintln!("[UPDATE] widget '{}' updated", id);
    }

    pub fn insert_image(&mut self, data: Vec<u8>) {
        eprintln!("[IMG] insert_image called with {} bytes", data.len());
        if data.len() >= 4 {
            eprintln!("[IMG] first 4 bytes: {:02x} {:02x} {:02x} {:02x}",
                data[0], data[1], data[2], data[3]);
        }
        if let Some(_tv) = self.view.upgrade() {
            let pixbuf_loader = gtk::gdk_pixbuf::PixbufLoader::new();
            if let Err(e) = pixbuf_loader.write(&data) {
                eprintln!("[IMG] pixbuf_loader.write failed: {}", e);
                return;
            }
            if let Err(e) = pixbuf_loader.close() {
                eprintln!("[IMG] pixbuf_loader.close failed: {}", e);
                return;
            }
            match pixbuf_loader.pixbuf() {
                None => eprintln!("[IMG] pixbuf_loader.pixbuf() returned None"),
                Some(pixbuf) => {
                    let img_w = pixbuf.width();
                    let img_h = pixbuf.height();
                    eprintln!("[IMG] pixbuf loaded: {}x{}", img_w, img_h);
                    let buffer = self.active_buffer();
                    let cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
                    let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
                    let mut iter = self.ensure_cursor_position(cx, cy);

                    // GTK4's native way to embed images in a TextBuffer:
                    let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
                    buffer.insert_paintable(&mut iter, &texture);

                    // advance cursor past the paintable character
                    if self.is_alternate { self.alt_cursor_x += 1; } else { self.cursor_x += 1; }
                    eprintln!("[IMG] paintable inserted at cursor ({}, {}), texture {}x{}", cx, cy, img_w, img_h);
                }
            }
        } else {
            eprintln!("[IMG] view WeakRef was dead");
        }
    }

    // ── Floating window management ─────────────────────────────────────────

    /// Create (or replace) a named floating window.
    /// spec = "id:w1;title:My Window;width:400;height:300;modal:false;x:100;y:100;resizable:true"
    /// The window's root Box is registered as a panel under the window id so
    /// Panel= and Widget= with panel:<id> route content into the window.
    pub fn create_wm_window(&mut self, spec: &str) {
        let props = Self::parse_widget_props(spec);
        let id       = props.get("id").cloned().unwrap_or_else(|| "wnd".into());
        let title    = props.get("title").cloned().unwrap_or_else(|| "Window".into());
        let width: i32  = props.get("width").and_then(|v| v.parse().ok()).unwrap_or(400);
        let height: i32 = props.get("height").and_then(|v| v.parse().ok()).unwrap_or(300);
        let modal: bool = props.get("modal").map(|v| v == "true" || v == "1").unwrap_or(false);
        let resizable: bool = props.get("resizable").map(|v| v != "false" && v != "0").unwrap_or(true);
        let decorated: bool = props.get("decorated").map(|v| v != "false" && v != "0").unwrap_or(true);

        // Close any previous window with the same id
        if let Some(old) = self.windows.remove(&id) { old.close(); }

        let parent: Option<gtk::Window> = self.view.upgrade()
            .and_then(|tv| tv.root())
            .and_then(|r| r.downcast::<gtk::Window>().ok());

        let win = gtk::Window::builder()
            .title(&title)
            .default_width(width)
            .default_height(height)
            .modal(modal)
            .resizable(resizable)
            .decorated(decorated)
            .build();
        if let Some(ref p) = parent {
            win.set_transient_for(Some(p));
        }
        win.add_css_class("main-app-window");
        win.add_css_class("wm-floating");

        // Root content box (vbox) — registered as a panel so children land here
        let root_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root_box.set_hexpand(true);
        root_box.set_vexpand(true);
        win.set_child(Some(&root_box));

        // Close button fires WidgetEvent back to PTY
        let wid = id.clone();
        let pty_tx = self.pty_input_tx.clone();
        win.connect_close_request(move |_| {
            if let Some(ref tx) = pty_tx {
                let msg = format!("\x1b]1337;WidgetEvent=id:{};action:close;value:\x07", wid);
                let _ = tx.try_send(msg.into_bytes());
            }
            glib::Propagation::Proceed
        });

        self.panels.insert(id.clone(), root_box);
        self.windows.insert(id.clone(), win.clone());
        // Only present if visible is not explicitly false
        let visible: bool = props.get("visible").map(|v| v != "false" && v != "0").unwrap_or(true);
        if visible {
            win.present();
        }
        eprintln!("[WINDOW] created '{}' {}x{} modal={} visible={}", id, width, height, modal, visible);
    }

    /// Update a named floating window.
    /// spec = "id:w1;visible:false"  |  "id:w1;title:New"  |  "id:w1;modal:true"  |  "id:w1;close:"
    pub fn update_wm_window(&mut self, spec: &str) {
        let props = Self::parse_widget_props(spec);
        let id = props.get("id").cloned().unwrap_or_default();
        if id.is_empty() { return; }

        if props.contains_key("close") {
            if let Some(win) = self.windows.remove(&id) {
                win.close();
                self.panels.remove(&id);
            }
            return;
        }

        let win = match self.windows.get(&id) {
            Some(w) => w.clone(),
            None => { eprintln!("[WINDOW] update: '{}' not found", id); return; }
        };

        if let Some(vis) = props.get("visible") {
            if vis == "true" || vis == "1" {
                win.present();
            } else {
                win.set_visible(false);
            }
        }
        if let Some(t) = props.get("title") {
            win.set_title(Some(t));
        }
        if let Some(m) = props.get("modal") {
            win.set_modal(m == "true" || m == "1");
        }
        if let Some(w) = props.get("width").and_then(|v| v.parse::<i32>().ok()) {
            let h = props.get("height").and_then(|v| v.parse::<i32>().ok()).unwrap_or(-1);
            win.set_default_size(w, h);
        }
        eprintln!("[WINDOW] updated '{}'", id);
    }

    /// Show a modal alert dialog.  spec = "title:Foo;body:Bar;ok:OK"
    pub fn show_alert(&self, spec: &str) {
        let props = Self::parse_widget_props(spec);
        let title   = props.get("title").cloned().unwrap_or_else(|| "Alert".into());
        let body    = props.get("body").cloned().unwrap_or_default();
        let ok_lbl  = props.get("ok").cloned().unwrap_or_else(|| "OK".into());
        let alert_id = props.get("id").cloned().unwrap_or_else(|| "alert".into());

        let parent: Option<gtk::Window> = self.view.upgrade()
            .and_then(|tv| tv.root())
            .and_then(|r| r.downcast::<gtk::Window>().ok());

        let dialog = gtk::Window::builder()
            .title(&title)
            .modal(true)
            .default_width(320)
            .resizable(false)
            .build();
        if let Some(ref p) = parent {
            dialog.set_transient_for(Some(p));
        }
        dialog.add_css_class("main-app-window");

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 16);
        vbox.set_margin_top(24); vbox.set_margin_bottom(20);
        vbox.set_margin_start(24); vbox.set_margin_end(24);

        let title_lbl = gtk::Label::new(Some(&title));
        title_lbl.add_css_class("title-4");
        title_lbl.set_halign(gtk::Align::Center);
        vbox.append(&title_lbl);

        if !body.is_empty() {
            let body_lbl = gtk::Label::new(Some(&body));
            body_lbl.set_wrap(true);
            body_lbl.set_halign(gtk::Align::Center);
            vbox.append(&body_lbl);
        }

        // Optional input field — present when spec contains input: key
        let input_placeholder = props.get("input").cloned().unwrap_or_default();
        let entry_opt: Option<gtk::Entry> = if !input_placeholder.is_empty() {
            let e = gtk::Entry::new();
            e.set_placeholder_text(Some(&input_placeholder));
            e.set_margin_top(4);
            vbox.append(&e);
            Some(e)
        } else {
            None
        };

        let ok_btn = gtk::Button::with_label(&ok_lbl);
        ok_btn.add_css_class("suggested-action");
        ok_btn.set_halign(gtk::Align::Center);
        ok_btn.set_margin_top(8);
        let d2 = dialog.clone();
        let pty_tx = self.pty_input_tx.clone();
        ok_btn.connect_clicked(move |_| {
            let value = entry_opt.as_ref().map(|e| e.text().to_string()).unwrap_or_default();
            d2.close();
            let msg = format!("\x1b]1337;WidgetEvent=id:{};action:clicked;value:{}\x07", alert_id, value);
            if let Some(ref tx) = pty_tx {
                let _ = tx.try_send(msg.into_bytes());
            }
        });
        vbox.append(&ok_btn);

        dialog.set_child(Some(&vbox));
        dialog.present();
    }

    /// Non-blocking toast notification.  spec = "msg:Hello;duration:3000"
    pub fn show_toast(&self, spec: &str) {
        // spec may be plain text (no key=val) or "msg:X;duration:N"
        let props = Self::parse_widget_props(spec);
        let msg      = props.get("msg").cloned().unwrap_or_else(|| spec.to_string());
        let duration: u32 = props.get("duration").and_then(|v| v.parse().ok()).unwrap_or(3000);

        let parent: Option<gtk::Window> = self.view.upgrade()
            .and_then(|tv| tv.root())
            .and_then(|r| r.downcast::<gtk::Window>().ok());

        let toast_win = gtk::Window::builder()
            .decorated(false)
            .resizable(false)
            .modal(false)
            .build();
        if let Some(ref p) = parent { toast_win.set_transient_for(Some(p)); }
        toast_win.add_css_class("toast-popup");

        let lbl = gtk::Label::new(Some(&msg));
        lbl.set_margin_top(10); lbl.set_margin_bottom(10);
        lbl.set_margin_start(18); lbl.set_margin_end(18);
        lbl.set_wrap(true);
        toast_win.set_child(Some(&lbl));
        toast_win.present();

        let tw2 = toast_win.clone();
        glib::timeout_add_local_once(
            std::time::Duration::from_millis(duration as u64),
            move || { tw2.close(); }
        );
    }

    /// Yes/No confirm dialog.  spec = "id:X;title:T;body:B;confirm:Yes;cancel:No"
    pub fn show_confirm(&self, spec: &str) {
        let props = Self::parse_widget_props(spec);
        let title       = props.get("title").cloned().unwrap_or_else(|| "Confirm".into());
        let body        = props.get("body").cloned().unwrap_or_default();
        let confirm_lbl = props.get("confirm").cloned().unwrap_or_else(|| "Yes".into());
        let cancel_lbl  = props.get("cancel").cloned().unwrap_or_else(|| "No".into());
        let confirm_id  = props.get("id").cloned().unwrap_or_else(|| "confirm".into());

        let parent: Option<gtk::Window> = self.view.upgrade()
            .and_then(|tv| tv.root())
            .and_then(|r| r.downcast::<gtk::Window>().ok());

        let dialog = gtk::Window::builder()
            .title(&title).modal(true).default_width(320).resizable(false).build();
        if let Some(ref p) = parent { dialog.set_transient_for(Some(p)); }
        dialog.add_css_class("main-app-window");

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 16);
        vbox.set_margin_top(24); vbox.set_margin_bottom(20);
        vbox.set_margin_start(24); vbox.set_margin_end(24);

        let title_lbl = gtk::Label::new(Some(&title));
        title_lbl.add_css_class("title-4");
        title_lbl.set_halign(gtk::Align::Center);
        vbox.append(&title_lbl);

        if !body.is_empty() {
            let body_lbl = gtk::Label::new(Some(&body));
            body_lbl.set_wrap(true);
            body_lbl.set_halign(gtk::Align::Center);
            vbox.append(&body_lbl);
        }

        let btn_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        btn_row.set_halign(gtk::Align::Center);
        btn_row.set_margin_top(8);

        let cancel_btn = gtk::Button::with_label(&cancel_lbl);
        cancel_btn.add_css_class("destructive-action");
        let d_cancel = dialog.clone();
        let pty_cancel = self.pty_input_tx.clone();
        let cid_cancel = confirm_id.clone();
        cancel_btn.connect_clicked(move |_| {
            d_cancel.close();
            let msg = format!("\x1b]1337;WidgetEvent=id:{};action:cancel;value:false\x07", cid_cancel);
            if let Some(ref tx) = pty_cancel { let _ = tx.try_send(msg.into_bytes()); }
        });

        let confirm_btn = gtk::Button::with_label(&confirm_lbl);
        confirm_btn.add_css_class("suggested-action");
        let d_confirm = dialog.clone();
        let pty_confirm = self.pty_input_tx.clone();
        let cid_confirm = confirm_id.clone();
        confirm_btn.connect_clicked(move |_| {
            d_confirm.close();
            let msg = format!("\x1b]1337;WidgetEvent=id:{};action:confirm;value:true\x07", cid_confirm);
            if let Some(ref tx) = pty_confirm { let _ = tx.try_send(msg.into_bytes()); }
        });

        btn_row.append(&cancel_btn);
        btn_row.append(&confirm_btn);
        vbox.append(&btn_row);

        dialog.set_child(Some(&vbox));
        dialog.present();
    }
}

impl Perform for TerminalState {
    fn print(&mut self, c: char) {
        // Auto-wrap: if cursor is already at or past the right edge, advance to the next line first
        {
            let cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
            let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
            if self.cols > 0 && cx >= self.cols {
                let new_cy = cy + 1;
                if self.is_alternate {
                    self.alt_cursor_x = 0;
                    self.alt_cursor_y = new_cy;
                } else {
                    self.cursor_x = 0;
                    self.cursor_y = new_cy;
                }
            }
        }

        let cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
        let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
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
            b'\n' => {
                let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
                // Use scroll-region logic only when:
                //   a) in the alternate buffer (vi/less/etc.), or
                //   b) an explicit restricted scroll region was set via CSI r
                //      (scroll_top > 0 or scroll_bottom < rows-1)
                // For the primary buffer with the default full-screen region, the
                // buffer must GROW so scrollback history is preserved.  Calling
                // scroll_region_up on the primary buffer deleted line 0 on every LF
                // at the bottom, wiping scrollback and corrupting cursor positions.
                let restricted = self.scroll_top > 0
                    || self.scroll_bottom < self.rows.saturating_sub(1);
                if (self.is_alternate || restricted) && cy == self.scroll_bottom {
                    self.scroll_region_up(1);
                    // cursor stays at scroll_bottom
                } else {
                    if self.is_alternate { self.alt_cursor_y += 1; } else { self.cursor_y += 1; }
                }
            }
            b'\r' => { if self.is_alternate { self.alt_cursor_x = 0; } else { self.cursor_x = 0; } }
            b'\x08' | b'\x7f' => {
                let cx = if self.is_alternate { &mut self.alt_cursor_x } else { &mut self.cursor_x };
                if *cx > 0 { *cx -= 1; }
            }
            b'\t' => {
                let cx = if self.is_alternate { &mut self.alt_cursor_x } else { &mut self.cursor_x };
                *cx = (*cx / 8 + 1) * 8;
            }
            b'\x07' => {
                if let Some(v) = self.view.upgrade() {
                    v.activate_action("app.bell", None).ok();
                }
            }
            _ => {}
        }
        self.update_visual_cursor();
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
                // Scroll region up: delete from scroll_top, blank lines appear at scroll_bottom
                let count = arg0.max(1);
                self.scroll_region_up(count);
            }
            'T' => {
                // Scroll region down: insert at scroll_top, delete from scroll_bottom
                let count = arg0.max(1);
                self.scroll_region_down(count);
            }
            'r' => {
                self.scroll_top = arg0.saturating_sub(1);
                let bottom_arg = if arg1 == 0 { self.rows } else { arg1 };
                self.scroll_bottom = bottom_arg.saturating_sub(1);
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
        self.update_visual_cursor();
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'D' => {
                // Index: same as LF — advance cursor, scroll only when appropriate
                let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
                let restricted = self.scroll_top > 0
                    || self.scroll_bottom < self.rows.saturating_sub(1);
                if (self.is_alternate || restricted) && cy == self.scroll_bottom {
                    self.scroll_region_up(1);
                } else {
                    if self.is_alternate { self.alt_cursor_y += 1; } else { self.cursor_y += 1; }
                }
            }
            b'M' => { // Reverse Index
                let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
                if cy == self.scroll_top {
                    let bottom = if self.scroll_bottom == usize::MAX { self.active_buffer().line_count().saturating_sub(1) as usize } else { self.scroll_bottom };
                    let buffer = self.active_buffer();
                    if let Some(mut start) = buffer.iter_at_line(self.scroll_top as i32) {
                        let start_offset = start.offset();
                        buffer.insert(&mut start, "\n");
                        buffer.remove_all_tags(&buffer.iter_at_offset(start_offset), &start);
                        if let Some(mut del_start) = buffer.iter_at_line((bottom + 1) as i32) {
                            let mut del_end = del_start.clone();
                            del_end.forward_visible_line();
                            buffer.delete(&mut del_start, &mut del_end);
                        }
                    }
                } else if cy > 0 {
                    let new_cy = cy - 1;
                    if self.is_alternate { self.alt_cursor_y = new_cy; } else { self.cursor_y = new_cy; }
                }
            }
            _ => {}
        }
        self.update_visual_cursor();
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        eprintln!("[OSC] osc_dispatch called, {} params, p[0]={:?}",
            params.len(),
            params.first().and_then(|p| std::str::from_utf8(p).ok()));
        if params.len() >= 2 {
            if params[0] == b"0" || params[0] == b"1" || params[0] == b"2" {
                // Rejoin for title too — title might contain semicolons
                let title_parts: Vec<&[u8]> = params[1..].to_vec();
                let title_bytes: Vec<u8> = title_parts.join(&b';');
                if let Ok(title) = std::str::from_utf8(&title_bytes) {
                    if let Some(lbl) = self.tab_label.upgrade() {
                        lbl.set_text(title);
                    }
                }
            } else if params[0] == b"8" {
                let url = if params.len() > 2 { std::str::from_utf8(params[2]).unwrap_or("") } else { "" };
                if !url.is_empty() {
                    self.current_tags.retain(|t| !t.starts_with("url:"));
                    self.current_tags.push(format!("url:{}", url));
                } else {
                    self.current_tags.retain(|t| !t.starts_with("url:"));
                }
            } else if params[0] == b"1337" {
                // VTE splits on ';', but our custom protocols use ';' as property
                // separator. Rejoin params[1..] to reconstruct the full payload.
                let payload_parts: Vec<&[u8]> = params[1..].to_vec();
                let payload_bytes: Vec<u8> = payload_parts.join(&b';');
                let payload = match std::str::from_utf8(&payload_bytes) {
                    Ok(s) => s.to_string(),
                    Err(_) => return,
                };

                if let Some(spec) = payload.strip_prefix("Window=") {
                    eprintln!("[WINDOW] OSC 1337 Window spec: {}", spec);
                    self.create_wm_window(spec);
                } else if let Some(spec) = payload.strip_prefix("WindowUpdate=") {
                    eprintln!("[WINDOW] OSC 1337 WindowUpdate spec: {}", spec);
                    self.update_wm_window(spec);
                } else if let Some(url) = payload.strip_prefix("OpenURL=") {
                    let url = url.trim().to_string();
                    eprintln!("[URL] Opening: {}", url);
                    if let Err(e) = gio::AppInfo::launch_default_for_uri(&url, None::<&gio::AppLaunchContext>) {
                        eprintln!("[URL] launch_default_for_uri error: {}", e);
                    }
                } else if let Some(spec) = payload.strip_prefix("FileDialog=") {
                    eprintln!("[FILE] FileDialog spec: {}", spec);
                    self.show_file_dialog(spec);
                } else if let Some(spec) = payload.strip_prefix("Notify=") {
                    let props = Self::parse_widget_props(spec);
                    let title = props.get("title").cloned().unwrap_or_else(|| "Notification".into());
                    let body  = props.get("body").cloned().unwrap_or_default();
                    let nid   = props.get("id").cloned().unwrap_or_else(|| "notif".into());
                    eprintln!("[NOTIFY] title={} body={}", title, body);
                    if let Some(app) = gio::Application::default() {
                        let notif = gio::Notification::new(&title);
                        if !body.is_empty() { notif.set_body(Some(&body)); }
                        app.send_notification(Some(&nid), &notif);
                    } else {
                        // Fallback: system command
                        #[cfg(target_os = "macos")]
                        { let _ = std::process::Command::new("osascript")
                            .arg("-e").arg(format!("display notification {:?} with title {:?}", body, title))
                            .spawn(); }
                        #[cfg(not(target_os = "macos"))]
                        { let _ = std::process::Command::new("notify-send")
                            .arg(&title).arg(&body).spawn(); }
                    }
                } else if let Some(spec) = payload.strip_prefix("Panel=") {
                    eprintln!("[PANEL] OSC 1337 Panel spec: {}", spec);
                    self.insert_panel(spec);
                } else if let Some(spec) = payload.strip_prefix("Alert=") {
                    eprintln!("[ALERT] OSC 1337 Alert spec: {}", spec);
                    self.show_alert(spec);
                } else if let Some(spec) = payload.strip_prefix("Toast=") {
                    eprintln!("[TOAST] OSC 1337 Toast spec: {}", spec);
                    self.show_toast(spec);
                } else if let Some(spec) = payload.strip_prefix("Confirm=") {
                    eprintln!("[CONFIRM] OSC 1337 Confirm spec: {}", spec);
                    self.show_confirm(spec);
                } else if let Some(spec) = payload.strip_prefix("Widget=") {
                    eprintln!("[WIDGET] OSC 1337 Widget spec: {}", spec);
                    self.insert_widget(spec);
                } else if let Some(spec) = payload.strip_prefix("WidgetUpdate=") {
                    eprintln!("[UPDATE] OSC 1337 WidgetUpdate spec: {}", spec);
                    self.update_widget(spec);
                } else if let Some(spec) = payload.strip_prefix("GetWidgetValue=") {
                    eprintln!("[GET] OSC 1337 GetWidgetValue spec: {}", spec);
                    self.get_widget_value(spec);
                } else if payload.starts_with("File=") {
                    eprintln!("[IMG] OSC 1337 received, payload len={}", payload.len());
                    if let Some(colon_pos) = payload.find(':') {
                        let b64 = &payload.as_bytes()[colon_pos + 1..];
                        eprintln!("[IMG] base64 slice len={}", b64.len());
                        match BASE64.decode(b64) {
                            Ok(data) => {
                                eprintln!("[IMG] decoded {} bytes, calling insert_image", data.len());
                                self.insert_image(data);
                            }
                            Err(e) => eprintln!("[IMG] base64 decode error: {}", e),
                        }
                    } else {
                        eprintln!("[IMG] no colon found in File= payload");
                    }
                }
            } else if params[0] == b"108" {
                if let Ok(data) = BASE64.decode(params[1]) {
                    self.insert_image(data);
                }
            }
        }
    }

    fn hook(&mut self, params: &vte::Params, intermediates: &[u8], _ignore: bool, action: char) {
        if action == 'q' && intermediates.is_empty() {
            self.is_sixel = true;
            self.image_buffer.clear();
            self.image_buffer.extend_from_slice(b"\x1bP");
            for (i, param) in params.iter().enumerate() {
                if i > 0 { self.image_buffer.push(b';'); }
                self.image_buffer.extend_from_slice(param[0].to_string().as_bytes());
            }
            self.image_buffer.push(b'q');
        }
    }

    fn put(&mut self, byte: u8) {
        if self.is_sixel {
            self.image_buffer.push(byte);
        }
    }

    fn unhook(&mut self) {
        if self.is_sixel {
            self.is_sixel = false;
            self.image_buffer.extend_from_slice(b"\x1b\\");
            if let Ok(image) = icy_sixel::SixelImage::decode(&self.image_buffer) {
                let width = image.width as i32;
                let height = image.height as i32;
                let bytes = gtk::glib::Bytes::from(&image.pixels);
                let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_bytes(
                    &bytes,
                    gtk::gdk_pixbuf::Colorspace::Rgb,
                    true,
                    8,
                    width,
                    height,
                    width * 4,
                );
                
                if let Some(tv) = self.view.upgrade() {
                    let cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
                    let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
                    let mut iter = self.ensure_cursor_position(cx, cy);
                    let anchor = tv.buffer().create_child_anchor(&mut iter);
                    let picture = gtk::Picture::for_pixbuf(&pixbuf);
                    picture.set_can_shrink(true);
                    tv.add_child_at_anchor(&picture, &anchor);
                }
            }
        }
    }
}
