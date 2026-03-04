use gtk4 as gtk;
use gtk::prelude::*;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::RcDom;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::default::Default;

pub mod css;
mod parser;
mod serializer;

// ── Undo Manager (snapshot-based) ─────────────────────────────────────────

/// Snapshot-based undo manager that saves/restores full HTML state.
/// GTK's built-in undo only tracks text insertions/deletions — it cannot
/// restore tag applications (bold, colors, CSS) or embedded widgets (tables,
/// images, HR).  By snapshotting the serialized HTML we get perfect undo of
/// everything the editor can represent.
struct UndoManager {
    /// Stack of HTML snapshots.  `current` indexes the active state.
    snapshots: Vec<String>,
    /// Index of the current state in `snapshots`.
    current: usize,
    /// Maximum number of undo levels to keep.
    max_levels: usize,
}

impl UndoManager {
    fn new(max_levels: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            current: 0,
            max_levels,
        }
    }

    /// Record initial state (e.g. after set_html).
    fn reset(&mut self, html: String) {
        self.snapshots.clear();
        self.snapshots.push(html);
        self.current = 0;
    }

    /// Push a new snapshot after user edits.  Discards any redo history.
    fn push(&mut self, html: String) {
        // Don't push if identical to current
        if let Some(cur) = self.snapshots.get(self.current) {
            if *cur == html {
                return;
            }
        }
        // Truncate any redo history beyond current position
        self.snapshots.truncate(self.current + 1);
        self.snapshots.push(html);
        self.current = self.snapshots.len() - 1;
        // Enforce max levels
        if self.snapshots.len() > self.max_levels {
            let excess = self.snapshots.len() - self.max_levels;
            self.snapshots.drain(0..excess);
            self.current = self.snapshots.len() - 1;
        }
    }

    fn can_undo(&self) -> bool {
        self.current > 0
    }

    fn can_redo(&self) -> bool {
        self.current + 1 < self.snapshots.len()
    }

    fn undo(&mut self) -> Option<&str> {
        if self.can_undo() {
            self.current -= 1;
            Some(&self.snapshots[self.current])
        } else {
            None
        }
    }

    fn redo(&mut self) -> Option<&str> {
        if self.can_redo() {
            self.current += 1;
            Some(&self.snapshots[self.current])
        } else {
            None
        }
    }
}

// ── NativeHtmlEditor ───────────────────────────────────────────────────────

pub struct NativeHtmlEditor {
    view: gtk::TextView,
    /// Per-editor store mapping css_* tag names → original CSS strings for serialization.
    css_rules_store: RefCell<HashMap<String, String>>,
    /// Snapshot-based undo/redo manager.
    undo_mgr: RefCell<UndoManager>,
    /// Guard flag to prevent recording snapshots while restoring undo/redo state.
    restoring: Cell<bool>,
    /// Internal clipboard: stores HTML from the last copy/cut operation.
    internal_clipboard: RefCell<Option<String>>,
    /// Guard: true when we just set the system clipboard ourselves.
    clipboard_set_by_us: Cell<bool>,
    /// Maps normal CSS tag name → hover variant CSS tag name (from :hover rules).
    hover_variants: RefCell<HashMap<String, String>>,
    /// Hover tag name for link: tags (from a:hover rules).
    link_hover_tag: RefCell<Option<String>>,
    /// Currently applied hover tags with their ranges (for removal on motion/leave).
    active_hovers: RefCell<Vec<(String, gtk::TextMark, gtk::TextMark)>>,
}

impl NativeHtmlEditor {
    pub fn new() -> Self {
        let view = gtk::TextView::new();
        view.set_wrap_mode(gtk::WrapMode::Word);
        view.set_left_margin(8);
        view.set_right_margin(8);
        view.set_top_margin(8);
        view.set_bottom_margin(8);

        let mut tab_array = gtk::pango::TabArray::new(10, true);
        for i in 1..10 {
            tab_array.set_tab(i - 1, gtk::pango::TabAlign::Left, i * 150);
        }
        view.set_tabs(&tab_array);

        let buffer = view.buffer();
        // Disable GTK's built-in undo — it only tracks text, not tags or widgets.
        buffer.set_enable_undo(false);
        setup_tags(&buffer);
        Self {
            view,
            css_rules_store: RefCell::new(HashMap::new()),
            undo_mgr: RefCell::new(UndoManager::new(100)),
            restoring: Cell::new(false),
            internal_clipboard: RefCell::new(None),
            clipboard_set_by_us: Cell::new(false),
            hover_variants: RefCell::new(HashMap::new()),
            link_hover_tag: RefCell::new(None),
            active_hovers: RefCell::new(Vec::new()),
        }
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.view.upcast_ref()
    }

    /// Connect buffer signals to automatically capture undo snapshots
    /// after user edits, and set up clipboard handling for HTML copy/paste.
    /// Must be called with an Rc<Self> after construction.
    pub fn connect_undo_signals(self: &std::rc::Rc<Self>) {
        // Snapshot capture after each user edit (typing, deleting, pasting)
        let editor = std::rc::Rc::downgrade(self);
        self.view.buffer().connect_end_user_action(move |_buf| {
            if let Some(ed) = editor.upgrade() {
                ed.capture_undo_snapshot();
            }
        });

        // Clear internal clipboard when system clipboard changes externally
        let ed_weak = std::rc::Rc::downgrade(self);
        self.view.clipboard().connect_changed(move |_| {
            if let Some(ed) = ed_weak.upgrade() {
                if ed.clipboard_set_by_us.get() {
                    ed.clipboard_set_by_us.set(false);
                } else {
                    *ed.internal_clipboard.borrow_mut() = None;
                }
            }
        });

        // Key controller: intercept Ctrl/Cmd+C/X/V for HTML-aware clipboard
        let key_ctrl = gtk::EventControllerKey::new();
        key_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
        let ed_weak = std::rc::Rc::downgrade(self);
        key_ctrl.connect_key_pressed(move |_ctrl, key, _code, modifiers| {
            let primary = modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                || modifiers.contains(gtk::gdk::ModifierType::META_MASK);
            if !primary {
                return gtk::glib::Propagation::Proceed;
            }
            let Some(ed) = ed_weak.upgrade() else {
                return gtk::glib::Propagation::Proceed;
            };
            let shift = modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK);
            match key {
                gtk::gdk::Key::z | gtk::gdk::Key::Z => {
                    if shift {
                        ed.redo();
                    } else {
                        ed.undo();
                    }
                    gtk::glib::Propagation::Stop
                }
                gtk::gdk::Key::y | gtk::gdk::Key::Y => {
                    ed.redo();
                    gtk::glib::Propagation::Stop
                }
                gtk::gdk::Key::c | gtk::gdk::Key::C => {
                    ed.copy_selection_as_html();
                    gtk::glib::Propagation::Stop
                }
                gtk::gdk::Key::x | gtk::gdk::Key::X => {
                    ed.cut_selection_as_html();
                    gtk::glib::Propagation::Stop
                }
                gtk::gdk::Key::v | gtk::gdk::Key::V => {
                    if ed.paste_html_from_internal() {
                        gtk::glib::Propagation::Stop
                    } else {
                        gtk::glib::Propagation::Proceed
                    }
                }
                _ => gtk::glib::Propagation::Proceed,
            }
        });
        self.view.add_controller(key_ctrl);

        // Clickable links (mailto:, tel:, http:, https:)
        let click_ctrl = gtk::GestureClick::new();
        click_ctrl.set_button(1); // Left click only
        let ed_weak = std::rc::Rc::downgrade(self);
        click_ctrl.connect_released(move |gesture, _n_press, x, y| {
            let Some(_ed) = ed_weak.upgrade() else { return };
            let Some(widget) = gesture.widget() else { return };
            let Some(view) = widget.downcast_ref::<gtk::TextView>() else { return };
            let (bx, by) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
            if let Some(iter) = view.iter_at_location(bx, by) {
                for tag in iter.tags().iter() {
                    if let Some(name) = tag.name() {
                        if let Some(url) = name.strip_prefix("link:") {
                            if url.starts_with("mailto:")
                                || url.starts_with("tel:")
                                || url.starts_with("http:")
                                || url.starts_with("https:")
                            {
                                let launcher = gtk::UriLauncher::new(url);
                                launcher.launch(
                                    gtk::Window::NONE,
                                    gtk::gio::Cancellable::NONE,
                                    |_| {},
                                );
                            }
                            break;
                        }
                    }
                }
            }
        });
        self.view.add_controller(click_ctrl);

        // Tooltip for <abbr title="..."> elements
        self.view.set_has_tooltip(true);
        let ed_weak = std::rc::Rc::downgrade(self);
        self.view.connect_query_tooltip(move |view, x, y, _keyboard, tooltip| {
            let Some(_ed) = ed_weak.upgrade() else {
                return false;
            };
            // Convert widget coords to buffer coords
            let (bx, by) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, x, y);
            if let Some(iter) = view.iter_at_location(bx, by) {
                for tag in iter.tags().iter() {
                    if let Some(name) = tag.name() {
                        if let Some(title) = name.strip_prefix("abbr_title:") {
                            tooltip.set_text(Some(title));
                            return true;
                        }
                    }
                }
            }
            false
        });

        // Hover support for CSS :hover rules
        let motion_ctrl = gtk::EventControllerMotion::new();
        let ed_weak = std::rc::Rc::downgrade(self);
        motion_ctrl.connect_motion(move |_ctrl, x, y| {
            if let Some(ed) = ed_weak.upgrade() {
                ed.update_hover(x, y);
            }
        });
        let ed_weak = std::rc::Rc::downgrade(self);
        motion_ctrl.connect_leave(move |_ctrl| {
            if let Some(ed) = ed_weak.upgrade() {
                ed.clear_hover();
            }
        });
        self.view.add_controller(motion_ctrl);
    }

    fn clear_hover(&self) {
        let buffer = self.view.buffer();
        let active = self.active_hovers.borrow().clone();
        for (tag_name, start_mark, end_mark) in active.iter() {
            if let Some(tag) = buffer.tag_table().lookup(tag_name) {
                let start = buffer.iter_at_mark(start_mark);
                let end = buffer.iter_at_mark(end_mark);
                buffer.remove_tag(&tag, &start, &end);
            }
            buffer.delete_mark(start_mark);
            buffer.delete_mark(end_mark);
        }
        self.active_hovers.borrow_mut().clear();
    }

    fn update_hover(&self, x: f64, y: f64) {
        self.clear_hover();

        let hover_variants = self.hover_variants.borrow();
        let link_hover = self.link_hover_tag.borrow();
        if hover_variants.is_empty() && link_hover.is_none() {
            return;
        }

        let (bx, by) = self.view.window_to_buffer_coords(
            gtk::TextWindowType::Widget, x as i32, y as i32,
        );
        let Some(iter) = self.view.iter_at_location(bx, by) else { return };

        let buffer = self.view.buffer();

        for tag in iter.tags().iter() {
            let Some(name) = tag.name() else { continue };
            let name_str = name.to_string();

            // Check CSS tag hover variants, or link:* → a:hover
            let hover_tag_name = if let Some(ht) = hover_variants.get(&name_str) {
                Some(ht.clone())
            } else if name_str.starts_with("link:") {
                link_hover.clone()
            } else {
                None
            };

            if let Some(hover_name) = hover_tag_name {
                if let Some(hover_tag) = buffer.tag_table().lookup(&hover_name) {
                    // Find the range of this tag around the cursor
                    let mut start = iter;
                    let mut end = iter;
                    start.backward_to_tag_toggle(Some(tag));
                    end.forward_to_tag_toggle(Some(tag));

                    // Apply hover variant tag to that range
                    buffer.apply_tag(&hover_tag, &start, &end);

                    let start_mark = buffer.create_mark(None, &start, true);
                    let end_mark = buffer.create_mark(None, &end, false);
                    self.active_hovers.borrow_mut().push((
                        hover_name,
                        start_mark,
                        end_mark,
                    ));
                }
            }
        }
    }

    // ── HTML I/O ───────────────────────────────────────────────────────────

    /// Internal helper: parse HTML into the buffer without touching the undo stack.
    fn load_html_internal(&self, html: &str) {
        let buffer = self.view.buffer();
        buffer.set_text("");

        let dom = parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .read_from(&mut html.as_bytes())
            .unwrap();

        let result = parser::parse_html_to_buffer(&self.view, &dom, &buffer);
        *self.css_rules_store.borrow_mut() = result.css_rules_store;
        *self.hover_variants.borrow_mut() = result.hover_variants;
        *self.link_hover_tag.borrow_mut() = result.link_hover_tag;
    }

    pub fn set_html(&self, html: &str) {
        self.load_html_internal(html);

        // Snapshot initial state for undo
        let initial_html = serializer::serialize_buffer(
            &self.view.buffer(),
            &self.css_rules_store.borrow(),
        );
        self.undo_mgr.borrow_mut().reset(initial_html);
    }

    pub fn get_html(&self) -> String {
        serializer::serialize_buffer(&self.view.buffer(), &self.css_rules_store.borrow())
    }

    // ── Clipboard (HTML-aware copy/cut/paste) ───────────────────────────────

    fn copy_selection_as_html(&self) {
        let buffer = self.view.buffer();
        let Some((start, end)) = buffer.selection_bounds() else {
            return;
        };
        let html = serializer::serialize_range(
            &buffer,
            &start,
            &end,
            &self.css_rules_store.borrow(),
        );
        *self.internal_clipboard.borrow_mut() = Some(html.clone());

        // Put both HTML and plain text on the system clipboard
        let plain_text = buffer.text(&start, &end, false).to_string();
        let html_bytes = gtk::glib::Bytes::from_owned(html.into_bytes());
        let text_bytes = gtk::glib::Bytes::from_owned(plain_text.into_bytes());
        let providers = [
            gtk::gdk::ContentProvider::for_bytes("text/html", &html_bytes),
            gtk::gdk::ContentProvider::for_bytes("text/plain;charset=utf-8", &text_bytes),
        ];
        let union = gtk::gdk::ContentProvider::new_union(&providers);
        self.clipboard_set_by_us.set(true);
        let _ = self.view.clipboard().set_content(Some(&union));
    }

    fn cut_selection_as_html(&self) {
        self.copy_selection_as_html();
        let buffer = self.view.buffer();
        if let Some((mut start, mut end)) = buffer.selection_bounds() {
            buffer.delete(&mut start, &mut end);
        }
        self.capture_undo_snapshot();
    }

    /// Paste from internal HTML clipboard. Returns true if handled.
    fn paste_html_from_internal(&self) -> bool {
        let clip_html = self.internal_clipboard.borrow().clone();
        let Some(pasted_html) = clip_html else {
            return false;
        };

        let buffer = self.view.buffer();

        // Delete selection if any
        if let Some((mut start, mut end)) = buffer.selection_bounds() {
            buffer.delete(&mut start, &mut end);
        }

        // Serialize everything before and after the cursor
        let cursor = buffer.iter_at_mark(&buffer.get_insert());
        let buf_start = buffer.start_iter();
        let buf_end = buffer.end_iter();
        let store = self.css_rules_store.borrow();
        let before = serializer::serialize_range(&buffer, &buf_start, &cursor, &store);
        let after = serializer::serialize_range(&buffer, &cursor, &buf_end, &store);
        drop(store);

        // Combine and re-render
        let combined = format!("{}{}{}", before, pasted_html, after);
        self.load_html_internal(&combined);
        self.capture_undo_snapshot();
        true
    }

    // ── Formatting Toggles ─────────────────────────────────────────────────

    pub fn toggle_bold(&self) {
        self.toggle_tag("b");
    }

    pub fn toggle_italic(&self) {
        self.toggle_tag("i");
    }

    pub fn toggle_underline(&self) {
        self.toggle_tag("u");
    }

    pub fn toggle_size(&self) {
        self.toggle_tag("h2");
    }

    fn toggle_tag(&self, tag_name: &str) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);
        if let Some(tag) = buffer.tag_table().lookup(tag_name) {
            if start.has_tag(&tag) {
                buffer.remove_tag(&tag, &start, &end);
            } else {
                buffer.apply_tag(&tag, &start, &end);
            }
            self.capture_undo_snapshot();
        }
    }

    // ── Alignment ──────────────────────────────────────────────────────────

    pub fn apply_alignment_tag(&self, tag_name: &str) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);
        for name in ["align_center", "align_right", "align_justify"] {
            if let Some(t) = buffer.tag_table().lookup(name) {
                buffer.remove_tag(&t, &start, &end);
            }
        }
        if !tag_name.is_empty()
            && let Some(t) = buffer.tag_table().lookup(tag_name) {
                buffer.apply_tag(&t, &start, &end);
            }
        self.capture_undo_snapshot();
    }

    pub fn align_left(&self) { self.apply_alignment_tag(""); }
    pub fn align_center(&self) { self.apply_alignment_tag("align_center"); }
    pub fn align_right(&self) { self.apply_alignment_tag("align_right"); }
    pub fn align_justify(&self) { self.apply_alignment_tag("align_justify"); }

    // ── Indentation ────────────────────────────────────────────────────────

    pub fn increase_indent(&self) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);
        if let Some(tag) = buffer.tag_table().lookup("blockquote") {
            buffer.apply_tag(&tag, &start, &end);
            self.capture_undo_snapshot();
        }
    }

    pub fn decrease_indent(&self) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);
        if let Some(tag) = buffer.tag_table().lookup("blockquote") {
            buffer.remove_tag(&tag, &start, &end);
            self.capture_undo_snapshot();
        }
    }

    // ── Undo/Redo (snapshot-based) ──────────────────────────────────────────

    /// Capture current buffer state as an undo snapshot.
    /// Call this after any user action that modifies the buffer.
    pub fn capture_undo_snapshot(&self) {
        if self.restoring.get() {
            return;
        }
        let html = serializer::serialize_buffer(&self.view.buffer(), &self.css_rules_store.borrow());
        self.undo_mgr.borrow_mut().push(html);
    }

    /// Restore the buffer from an HTML string (used by undo/redo).
    fn restore_html(&self, html: &str) {
        self.restoring.set(true);
        self.load_html_internal(html);
        self.restoring.set(false);
    }

    pub fn undo(&self) {
        let html = self.undo_mgr.borrow_mut().undo().map(|s| s.to_string());
        if let Some(html) = html {
            self.restore_html(&html);
        }
    }

    pub fn redo(&self) {
        let html = self.undo_mgr.borrow_mut().redo().map(|s| s.to_string());
        if let Some(html) = html {
            self.restore_html(&html);
        }
    }

    // ── Horizontal Rule ────────────────────────────────────────────────────

    pub fn insert_hr(&self) {
        let buffer = self.view.buffer();
        let mut iter = buffer.iter_at_mark(&buffer.get_insert());
        if iter.offset() > 0 && !iter.starts_line() {
            buffer.insert(&mut iter, "\n");
        }

        let anchor = buffer.create_child_anchor(&mut iter);
        let hr_line = gtk::Separator::new(gtk::Orientation::Horizontal);
        hr_line.set_margin_top(8);
        hr_line.set_margin_bottom(8);
        hr_line.set_hexpand(true);
        hr_line.set_halign(gtk::Align::Fill);
        hr_line.set_size_request(400, -1);
        hr_line.set_widget_name("hr_rule");

        #[allow(deprecated)]
        {
            let provider = gtk::CssProvider::new();
            provider.load_from_data("separator#hr_rule { min-height: 2px; background-color: black; }");
            hr_line.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }

        self.view.add_child_at_anchor(&hr_line, &anchor);
        self.capture_undo_snapshot();
    }

    // ── Image ──────────────────────────────────────────────────────────────

    /// Insert an image from a file path.
    pub fn insert_image(&self, path: &str) {
        let file = gtk::gio::File::for_path(path);
        self.insert_image_from_file(&file);
    }

    /// Insert an image from a gio::File (e.g. from a FileDialog).
    pub fn insert_image_from_file(&self, file: &gtk::gio::File) {
        let buffer = self.view.buffer();
        let mut iter = buffer.iter_at_mark(&buffer.get_insert());

        let path_str = file.path().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

        // Load via Texture for proper error handling
        let picture = match gtk::gdk::Texture::from_file(file) {
            Ok(texture) => {
                let pic = gtk::Picture::for_paintable(&texture);
                // Constrain width while preserving aspect ratio
                let natural_w = texture.width();
                let natural_h = texture.height();
                let max_w = 600;
                let (display_w, display_h) = if natural_w > max_w {
                    let scale = max_w as f64 / natural_w as f64;
                    (max_w, (natural_h as f64 * scale) as i32)
                } else {
                    (natural_w, natural_h)
                };
                pic.set_size_request(display_w, display_h);
                pic
            }
            Err(_) => {
                // Fallback: try Picture::for_file which handles more formats
                let pic = gtk::Picture::for_file(file);
                pic.set_size_request(400, -1);
                pic
            }
        };

        picture.set_can_shrink(true);
        picture.set_halign(gtk::Align::Start);
        picture.set_widget_name(&format!("img:{}", path_str));

        let anchor = buffer.create_child_anchor(&mut iter);
        self.view.add_child_at_anchor(&picture, &anchor);
        self.capture_undo_snapshot();
    }

    // ── Color ──────────────────────────────────────────────────────────────

    pub fn apply_color(&self, color: &str) {
        let buffer = self.view.buffer();
        if let Some((start, end)) = buffer.selection_bounds() {
            let tag_name = format!("color: {}", color);
            let tag = if let Some(t) = buffer.tag_table().lookup(&tag_name) {
                t
            } else {
                let new_tag = gtk::TextTag::builder().name(&tag_name).foreground(color).build();
                buffer.tag_table().add(&new_tag);
                new_tag
            };
            buffer.apply_tag(&tag, &start, &end);
            self.capture_undo_snapshot();
        }
    }

    // ── Bullet List ────────────────────────────────────────────────────────

    pub fn insert_bullet(&self) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);

        let start_line = start.line();
        let end_line = end.line();

        let tag_table = buffer.tag_table();
        let list_marker_tag = tag_table.lookup("list_marker").unwrap();
        let li_tag = tag_table.lookup("li").unwrap();
        let ul_tag = tag_table.lookup("ul_1").unwrap();

        let first_line_start = buffer.iter_at_line(start_line).unwrap_or_else(|| buffer.start_iter());
        let first_line_check = buffer.iter_at_offset(first_line_start.offset() + 2);
        let text_check1 = buffer.text(&first_line_start, &first_line_check, false);
        let toggling_off = first_line_start.has_tag(&list_marker_tag)
            || first_line_check.has_tag(&list_marker_tag)
            || text_check1.starts_with("\u{2022} ");

        for line_num in (start_line..=end_line).rev() {
            let mut line_start = buffer.iter_at_line(line_num).unwrap_or_else(|| buffer.start_iter());

            let check_end = buffer.iter_at_offset(line_start.offset() + 2);
            let text_check2 = buffer.text(&line_start, &check_end, false);
            let has_bullet = line_start.has_tag(&list_marker_tag)
                || check_end.has_tag(&list_marker_tag)
                || text_check2.starts_with("\u{2022} ");

            if toggling_off {
                if has_bullet {
                    let mut marker_end = line_start;
                    marker_end.forward_chars(2);
                    buffer.delete(&mut line_start, &mut marker_end);

                    let mut new_end = line_start;
                    if !new_end.ends_line() { new_end.forward_to_line_end(); }
                    buffer.remove_tag(&li_tag, &line_start, &new_end);
                    buffer.remove_tag(&ul_tag, &line_start, &new_end);
                }
            } else if !has_bullet {
                let start_offset = line_start.offset();
                buffer.insert(&mut line_start, "\u{2022} ");

                let bullet_start = buffer.iter_at_offset(start_offset);
                let marker_end = buffer.iter_at_offset(start_offset + 1);
                buffer.apply_tag(&list_marker_tag, &bullet_start, &marker_end);

                let mut new_end = bullet_start;
                if !new_end.ends_line() { new_end.forward_to_line_end(); }
                buffer.apply_tag(&li_tag, &bullet_start, &new_end);
                buffer.apply_tag(&ul_tag, &bullet_start, &new_end);
            }
        }
        self.capture_undo_snapshot();
    }
}

impl Default for NativeHtmlEditor {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tag Setup (shared between main editor and table cell buffers) ──────────

pub fn setup_tags(buffer: &gtk::TextBuffer) {
    create_tag_weight(buffer, "b", 700);
    create_tag_weight(buffer, "strong", 700);
    create_tag_style(buffer, "i", gtk::pango::Style::Italic);
    create_tag_style(buffer, "em", gtk::pango::Style::Italic);
    create_tag_style(buffer, "cite", gtk::pango::Style::Italic);
    create_tag_style(buffer, "var", gtk::pango::Style::Italic);

    let u_tag = gtk::TextTag::builder().name("u").underline(gtk::pango::Underline::Single).build();
    buffer.tag_table().add(&u_tag);
    let s_tag = gtk::TextTag::builder().name("s").strikethrough(true).build();
    buffer.tag_table().add(&s_tag);

    create_tag_weight_scale(buffer, "h1", 700, 2.0);
    create_tag_weight_scale(buffer, "h2", 700, 1.5);
    create_tag_weight_scale(buffer, "h3", 700, 1.17);
    create_tag_weight_scale(buffer, "h4", 700, 1.0);
    create_tag_weight_scale(buffer, "h5", 700, 0.83);
    create_tag_weight_scale(buffer, "h6", 700, 0.67);

    let sub_tag = gtk::TextTag::builder().name("sub").scale(0.75).rise(-3000).build();
    buffer.tag_table().add(&sub_tag);
    let sup_tag = gtk::TextTag::builder().name("sup").scale(0.75).rise(5000).build();
    buffer.tag_table().add(&sup_tag);
    let small_tag = gtk::TextTag::builder().name("small").scale(0.83).build();
    buffer.tag_table().add(&small_tag);
    let big_tag = gtk::TextTag::builder().name("big").scale(1.17).build();
    buffer.tag_table().add(&big_tag);
    let mark_tag = gtk::TextTag::builder().name("mark").background("yellow").foreground("black").build();
    buffer.tag_table().add(&mark_tag);

    let pre_tag = gtk::TextTag::builder()
        .name("pre")
        .family("monospace")
        .paragraph_background("#f6f8fa")
        .left_margin(12)
        .right_margin(12)
        .pixels_above_lines(4)
        .pixels_below_lines(4)
        .wrap_mode(gtk::WrapMode::None)
        .build();
    buffer.tag_table().add(&pre_tag);
    let code_tag = gtk::TextTag::builder()
        .name("code")
        .family("monospace")
        .background("#f0f0f0")
        .build();
    buffer.tag_table().add(&code_tag);

    // Blockquote: left border effect via indent + paragraph background
    let bq_tag = gtk::TextTag::builder()
        .name("blockquote")
        .left_margin(20)
        .indent(-4)
        .right_margin(20)
        .pixels_above_lines(4)
        .pixels_below_lines(4)
        .paragraph_background("#f9f9f9")
        .build();
    buffer.tag_table().add(&bq_tag);

    // Depth-level blockquote tags for nested email reply chains
    let bq_colors = ["#4a90d9", "#6ab04c", "#e67e22", "#9b59b6", "#e74c3c"];
    let bq_bg = ["#f0f4ff", "#f4fff0", "#fff4f0", "#f8f0ff", "#fff8f0"];
    for depth in 1..=5usize {
        let margin = 8 + depth as i32 * 16;
        let tag = gtk::TextTag::builder()
            .name(format!("blockquote_{}", depth))
            .left_margin(margin)
            .indent(-4)
            .paragraph_background(bq_bg[depth - 1])
            .pixels_above_lines(2)
            .pixels_below_lines(2)
            .build();
        buffer.tag_table().add(&tag);
        // Left-border color tag applied separately for the border visual
        let border_tag = gtk::TextTag::builder()
            .name(format!("bq_border_{}", depth))
            .foreground(bq_colors[depth - 1])
            .build();
        buffer.tag_table().add(&border_tag);
    }

    let align_center = gtk::TextTag::builder().name("align_center").justification(gtk::Justification::Center).build();
    buffer.tag_table().add(&align_center);
    let align_right = gtk::TextTag::builder().name("align_right").justification(gtk::Justification::Right).build();
    buffer.tag_table().add(&align_right);
    let align_justify = gtk::TextTag::builder().name("align_justify").justification(gtk::Justification::Fill).build();
    buffer.tag_table().add(&align_justify);

    let hr_tag = gtk::TextTag::builder().name("hr").strikethrough(true).build();
    buffer.tag_table().add(&hr_tag);

    for depth in 1..=5 {
        let margin = depth * 20;
        let ul_tag = gtk::TextTag::builder().name(format!("ul_{}", depth)).left_margin(margin).build();
        buffer.tag_table().add(&ul_tag);
        let ol_tag = gtk::TextTag::builder().name(format!("ol_{}", depth)).left_margin(margin).build();
        buffer.tag_table().add(&ol_tag);
    }
    let li_tag = gtk::TextTag::builder().name("li").build();
    buffer.tag_table().add(&li_tag);
    let list_marker_tag = gtk::TextTag::builder().name("list_marker").build();
    buffer.tag_table().add(&list_marker_tag);

    let a_tag = gtk::TextTag::builder().name("a").foreground("blue").underline(gtk::pango::Underline::Single).build();
    buffer.tag_table().add(&a_tag);
}

fn create_tag_weight(buffer: &gtk::TextBuffer, name: &str, weight: i32) {
    let tag = gtk::TextTag::new(Some(name));
    tag.set_property("weight", weight);
    buffer.tag_table().add(&tag);
}

fn create_tag_style(buffer: &gtk::TextBuffer, name: &str, style: gtk::pango::Style) {
    let tag = gtk::TextTag::new(Some(name));
    tag.set_property("style", style);
    buffer.tag_table().add(&tag);
}

fn create_tag_weight_scale(buffer: &gtk::TextBuffer, name: &str, weight: i32, scale: f64) {
    let tag = gtk::TextTag::new(Some(name));
    tag.set_property("weight", weight);
    tag.set_property("scale", scale);
    buffer.tag_table().add(&tag);
}

fn get_target_bounds(buffer: &gtk::TextBuffer) -> (gtk::TextIter, gtk::TextIter) {
    if let Some((start, end)) = buffer.selection_bounds() {
        (start, end)
    } else {
        let mut start = buffer.iter_at_mark(&buffer.get_insert());
        let mut end = start;
        start.set_line_offset(0);
        if !end.ends_line() {
            end.forward_to_line_end();
        }
        if start == end {
            let mut ins = end;
            buffer.insert(&mut ins, "\u{200B}");
            start.set_line_offset(0);
            end = ins;
        }
        (start, end)
    }
}
