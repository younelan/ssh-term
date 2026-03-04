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
    /// Readonly mode: None = fully editable, Some(vec) = readonly except these IDs.
    readonly_ids: RefCell<Option<Vec<String>>>,
    /// CSS rules from <style> blocks (selector → declarations), for class manipulation.
    style_rules: RefCell<HashMap<String, String>>,
    /// Hover CSS rules from <style> blocks.
    hover_style_rules: RefCell<HashMap<String, String>>,
    /// Per-ID element metadata (tag, classes, inline_style) for class manipulation.
    element_meta: RefCell<HashMap<String, parser::ElementMeta>>,
    /// Per-ID CssProvider references for live widget restyling.
    element_providers: RefCell<HashMap<String, gtk::CssProvider>>,
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
            readonly_ids: RefCell::new(None),
            style_rules: RefCell::new(HashMap::new()),
            hover_style_rules: RefCell::new(HashMap::new()),
            element_meta: RefCell::new(HashMap::new()),
            element_providers: RefCell::new(HashMap::new()),
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
        *self.style_rules.borrow_mut() = result.style_rules;
        *self.hover_style_rules.borrow_mut() = result.hover_rules;
        *self.element_meta.borrow_mut() = result.element_meta;
        *self.element_providers.borrow_mut() = result.element_providers;
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

    /// Get the inner HTML content of an element by its `id` attribute.
    /// Returns `None` if no element with that ID exists in the buffer.
    pub fn get_html_by_id(&self, id: &str) -> Option<String> {
        let buffer = self.view.buffer();
        let tag_name = format!("editable_id:{}", id);
        let id_tag = buffer.tag_table().lookup(&tag_name)?;

        // Find the first region tagged with this ID
        let mut iter = buffer.start_iter();
        loop {
            if iter.starts_tag(Some(&id_tag)) {
                let start = iter;
                if iter.forward_to_tag_toggle(Some(&id_tag)) {
                    return Some(serializer::serialize_range(
                        &buffer, &start, &iter, &self.css_rules_store.borrow(),
                    ));
                }
                break;
            }
            if !iter.forward_to_tag_toggle(Some(&id_tag)) { break; }
        }
        None
    }

    /// Replace the inner HTML content of an element by its `id` attribute.
    /// Returns `true` if the element was found and replaced, `false` otherwise.
    pub fn set_html_by_id(&self, id: &str, new_inner_html: &str) -> bool {
        // Get the full HTML, find the element, replace its content, re-render
        let full_html = self.get_html();

        // Find the element with this id in the serialized HTML
        // Look for id="..." in an opening tag, then replace inner content
        let search_patterns = [
            format!("id=\"{}\"", id),
            format!("id='{}'", id),
        ];

        let mut found_pos = None;
        for pat in &search_patterns {
            if let Some(pos) = full_html.find(pat.as_str()) {
                found_pos = Some(pos);
                break;
            }
        }
        let Some(attr_pos) = found_pos else { return false; };

        // Find the opening tag's '>'
        let after_attr = &full_html[attr_pos..];
        let Some(gt_offset) = after_attr.find('>') else { return false; };
        let content_start = attr_pos + gt_offset + 1;

        // Find the tag name by scanning backwards to '<'
        let before_attr = &full_html[..attr_pos];
        let Some(lt_pos) = before_attr.rfind('<') else { return false; };
        let tag_fragment = &full_html[lt_pos + 1..attr_pos];
        let tag_name = tag_fragment.split_whitespace().next().unwrap_or("div");

        // Find the matching closing tag
        let closing_tag = format!("</{}>", tag_name);
        let rest = &full_html[content_start..];

        // Handle nested same-name tags by counting depth
        let opening_prefix = format!("<{}", tag_name);
        let mut depth = 1;
        let mut search_pos = 0;
        let mut close_pos = None;

        while depth > 0 && search_pos < rest.len() {
            let next_open = rest[search_pos..].find(opening_prefix.as_str())
                .map(|p| p + search_pos);
            let next_close = rest[search_pos..].find(closing_tag.as_str())
                .map(|p| p + search_pos);

            match (next_open, next_close) {
                (Some(o), Some(c)) if o < c => {
                    // Check it's actually an opening tag (followed by space or >)
                    let after = rest.as_bytes().get(o + opening_prefix.len());
                    if after == Some(&b' ') || after == Some(&b'>') || after == Some(&b'/') {
                        depth += 1;
                    }
                    search_pos = o + 1;
                }
                (_, Some(c)) => {
                    depth -= 1;
                    if depth == 0 {
                        close_pos = Some(c);
                    } else {
                        search_pos = c + closing_tag.len();
                    }
                }
                _ => break,
            }
        }

        let Some(inner_end_offset) = close_pos else { return false; };
        let content_end = content_start + inner_end_offset;

        // Build new HTML with replaced inner content
        let mut new_html = String::with_capacity(full_html.len());
        new_html.push_str(&full_html[..content_start]);
        new_html.push_str(new_inner_html);
        new_html.push_str(&full_html[content_end..]);

        // Re-render
        self.load_html_internal(&new_html);

        // Re-apply readonly if active
        if self.readonly_ids.borrow().is_some() {
            self.apply_readonly();
        }

        // Record undo snapshot
        let snapshot = serializer::serialize_buffer(
            &self.view.buffer(), &self.css_rules_store.borrow(),
        );
        self.undo_mgr.borrow_mut().push(snapshot);

        true
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
        let current = self.current_indent_level(&buffer, &start);
        let new_level = (current + 1).min(10);
        // Remove old indent tag if any
        if current > 0 {
            if let Some(old) = buffer.tag_table().lookup(&format!("indent_{}", current)) {
                buffer.remove_tag(&old, &start, &end);
            }
        }
        if let Some(tag) = buffer.tag_table().lookup(&format!("indent_{}", new_level)) {
            buffer.apply_tag(&tag, &start, &end);
            self.capture_undo_snapshot();
        }
    }

    pub fn decrease_indent(&self) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);
        let current = self.current_indent_level(&buffer, &start);
        if current > 0 {
            if let Some(old) = buffer.tag_table().lookup(&format!("indent_{}", current)) {
                buffer.remove_tag(&old, &start, &end);
            }
            if current > 1 {
                if let Some(tag) = buffer.tag_table().lookup(&format!("indent_{}", current - 1)) {
                    buffer.apply_tag(&tag, &start, &end);
                }
            }
            self.capture_undo_snapshot();
        }
    }

    fn current_indent_level(&self, _buffer: &gtk::TextBuffer, iter: &gtk::TextIter) -> i32 {
        for tag in &iter.tags() {
            if let Some(name) = tag.name() {
                if let Some(level) = name.strip_prefix("indent_") {
                    if let Ok(n) = level.parse::<i32>() {
                        return n;
                    }
                }
            }
        }
        0
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

    pub fn apply_background_color(&self, color: &str) {
        let buffer = self.view.buffer();
        if let Some((start, end)) = buffer.selection_bounds() {
            let tag_name = format!("bgcolor:{}", color);
            let tag = if let Some(t) = buffer.tag_table().lookup(&tag_name) {
                t
            } else {
                let new_tag = gtk::TextTag::builder()
                    .name(&tag_name)
                    .background(color)
                    .build();
                buffer.tag_table().add(&new_tag);
                new_tag
            };
            buffer.apply_tag(&tag, &start, &end);
            self.capture_undo_snapshot();
        }
    }

    pub fn apply_font_family(&self, family: &str) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);
        // Remove any existing font: tags in this range
        let tags = start.tags();
        for tag in tags.iter() {
            if let Some(name) = tag.name() {
                if name.starts_with("font:") {
                    buffer.remove_tag(tag, &start, &end);
                }
            }
        }
        if !family.is_empty() {
            let tag_name = format!("font:{}", family);
            let tag = if let Some(t) = buffer.tag_table().lookup(&tag_name) {
                t
            } else {
                let new_tag = gtk::TextTag::builder().name(&tag_name).family(family).build();
                buffer.tag_table().add(&new_tag);
                new_tag
            };
            buffer.apply_tag(&tag, &start, &end);
        }
        self.capture_undo_snapshot();
    }

    pub fn apply_font_size(&self, size_pt: f64) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);
        // Remove any existing size: tags in this range
        let tags = start.tags();
        for tag in tags.iter() {
            if let Some(name) = tag.name() {
                if name.starts_with("size:") {
                    buffer.remove_tag(tag, &start, &end);
                }
            }
        }
        if size_pt > 0.0 {
            let tag_name = format!("size:{}", size_pt);
            let tag = if let Some(t) = buffer.tag_table().lookup(&tag_name) {
                t
            } else {
                let new_tag = gtk::TextTag::builder()
                    .name(&tag_name)
                    .size_points(size_pt)
                    .build();
                buffer.tag_table().add(&new_tag);
                new_tag
            };
            buffer.apply_tag(&tag, &start, &end);
        }
        self.capture_undo_snapshot();
    }

    /// Returns the font family at the current cursor position, or None for default.
    pub fn current_font_family(&self) -> Option<String> {
        let buffer = self.view.buffer();
        let iter = buffer.iter_at_offset(buffer.cursor_position());
        for tag in &iter.tags() {
            if let Some(name) = tag.name() {
                if let Some(family) = name.strip_prefix("font:") {
                    return Some(family.to_string());
                }
                // Also check css_ tags that may carry font-family
                if name.starts_with("css_") {
                    if tag.family().is_some() {
                        return tag.family().map(|f| f.to_string());
                    }
                }
            }
        }
        None
    }

    /// Returns the font size (in pt) at the current cursor position, or None for default.
    pub fn current_font_size(&self) -> Option<f64> {
        let buffer = self.view.buffer();
        let iter = buffer.iter_at_offset(buffer.cursor_position());
        for tag in &iter.tags() {
            if let Some(name) = tag.name() {
                if let Some(size_str) = name.strip_prefix("size:") {
                    if let Ok(sz) = size_str.parse::<f64>() {
                        return Some(sz);
                    }
                }
                // Also check css_ tags that may carry font-size
                if name.starts_with("css_") {
                    let pts = tag.size_points();
                    if pts > 0.0 {
                        return Some(pts);
                    }
                }
            }
        }
        None
    }

    /// Returns the foreground color at the current cursor position, or None for default.
    pub fn current_color(&self) -> Option<gtk::gdk::RGBA> {
        let buffer = self.view.buffer();
        let iter = buffer.iter_at_offset(buffer.cursor_position());
        for tag in &iter.tags() {
            if let Some(name) = tag.name() {
                if name.starts_with("color: ") {
                    let color_str = name.strip_prefix("color: ").unwrap_or("");
                    if let Ok(rgba) = gtk::gdk::RGBA::parse(color_str) {
                        return Some(rgba);
                    }
                }
                // Also check css_ tags with foreground set
                if name.starts_with("css_") {
                    if let Some(rgba) = tag.foreground_rgba() {
                        return Some(rgba);
                    }
                }
            }
        }
        None
    }

    /// Returns the background color at the current cursor position, or None for default.
    pub fn current_background_color(&self) -> Option<gtk::gdk::RGBA> {
        let buffer = self.view.buffer();
        let iter = buffer.iter_at_offset(buffer.cursor_position());
        for tag in &iter.tags() {
            if let Some(name) = tag.name() {
                if let Some(color_str) = name.strip_prefix("bgcolor:") {
                    if let Ok(rgba) = gtk::gdk::RGBA::parse(color_str) {
                        return Some(rgba);
                    }
                }
                if name.starts_with("css_") {
                    if let Some(rgba) = tag.background_rgba() {
                        return Some(rgba);
                    }
                }
            }
        }
        None
    }

    /// Connect a callback that fires whenever the cursor moves (for toolbar updates).
    pub fn connect_cursor_changed<F: Fn() + 'static>(&self, f: F) {
        let buffer = self.view.buffer();
        buffer.connect_cursor_position_notify(move |_| {
            f();
        });
    }

    // ── Readonly Mode ──────────────────────────────────────────────────────

    /// Make the entire document readonly.
    pub fn set_readonly(&self, readonly: bool) {
        if readonly {
            *self.readonly_ids.borrow_mut() = Some(Vec::new());
        } else {
            *self.readonly_ids.borrow_mut() = None;
        }
        self.apply_readonly();
    }

    /// Make the document readonly except for elements with the given IDs.
    /// Pass an empty slice to make everything readonly.
    pub fn set_readonly_except(&self, editable_ids: &[&str]) {
        *self.readonly_ids.borrow_mut() = Some(editable_ids.iter().map(|s| s.to_string()).collect());
        self.apply_readonly();
    }

    /// Make the document fully editable (exit readonly mode).
    pub fn set_editable(&self) {
        *self.readonly_ids.borrow_mut() = None;
        self.apply_readonly();
    }

    fn apply_readonly(&self) {
        let readonly_ids = self.readonly_ids.borrow();
        let buffer = self.view.buffer();

        match readonly_ids.as_ref() {
            None => {
                // Fully editable
                self.view.set_editable(true);
                // Remove any readonly/editable tags
                if let Some(tag) = buffer.tag_table().lookup("_readonly") {
                    buffer.remove_tag(&tag, &buffer.start_iter(), &buffer.end_iter());
                }
                if let Some(tag) = buffer.tag_table().lookup("_editable") {
                    buffer.remove_tag(&tag, &buffer.start_iter(), &buffer.end_iter());
                }
                // Make all child widgets editable
                self.set_child_widgets_editable(true, &[]);
            }
            Some(ids) => {
                // Readonly mode
                self.view.set_editable(false);

                // Ensure tags exist
                if buffer.tag_table().lookup("_readonly").is_none() {
                    let tag = gtk::TextTag::builder()
                        .name("_readonly")
                        .editable(false)
                        .build();
                    buffer.tag_table().add(&tag);
                }
                if buffer.tag_table().lookup("_editable").is_none() {
                    let tag = gtk::TextTag::builder()
                        .name("_editable")
                        .editable(true)
                        .build();
                    buffer.tag_table().add(&tag);
                }

                // Apply readonly to entire buffer
                let readonly_tag = buffer.tag_table().lookup("_readonly").unwrap();
                buffer.apply_tag(&readonly_tag, &buffer.start_iter(), &buffer.end_iter());

                if !ids.is_empty() {
                    // Find regions with matching editable_id: tags and make them editable
                    let editable_tag = buffer.tag_table().lookup("_editable").unwrap();
                    for eid in ids.iter() {
                        let tag_name = format!("editable_id:{}", eid);
                        if let Some(id_tag) = buffer.tag_table().lookup(&tag_name) {
                            // Walk tag toggles to find all regions with this tag
                            let mut iter = buffer.start_iter();
                            loop {
                                if !iter.starts_tag(Some(&id_tag)) {
                                    if !iter.forward_to_tag_toggle(Some(&id_tag)) { break; }
                                    if !iter.starts_tag(Some(&id_tag)) { continue; }
                                }
                                let region_start = iter;
                                if !iter.forward_to_tag_toggle(Some(&id_tag)) { break; }
                                buffer.apply_tag(&editable_tag, &region_start, &iter);
                            }
                        }
                    }
                }

                // Set child widget editability
                self.set_child_widgets_editable(false, ids);
            }
        }
    }

    fn set_child_widgets_editable(&self, default_editable: bool, editable_ids: &[String]) {
        let buffer = self.view.buffer();
        let mut iter = buffer.start_iter();
        loop {
            if iter.child_anchor().is_some() {
                // Get widgets at this anchor
                let anchor = iter.child_anchor().unwrap();
                for widget in anchor.widgets() {
                    self.set_widget_tree_editable(&widget, default_editable, editable_ids);
                }
            }
            if !iter.forward_char() { break; }
        }
    }

    fn set_widget_tree_editable(&self, widget: &gtk::Widget, default_editable: bool, editable_ids: &[String]) {
        let name = widget.widget_name().to_string();

        // Check if this widget or any ancestor has an editable ID
        let is_editable = if !editable_ids.is_empty() {
            // Extract ID from widget name patterns like "cssgrid:div|id=\"myid\" ..."
            editable_ids.iter().any(|id| name.contains(&format!("id=\"{}\"", id)))
        } else {
            default_editable
        };

        // If it's a TextView, set editable
        if let Some(tv) = widget.downcast_ref::<gtk::TextView>() {
            tv.set_editable(if is_editable { true } else { default_editable });
        }

        // Recurse into container children
        if let Some(container) = widget.downcast_ref::<gtk::Box>() {
            let mut child = container.first_child();
            while let Some(c) = child {
                self.set_widget_tree_editable(&c, if is_editable { true } else { default_editable }, editable_ids);
                child = c.next_sibling();
            }
        } else if let Some(grid) = widget.downcast_ref::<gtk::Grid>() {
            let mut child = grid.first_child();
            while let Some(c) = child {
                self.set_widget_tree_editable(&c, if is_editable { true } else { default_editable }, editable_ids);
                child = c.next_sibling();
            }
        } else if let Some(fb) = widget.downcast_ref::<gtk::FlowBox>() {
            let mut child = fb.first_child();
            while let Some(c) = child {
                // FlowBox wraps children in FlowBoxChild
                if let Some(inner) = c.first_child() {
                    self.set_widget_tree_editable(&inner, if is_editable { true } else { default_editable }, editable_ids);
                }
                child = c.next_sibling();
            }
        }
    }

    // ── Class Manipulation (like classList API) ─────────────────────────────

    /// Check if an element with the given ID has a specific CSS class.
    pub fn has_class(&self, id: &str, class: &str) -> bool {
        let meta = self.element_meta.borrow();
        meta.get(id).map_or(false, |m| m.classes.iter().any(|c| c == class))
    }

    /// Add a CSS class to an element by ID. Returns true if the class was added
    /// (false if it was already present or element not found).
    pub fn add_class(&self, id: &str, class: &str) -> bool {
        {
            let mut meta = self.element_meta.borrow_mut();
            let Some(m) = meta.get_mut(id) else { return false; };
            if m.classes.iter().any(|c| c == class) { return false; }
            m.classes.push(class.to_string());
        }
        self.update_class_store(id);
        self.restyle_element(id);
        true
    }

    /// Remove a CSS class from an element by ID. Returns true if the class was removed.
    pub fn remove_class(&self, id: &str, class: &str) -> bool {
        {
            let mut meta = self.element_meta.borrow_mut();
            let Some(m) = meta.get_mut(id) else { return false; };
            let before = m.classes.len();
            m.classes.retain(|c| c != class);
            if m.classes.len() == before { return false; }
        }
        self.update_class_store(id);
        self.restyle_element(id);
        true
    }

    /// Toggle a CSS class on an element by ID. Returns true if the class is now
    /// present, false if it was removed, None if element not found.
    pub fn toggle_class(&self, id: &str, class: &str) -> Option<bool> {
        let added;
        {
            let mut meta = self.element_meta.borrow_mut();
            let m = meta.get_mut(id)?;
            if let Some(pos) = m.classes.iter().position(|c| c == class) {
                m.classes.remove(pos);
                added = false;
            } else {
                m.classes.push(class.to_string());
                added = true;
            }
        }
        self.update_class_store(id);
        self.restyle_element(id);
        Some(added)
    }

    /// Update the css_rules_store entry for class serialization.
    fn update_class_store(&self, id: &str) {
        let meta = self.element_meta.borrow();
        let mut store = self.css_rules_store.borrow_mut();
        let key = format!("classattr:{}", id);
        if let Some(m) = meta.get(id) {
            if m.classes.is_empty() {
                store.remove(&key);
            } else {
                store.insert(key, m.classes.join(" "));
            }
        }
    }

    /// Re-resolve CSS cascade for an element and apply new styles.
    fn restyle_element(&self, id: &str) {
        let meta = self.element_meta.borrow();
        let Some(m) = meta.get(id) else { return; };

        let classes_str = if m.classes.is_empty() { None } else { Some(m.classes.join(" ")) };
        let style_rules = self.style_rules.borrow();
        let hover_rules = self.hover_style_rules.borrow();

        // Recompute CSS cascade with updated classes
        let new_css = css::apply_css_cascade(
            &m.tag_name,
            classes_str.as_deref(),
            Some(id),
            m.inline_style.as_deref(),
            &style_rules,
        );

        // Compute hover CSS
        let hover_css = if !hover_rules.is_empty() {
            let delta = css::apply_css_cascade(
                &m.tag_name,
                classes_str.as_deref(),
                Some(id),
                None,
                &hover_rules,
            );
            if parser::has_meaningful_css(&delta) { Some(delta) } else { None }
        } else {
            None
        };

        // Try widget path first (CssProvider update — instant, no re-render)
        let providers = self.element_providers.borrow();
        if let Some(provider) = providers.get(id) {
            let css_str = parser::build_widget_css_string(&new_css, hover_css.as_ref());
            provider.load_from_data(&css_str);
            return;
        }
        drop(providers);

        // Fallback: inline text path — swap css_ tags on the editable_id region
        let buffer = self.view.buffer();
        let tag_name = format!("editable_id:{}", id);
        let Some(id_tag) = buffer.tag_table().lookup(&tag_name) else { return; };

        let mut iter = buffer.start_iter();
        loop {
            if !iter.starts_tag(Some(&id_tag)) {
                if !iter.forward_to_tag_toggle(Some(&id_tag)) { break; }
                if !iter.starts_tag(Some(&id_tag)) { continue; }
            }
            let region_start = iter;
            if !iter.forward_to_tag_toggle(Some(&id_tag)) { break; }
            let region_end = iter;

            // Remove old css_ tags from this region
            for tag in &region_start.tags() {
                if let Some(name) = tag.name() {
                    if name.starts_with("css_") {
                        buffer.remove_tag(tag, &region_start, &region_end);
                    }
                }
            }

            // Apply new css_ tag
            if parser::has_meaningful_css(&new_css) {
                let is_block = parser::is_block_element(&m.tag_name);
                let css_tag = parser::create_or_get_css_tag(
                    &buffer, &new_css, is_block, &mut self.css_rules_store.borrow_mut(),
                );
                buffer.apply_tag(&css_tag, &region_start, &region_end);
            }
            break; // Only first occurrence
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

    // Indent levels (margin-left: 40px per level)
    for level in 1..=10 {
        let tag = gtk::TextTag::builder()
            .name(format!("indent_{}", level))
            .left_margin(level * 40)
            .build();
        buffer.tag_table().add(&tag);
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
