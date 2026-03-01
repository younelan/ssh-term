use base64::Engine as _;
const BASE64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::STANDARD;
use gtk4 as gtk;
use gtk::{glib, Label, TextBuffer, TextTag, TextView};
use gtk::prelude::*;
use vte::Perform;
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
        for part in spec.split(';') {
            if let Some((k, v)) = part.split_once(':') {
                map.insert(k.to_string(), v.to_string());
            }
        }
        map
    }

    /// Send a widget event response back to the PTY.
    fn send_widget_event(&self, id: &str, action: &str, value: &str) {
        if let Some(ref tx) = self.pty_input_tx {
            // Format: ESC ] 1337 ; WidgetEvent=id:ID;action:ACTION;value:VALUE ST
            let msg = format!("\x1b]1337;WidgetEvent=id:{};action:{};value:{}\x07", id, action, value);
            let _ = tx.send(msg.into_bytes());
        }
    }

    /// Insert a GTK widget into the text buffer at the current cursor position.
    /// `spec` is the part after "Widget=" e.g. "type:button;id:btn1;label:Click Me"
    pub fn insert_widget(&mut self, spec: &str) {
        let props = Self::parse_widget_props(spec);
        let widget_type = match props.get("type") {
            Some(t) => t.as_str(),
            None => { eprintln!("[WIDGET] no type specified"); return; }
        };
        let id = props.get("id").cloned().unwrap_or_else(|| "unnamed".to_string());

        let tv = match self.view.upgrade() {
            Some(tv) => tv,
            None => return,
        };

        let buffer = self.active_buffer();
        let cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
        let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
        let mut iter = self.ensure_cursor_position(cx, cy);
        let anchor = buffer.create_child_anchor(&mut iter);

        let pty_tx = self.pty_input_tx.clone();

        let widget: Option<gtk::Widget> = match widget_type {
            "button" => {
                let label = props.get("label").cloned().unwrap_or_else(|| "Button".into());
                let btn = gtk::Button::with_label(&label);
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
                let group = props.get("group").cloned().unwrap_or_default();
                let rb = gtk::CheckButton::with_label(&label);
                // For radio groups, look for an existing radio button with the same group tag
                // in the buffer's text anchors. For simplicity, we just create standalone radios;
                // grouping is done by the demo via the group property name echoed in events.
                let wid = id.clone();
                let tx = pty_tx.clone();
                let grp = group.clone();
                rb.connect_toggled(move |c| {
                    if let Some(ref tx) = tx {
                        let val = if c.is_active() { "true" } else { "false" };
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:toggled;value:{};group:{}\x07", wid, val, grp);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(rb.upcast())
            }
            "dropdown" | "combo" | "select" => {
                let items_str = props.get("items").cloned().unwrap_or_default();
                let items: Vec<&str> = items_str.split(',').collect();
                let dd = gtk::DropDown::from_strings(&items.iter().map(|s| *s).collect::<Vec<&str>>());
                dd.set_selected(0);
                let wid = id.clone();
                let tx = pty_tx.clone();
                let items_owned: Vec<String> = items.iter().map(|s| s.to_string()).collect();
                dd.connect_selected_notify(move |d| {
                    if let Some(ref tx) = tx {
                        let idx = d.selected() as usize;
                        let val = items_owned.get(idx).cloned().unwrap_or_default();
                        let msg = format!("\x1b]1337;WidgetEvent=id:{};action:selected;value:{}\x07", wid, val);
                        let _ = tx.send(msg.into_bytes());
                    }
                });
                Some(dd.upcast())
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
            "label" => {
                let text = props.get("text").cloned().unwrap_or_else(|| "Label".into());
                let lbl = gtk::Label::new(Some(&text));
                if let Some(css_class) = props.get("class") {
                    lbl.add_css_class(css_class);
                }
                Some(lbl.upcast())
            }
            "spinbutton" | "spin" => {
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
            _ => {
                eprintln!("[WIDGET] unknown type: {}", widget_type);
                None
            }
        };

        if let Some(w) = widget {
            w.set_visible(true);
            tv.add_child_at_anchor(&w, &anchor);
            if self.is_alternate { self.alt_cursor_x += 1; } else { self.cursor_x += 1; }
            eprintln!("[WIDGET] inserted {}(id={}) at ({}, {})", widget_type, id, cx, cy);
        }
    }

    pub fn insert_image(&mut self, data: Vec<u8>) {
        eprintln!("[IMG] insert_image called with {} bytes", data.len());
        if data.len() >= 4 {
            eprintln!("[IMG] first 4 bytes: {:02x} {:02x} {:02x} {:02x}",
                data[0], data[1], data[2], data[3]);
        }
        if let Some(tv) = self.view.upgrade() {
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
                if let Ok(title) = std::str::from_utf8(params[1]) {
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
            } else if params[0] == b"1337" && params[1].starts_with(b"Widget=") {
                // Widget embedding: OSC 1337 ; Widget=type:button;id:btn1;label:Click ST
                if let Ok(spec) = std::str::from_utf8(&params[1][7..]) {
                    eprintln!("[WIDGET] OSC 1337 Widget spec: {}", spec);
                    self.insert_widget(spec);
                }
            } else if params[0] == b"1337" && params[1].starts_with(b"File=") {
                eprintln!("[IMG] OSC 1337 received, params[1] len={}", params[1].len());
                if let Some(colon_pos) = params[1].iter().position(|&b| b == b':') {
                    let b64 = &params[1][colon_pos + 1..];
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
