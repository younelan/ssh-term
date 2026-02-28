use gtk4 as gtk;
use gtk::{glib, Label, TextBuffer, TextTag, TextView};
use gtk::prelude::*;
use vte::Perform;

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
}

impl TerminalState {
    pub fn new(view: glib::WeakRef<TextView>, tab_label: glib::WeakRef<Label>, palette: Vec<String>) -> Self {
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
            let color = crate::config::get_256_color(i, &palette);
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

    pub fn active_buffer(&self) -> &TextBuffer {
        if self.is_alternate { &self.alternate_buffer } else { &self.primary_buffer }
    }

    pub fn ensure_cursor_position(&self, cx: usize, cy: usize) -> gtk::TextIter {
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
        
        for i in 0..=255 {
            let color = crate::config::get_256_color(i, palette);
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
