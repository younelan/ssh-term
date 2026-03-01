use std::time::Duration;
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
    pub scroll_top: usize,
    pub scroll_bottom: usize,
    pub saved_cursor_x: usize,
    pub saved_cursor_y: usize,
    pub saved_tags: Vec<String>,
    pub bracketed_paste_mode: bool,
    pub char_width: f32,
    pub char_height: f32,
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
        
        let dim_tag = TextTag::new(Some("dim"));
        dim_tag.set_weight(300);
        tag_table.add(&dim_tag);
        
        let italic_tag = TextTag::new(Some("italic"));
        italic_tag.set_style(gtk::pango::Style::Italic);
        tag_table.add(&italic_tag);
        
        let inv_tag = TextTag::new(Some("inverse"));
        tag_table.add(&inv_tag);
        
        let und_tag = TextTag::new(Some("underline"));
        und_tag.set_underline(gtk::pango::Underline::Single);
        tag_table.add(&und_tag);
        
        let st_tag = TextTag::new(Some("strikethrough"));
        st_tag.set_strikethrough(true);
        tag_table.add(&st_tag);

        let link_tag = TextTag::new(Some("link"));
        link_tag.set_underline(gtk::pango::Underline::Single);
        tag_table.add(&link_tag);
        
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
            scroll_bottom: usize::MAX,
            saved_cursor_x: 0,
            saved_cursor_y: 0,
            saved_tags: Vec::new(),
            bracketed_paste_mode: false,
            char_width: 1.0,
            char_height: 1.0,
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
            let start_offset = end.offset();
            let newlines = "\n".repeat((cy + 1).saturating_sub(line_count));
            buffer.insert(&mut end, &newlines);
            buffer.remove_all_tags(&buffer.iter_at_offset(start_offset), &end);
        }
        
        let mut shadow_iter = buffer.iter_at_line(cy as i32).unwrap_or_else(|| buffer.end_iter());
        let mut current_offset = 0;
        
        while current_offset < cx {
            if shadow_iter.ends_line() || shadow_iter.is_end() {
                break;
            }
            shadow_iter.forward_char();
            current_offset += 1;
        }
        
        if current_offset < cx {
            let start_offset = shadow_iter.offset();
            let spaces = " ".repeat(cx - current_offset);
            buffer.insert(&mut shadow_iter, &spaces);
            buffer.remove_all_tags(&buffer.iter_at_offset(start_offset), &shadow_iter);
        }
        shadow_iter
    }

    pub fn update_palette(&mut self, palette: &[String]) {
        let tag_table = self.active_buffer().tag_table();
        let bg = palette.first().cloned().unwrap_or_else(|| "#000000".to_string());
        let fg = palette.get(7).cloned().unwrap_or_else(|| "#ffffff".to_string());
        
        if let Some(tag) = tag_table.lookup("inverse") {
            tag.set_foreground(Some(&bg));
            tag.set_background(Some(&fg));
        }

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
                2 => if !self.current_tags.contains(&"dim".to_string()) { self.current_tags.push("dim".to_string()); },
                3 => if !self.current_tags.contains(&"italic".to_string()) { self.current_tags.push("italic".to_string()); },
                4 => if !self.current_tags.contains(&"underline".to_string()) { self.current_tags.push("underline".to_string()); },
                7 => if !self.current_tags.contains(&"inverse".to_string()) { self.current_tags.push("inverse".to_string()); },
                9 => if !self.current_tags.contains(&"strikethrough".to_string()) { self.current_tags.push("strikethrough".to_string()); },
                22 => self.current_tags.retain(|t| t != "bold" && t != "dim"),
                23 => self.current_tags.retain(|t| t != "italic"),
                24 => self.current_tags.retain(|t| t != "underline"),
                27 => self.current_tags.retain(|t| t != "inverse"),
                29 => self.current_tags.retain(|t| t != "strikethrough"),
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

    pub fn update_visual_cursor(&self) {
        let buffer = self.active_buffer();
        let cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
        let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
        let iter = self.ensure_cursor_position(cx, cy);
        buffer.place_cursor(&iter);
    }

    pub fn bell(&mut self) {
        if let Some(tv) = self.view.upgrade() {
            let context = tv.style_context();
            context.add_class("bell-flash");
            glib::timeout_add_local(Duration::from_millis(100), move || {
                context.remove_class("bell-flash");
                glib::ControlFlow::Break
            });
        }
        if let Some(display) = gtk::gdk::Display::default() {
            display.beep();
        }
    }
}

use unicode_width::UnicodeWidthChar;

impl Perform for TerminalState {
    fn print(&mut self, c: char) {
        let width = c.width().unwrap_or(0);
        if width == 0 {
            let cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
            let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
            let mut iter = self.ensure_cursor_position(cx, cy);
            let buffer = self.active_buffer();
            buffer.insert(&mut iter, &c.to_string());
            return;
        }

        let cx;
        let cy;
        {
            cx = if self.is_alternate { self.alt_cursor_x } else { self.cursor_x };
            cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
        }

        let mut iter = self.ensure_cursor_position(cx, cy);
        let buffer = self.active_buffer();
        
        let mut del_iter = iter.clone();
        for _ in 0..width {
            if !del_iter.ends_line() {
                let mut next = del_iter.clone();
                next.forward_char();
                buffer.delete(&mut del_iter, &mut next);
            }
        }
        
        let start_offset = iter.offset();
        buffer.insert(&mut iter, &c.to_string());
        
        let start_iter = buffer.iter_at_offset(start_offset);
        buffer.remove_all_tags(&start_iter, &iter);
        
        if self.is_alternate {
            self.alt_cursor_x += width;
        } else {
            self.cursor_x += width;
        }
        
        let buffer = self.active_buffer();
        if !self.current_tags.is_empty() {
            for tag_name in &self.current_tags {
                if tag_name.starts_with("url:") {
                    if let Some(tag) = buffer.tag_table().lookup("link") {
                        buffer.apply_tag(&tag, &start_iter, &iter);
                    }
                    if buffer.tag_table().lookup(tag_name).is_none() {
                        let url_tag = TextTag::new(Some(tag_name));
                        buffer.tag_table().add(&url_tag);
                    }
                    if let Some(tag) = buffer.tag_table().lookup(tag_name) {
                        buffer.apply_tag(&tag, &start_iter, &iter);
                    }
                } else if let Some(tag) = buffer.tag_table().lookup(tag_name) {
                    buffer.apply_tag(&tag, &start_iter, &iter);
                }
            }
        }
        self.update_visual_cursor();
    }

    // Removal of broken trait hook

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
                if cy == self.scroll_bottom && self.scroll_bottom != usize::MAX {
                    let top = self.scroll_top;
                    let buffer = self.active_buffer();
                    if let Some(mut start) = buffer.iter_at_line(top as i32) {
                        let mut end = start.clone();
                        end.forward_visible_line();
                        buffer.delete(&mut start, &mut end);
                        let mut insert_iter = self.ensure_cursor_position(0, cy);
                        let start_offset = insert_iter.offset();
                        buffer.insert(&mut insert_iter, "\n");
                        buffer.remove_all_tags(&buffer.iter_at_offset(start_offset), &insert_iter);
                    }
                } else {
                    if self.is_alternate { self.alt_cursor_y += 1; } else { self.cursor_y += 1; }
                }
            }
            b'\r' => { if self.is_alternate { self.alt_cursor_x = 0; } else { self.cursor_x = 0; } }
            b'\x08' | b'\x7f' => {
                let cx = if self.is_alternate { &mut self.alt_cursor_x } else { &mut self.cursor_x };
                if *cx > 0 { *cx -= 1; }
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
                    7 => {
                        if let Some(v) = self.view.upgrade() {
                            v.set_wrap_mode(if c == 'h' { gtk::WrapMode::Char } else { gtk::WrapMode::None });
                        }
                    }
                    1000 => self.mouse_tracking_mode = if c == 'h' { 1000 } else { 0 },
                    1002 => self.mouse_tracking_mode = if c == 'h' { 1002 } else { 0 },
                    1006 => self.mouse_tracking_mode = if c == 'h' { 1006 } else { 0 },
                    1047 => {
                        if c == 'h' && !self.is_alternate {
                            self.is_alternate = true;
                            if let Some(v) = self.view.upgrade() { v.set_buffer(Some(&self.alternate_buffer)); }
                        } else if c == 'l' && self.is_alternate {
                            self.is_alternate = false;
                            if let Some(v) = self.view.upgrade() { v.set_buffer(Some(&self.primary_buffer)); }
                        }
                    }
                    1048 => {
                        if c == 'h' {
                            self.saved_cursor_x = self.cursor_x;
                            self.saved_cursor_y = self.cursor_y;
                            self.saved_tags = self.current_tags.clone();
                        } else if c == 'l' {
                            self.cursor_x = self.saved_cursor_x;
                            self.cursor_y = self.saved_cursor_y;
                            self.current_tags = self.saved_tags.clone();
                        }
                    }
                    1049 => {
                        if c == 'h' && !self.is_alternate {
                            self.saved_cursor_x = self.cursor_x;
                            self.saved_cursor_y = self.cursor_y;
                            self.saved_tags = self.current_tags.clone();
                            self.is_alternate = true;
                            self.alternate_buffer.set_text("");
                            self.alt_cursor_x = 0;
                            self.alt_cursor_y = 0;
                            if let Some(v) = self.view.upgrade() { v.set_buffer(Some(&self.alternate_buffer)); }
                        } else if c == 'l' && self.is_alternate {
                            self.is_alternate = false;
                            self.cursor_x = self.saved_cursor_x;
                            self.cursor_y = self.saved_cursor_y;
                            self.current_tags = self.saved_tags.clone();
                            if let Some(v) = self.view.upgrade() { v.set_buffer(Some(&self.primary_buffer)); }
                        }
                    }
                    2004 => self.bracketed_paste_mode = c == 'h',
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
                        if !end.ends_line() { end.forward_to_line_end(); }
                        buffer.delete(&mut iter, &mut end);
                    }
                    1 => {
                        if let Some(mut start) = buffer.iter_at_line(cy as i32) {
                            buffer.delete(&mut start, &mut iter);
                            let spaces = " ".repeat(cx);
                            buffer.insert(&mut start, &spaces);
                        }
                    }
                    2 => {
                        if let Some(mut start) = buffer.iter_at_line(cy as i32) {
                            let mut end = start.clone();
                            if !end.ends_line() { end.forward_to_line_end(); }
                            buffer.delete(&mut start, &mut end);
                        }
                    }
                    _ => {}
                }
            }
            'r' => {
                let top = arg0.max(1);
                let bottom = if arg1 == 0 { usize::MAX } else { arg1 };
                self.scroll_top = top - 1;
                self.scroll_bottom = if bottom == usize::MAX { usize::MAX } else { bottom - 1 };
                cx = 0;
                cy = 0;
            }
            '@' => {
                let count = arg0.max(1);
                let buffer = self.active_buffer();
                let mut iter = self.ensure_cursor_position(cx, cy);
                let spaces = " ".repeat(count);
                let start_offset = iter.offset();
                buffer.insert(&mut iter, &spaces);
                buffer.remove_all_tags(&buffer.iter_at_offset(start_offset), &iter);
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
                let bottom = if self.scroll_bottom == usize::MAX { buffer.line_count().saturating_sub(1) as usize } else { self.scroll_bottom };
                if cy <= bottom {
                    let mut iter = self.ensure_cursor_position(0, cy);
                    let newlines = "\n".repeat(count);
                    let start_offset = iter.offset();
                    buffer.insert(&mut iter, &newlines);
                    buffer.remove_all_tags(&buffer.iter_at_offset(start_offset), &iter);
                    
                    if let Some(mut del_start) = buffer.iter_at_line((bottom + 1) as i32) {
                        let mut del_end = del_start.clone();
                        for _ in 0..count {
                            if !del_end.is_end() { del_end.forward_visible_line(); }
                        }
                        buffer.delete(&mut del_start, &mut del_end);
                    }
                }
            }
            'M' => {
                let count = arg0.max(1);
                let buffer = self.active_buffer();
                let bottom = if self.scroll_bottom == usize::MAX { buffer.line_count().saturating_sub(1) as usize } else { self.scroll_bottom };
                if cy <= bottom {
                    if let Some(mut start) = buffer.iter_at_line(cy as i32) {
                        let mut end = start.clone();
                        for _ in 0..count {
                            if !end.is_end() { end.forward_visible_line(); }
                        }
                        if let Some(limit) = buffer.iter_at_line((bottom + 1) as i32) {
                            if end.offset() > limit.offset() { end = limit; }
                        }
                        buffer.delete(&mut start, &mut end);
                        
                        let mut insert_iter = self.ensure_cursor_position(0, bottom);
                        let newlines = "\n".repeat(count);
                        let start_offset = insert_iter.offset();
                        buffer.insert(&mut insert_iter, &newlines);
                        buffer.remove_all_tags(&buffer.iter_at_offset(start_offset), &insert_iter);
                    }
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
                let start_offset = insert_iter.offset();
                buffer.insert(&mut insert_iter, &spaces);
                buffer.remove_all_tags(&buffer.iter_at_offset(start_offset), &insert_iter);
            }
            'S' => {
                let count = arg0.max(1);
                let buffer = self.active_buffer();
                let top = self.scroll_top;
                let bottom = if self.scroll_bottom == usize::MAX { buffer.line_count().saturating_sub(1) as usize } else { self.scroll_bottom };
                
                if let Some(mut start) = buffer.iter_at_line(top as i32) {
                    let mut end = start.clone();
                    for _ in 0..count {
                        if !end.is_end() { end.forward_visible_line(); }
                    }
                    if let Some(limit) = buffer.iter_at_line((bottom + 1) as i32) {
                        if end.offset() > limit.offset() { end = limit; }
                    }
                    buffer.delete(&mut start, &mut end);
                    
                    let mut insert_iter = self.ensure_cursor_position(0, bottom);
                    let newlines = "\n".repeat(count);
                    let start_offset = insert_iter.offset();
                    buffer.insert(&mut insert_iter, &newlines);
                    buffer.remove_all_tags(&buffer.iter_at_offset(start_offset), &insert_iter);
                }
            }
            'T' => {
                let count = arg0.max(1);
                let buffer = self.active_buffer();
                let top = self.scroll_top;
                let bottom = if self.scroll_bottom == usize::MAX { buffer.line_count().saturating_sub(1) as usize } else { self.scroll_bottom };
                
                if let Some(mut start) = buffer.iter_at_line(top as i32) {
                    let newlines = "\n".repeat(count);
                    let start_offset = start.offset();
                    buffer.insert(&mut start, &newlines);
                    buffer.remove_all_tags(&buffer.iter_at_offset(start_offset), &start);
                    
                    if let Some(mut del_start) = buffer.iter_at_line((bottom + 1) as i32) {
                        let mut del_end = del_start.clone();
                        for _ in 0..count {
                            if !del_end.is_end() { del_end.forward_visible_line(); }
                        }
                        buffer.delete(&mut del_start, &mut del_end);
                    }
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
        self.update_visual_cursor();
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        if intermediates.is_empty() {
            match byte {
                b'7' => {
                    self.saved_cursor_x = self.cursor_x;
                    self.saved_cursor_y = self.cursor_y;
                    self.saved_tags = self.current_tags.clone();
                }
                b'8' => {
                    if !self.is_alternate {
                        self.cursor_x = self.saved_cursor_x;
                        self.cursor_y = self.saved_cursor_y;
                        self.current_tags = self.saved_tags.clone();
                    }
                }
                b'D' => {
                    let cy = if self.is_alternate { self.alt_cursor_y } else { self.cursor_y };
                    if cy == self.scroll_bottom && self.scroll_bottom != usize::MAX {
                        let top = self.scroll_top;
                        let buffer = self.active_buffer();
                        if let Some(mut start) = buffer.iter_at_line(top as i32) {
                            let mut end = start.clone();
                            end.forward_visible_line();
                            buffer.delete(&mut start, &mut end);
                            let mut insert_iter = self.ensure_cursor_position(0, cy);
                            buffer.insert(&mut insert_iter, "\n");
                        }
                    } else {
                        if self.is_alternate { self.alt_cursor_y += 1; } else { self.cursor_y += 1; }
                    }
                }
                b'M' => {
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
        }
        self.update_visual_cursor();
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
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
            }
        }
    }
}
