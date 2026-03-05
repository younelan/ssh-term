use gtk4 as gtk;
use gtk::prelude::*;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{NodeData, RcDom};
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

// ── OHtmlEdit ───────────────────────────────────────────────────────

pub struct OHtmlEdit {
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
    /// Resolver for cid: image references in HTML emails.
    cid_resolver: RefCell<Option<std::rc::Rc<dyn Fn(&str) -> Option<Vec<u8>>>>>,
    /// Weak self-reference for thread-local callbacks (e.g. table cell right-click).
    self_weak: RefCell<Option<std::rc::Weak<OHtmlEdit>>>,
    /// Captured `<body>` attributes for round-trip serialization.
    body_attrs: RefCell<parser::BodyAttrs>,
}

impl OHtmlEdit {
    pub fn new() -> Self {
        let view = gtk::TextView::new();
        view.set_wrap_mode(gtk::WrapMode::Word);
        view.set_input_hints(gtk::InputHints::SPELLCHECK | gtk::InputHints::WORD_COMPLETION);
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
            cid_resolver: RefCell::new(None),
            self_weak: RefCell::new(None),
            body_attrs: RefCell::new(parser::BodyAttrs::default()),
        }
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.view.upcast_ref()
    }

    /// Set a callback to resolve `cid:` image references in HTML emails.
    /// The callback receives the content-id (without `cid:` prefix) and should
    /// return image bytes if available.
    pub fn set_cid_resolver<F>(&self, resolver: F)
    where
        F: Fn(&str) -> Option<Vec<u8>> + 'static,
    {
        *self.cid_resolver.borrow_mut() = Some(std::rc::Rc::new(resolver));
    }

    /// Connect buffer signals to automatically capture undo snapshots
    /// after user edits, and set up clipboard handling for HTML copy/paste.
    /// Must be called with an Rc<Self> after construction.
    pub fn connect_undo_signals(self: &std::rc::Rc<Self>) {
        // Store weak self-reference for thread-local callbacks
        *self.self_weak.borrow_mut() = Some(std::rc::Rc::downgrade(self));

        // Resize handler: recompute percentage-based widths when view width changes.
        // Uses a timer (200ms) to detect allocation changes without causing
        // continuous frame-clock requests like add_tick_callback would.
        let last_w = std::rc::Rc::new(Cell::new(0i32));
        let ed_resize = std::rc::Rc::downgrade(self);
        let view_clone = self.view.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
            let Some(ed) = ed_resize.upgrade() else {
                return gtk::glib::ControlFlow::Break;
            };
            let w = view_clone.allocated_width();
            if w > 100 && w != last_w.get() {
                last_w.set(w);
                ed.recompute_percentage_widths();
            }
            gtk::glib::ControlFlow::Continue
        });

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
            let Some(ed) = ed_weak.upgrade() else {
                return gtk::glib::Propagation::Proceed;
            };
            let primary = modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                || modifiers.contains(gtk::gdk::ModifierType::META_MASK);

            // Handle Delete/Backspace on selected image (no modifier needed)
            if matches!(key, gtk::gdk::Key::Delete | gtk::gdk::Key::BackSpace) {
                if let Some(pic) = parser::selected_image() {
                    ed.delete_image_widget(&pic);
                    parser::clear_image_selection();
                    return gtk::glib::Propagation::Stop;
                }
            }

            if !primary {
                // Only clear image selection on printable/typing keys, not modifiers or nav
                if parser::selected_image().is_some() {
                    let dominated_by_modifier = matches!(key,
                        gtk::gdk::Key::Shift_L | gtk::gdk::Key::Shift_R |
                        gtk::gdk::Key::Control_L | gtk::gdk::Key::Control_R |
                        gtk::gdk::Key::Meta_L | gtk::gdk::Key::Meta_R |
                        gtk::gdk::Key::Alt_L | gtk::gdk::Key::Alt_R |
                        gtk::gdk::Key::Super_L | gtk::gdk::Key::Super_R |
                        gtk::gdk::Key::Caps_Lock | gtk::gdk::Key::Escape |
                        gtk::gdk::Key::Delete | gtk::gdk::Key::BackSpace |
                        gtk::gdk::Key::Left | gtk::gdk::Key::Right |
                        gtk::gdk::Key::Up | gtk::gdk::Key::Down |
                        gtk::gdk::Key::Home | gtk::gdk::Key::End |
                        gtk::gdk::Key::Page_Up | gtk::gdk::Key::Page_Down |
                        gtk::gdk::Key::Tab | gtk::gdk::Key::F1 | gtk::gdk::Key::F2 |
                        gtk::gdk::Key::F3 | gtk::gdk::Key::F4 | gtk::gdk::Key::F5 |
                        gtk::gdk::Key::F6 | gtk::gdk::Key::F7 | gtk::gdk::Key::F8 |
                        gtk::gdk::Key::F9 | gtk::gdk::Key::F10 | gtk::gdk::Key::F11 |
                        gtk::gdk::Key::F12
                    );
                    if !dominated_by_modifier {
                        parser::clear_image_selection();
                    }
                }
                return gtk::glib::Propagation::Proceed;
            }
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
                    // Copy selected image as HTML, or fall back to text selection
                    if let Some(pic) = parser::selected_image() {
                        ed.copy_image_as_html(&pic);
                        return gtk::glib::Propagation::Stop;
                    }
                    ed.copy_selection_as_html();
                    gtk::glib::Propagation::Stop
                }
                gtk::gdk::Key::x | gtk::gdk::Key::X => {
                    // Cut selected image
                    if let Some(pic) = parser::selected_image() {
                        ed.copy_image_as_html(&pic);
                        ed.delete_image_widget(&pic);
                        parser::clear_image_selection();
                        return gtk::glib::Propagation::Stop;
                    }
                    ed.cut_selection_as_html();
                    gtk::glib::Propagation::Stop
                }
                gtk::gdk::Key::v | gtk::gdk::Key::V => {
                    if ed.paste_html_from_internal() {
                        gtk::glib::Propagation::Stop
                    } else {
                        ed.paste_from_system_clipboard();
                        gtk::glib::Propagation::Stop
                    }
                }
                gtk::gdk::Key::b | gtk::gdk::Key::B => {
                    ed.toggle_bold();
                    gtk::glib::Propagation::Stop
                }
                gtk::gdk::Key::i | gtk::gdk::Key::I => {
                    ed.toggle_italic();
                    gtk::glib::Propagation::Stop
                }
                gtk::gdk::Key::u | gtk::gdk::Key::U => {
                    ed.toggle_underline();
                    gtk::glib::Propagation::Stop
                }
                gtk::gdk::Key::k | gtk::gdk::Key::K => {
                    ed.insert_link();
                    gtk::glib::Propagation::Stop
                }
                gtk::gdk::Key::backslash => {
                    ed.remove_formatting();
                    gtk::glib::Propagation::Stop
                }
                _ => gtk::glib::Propagation::Proceed,
            }
        });
        self.view.add_controller(key_ctrl);

        // Clickable links + image selection with resize popover
        let click_ctrl = gtk::GestureClick::new();
        click_ctrl.set_button(1); // Left click only
        let ed_weak = std::rc::Rc::downgrade(self);
        click_ctrl.connect_released(move |gesture, _n_press, x, y| {
            let Some(ed) = ed_weak.upgrade() else { return };
            let Some(widget) = gesture.widget() else { return };
            let Some(view) = widget.downcast_ref::<gtk::TextView>() else { return };
            let (bx, by) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
            if let Some(iter) = view.iter_at_location(bx, by) {
                for tag in iter.tags().iter() {
                    if let Some(name) = tag.name() {
                        if let Some(url) = name.strip_prefix("link:") {
                            let url = url.to_string();
                            ed.show_link_popover(x, y, &url);
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

        // Drag & drop image insert
        let drop_target = gtk::DropTarget::new(gtk::gio::File::static_type(), gtk::gdk::DragAction::COPY);
        let ed_weak = std::rc::Rc::downgrade(self);
        drop_target.connect_drop(move |_target, value, _x, _y| {
            let Some(ed) = ed_weak.upgrade() else { return false };
            if let Ok(file) = value.get::<gtk::gio::File>() {
                let path = file.path().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                let lower = path.to_lowercase();
                if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg")
                    || lower.ends_with(".gif") || lower.ends_with(".svg") || lower.ends_with(".webp")
                    || lower.ends_with(".bmp") || lower.ends_with(".tiff")
                {
                    ed.insert_image_from_file(&file);
                    return true;
                }
            }
            false
        });
        self.view.add_controller(drop_target);

        // Right-click context menu
        let right_click = gtk::GestureClick::new();
        right_click.set_button(3);
        let ed_weak = std::rc::Rc::downgrade(self);
        right_click.connect_released(move |_gesture, _n_press, x, y| {
            let Some(ed) = ed_weak.upgrade() else { return };
            ed.show_context_menu(x, y);
        });
        self.view.add_controller(right_click);
    }

    fn show_context_menu(self: &std::rc::Rc<Self>, x: f64, y: f64) {
        let buffer = self.view.buffer();
        let (bx, by) = self.view.window_to_buffer_coords(
            gtk::TextWindowType::Widget, x as i32, y as i32,
        );

        // Detect context at click position
        let mut on_link: Option<String> = None;
        let mut on_image: Option<gtk::Picture> = None;

        if let Some(iter) = self.view.iter_at_location(bx, by) {
            for tag in iter.tags().iter() {
                if let Some(name) = tag.name() {
                    if let Some(url) = name.as_str().strip_prefix("link:") {
                        on_link = Some(url.to_string());
                        break;
                    }
                }
            }
            if on_link.is_none() {
                if let Some(anchor) = iter.child_anchor() {
                    for w in anchor.widgets() {
                        if let Ok(pic) = w.downcast::<gtk::Picture>() {
                            on_image = Some(pic);
                            break;
                        }
                    }
                }
            }
        }

        let has_selection = buffer.selection_bounds().is_some();

        if let Some(url) = on_link {
            self.show_link_context_menu(x, y, &url);
        } else if let Some(pic) = on_image {
            self.show_image_context_menu(x, y, &pic);
        } else {
            self.show_text_context_menu(x, y, has_selection);
        }
    }

    fn show_text_context_menu(self: &std::rc::Rc<Self>, x: f64, y: f64, has_selection: bool) {
        let popover = gtk::Popover::new();
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
        vbox.set_margin_top(4);
        vbox.set_margin_bottom(4);
        vbox.set_margin_start(4);
        vbox.set_margin_end(4);

        let cut_btn = gtk::Button::with_label("Cut");
        cut_btn.set_has_frame(false);
        let copy_btn = gtk::Button::with_label("Copy");
        copy_btn.set_has_frame(false);
        let paste_btn = gtk::Button::with_label("Paste");
        paste_btn.set_has_frame(false);

        vbox.append(&cut_btn);
        vbox.append(&copy_btn);
        vbox.append(&paste_btn);

        if has_selection {
            vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            let bold_btn = gtk::Button::with_label("Bold");
            bold_btn.set_has_frame(false);
            let italic_btn = gtk::Button::with_label("Italic");
            italic_btn.set_has_frame(false);
            let underline_btn = gtk::Button::with_label("Underline");
            underline_btn.set_has_frame(false);
            vbox.append(&bold_btn);
            vbox.append(&italic_btn);
            vbox.append(&underline_btn);

            let pop = popover.clone();
            let ed = std::rc::Rc::downgrade(self);
            bold_btn.connect_clicked(move |_| {
                if let Some(e) = ed.upgrade() { e.toggle_bold(); }
                pop.popdown();
            });
            let pop = popover.clone();
            let ed = std::rc::Rc::downgrade(self);
            italic_btn.connect_clicked(move |_| {
                if let Some(e) = ed.upgrade() { e.toggle_italic(); }
                pop.popdown();
            });
            let pop = popover.clone();
            let ed = std::rc::Rc::downgrade(self);
            underline_btn.connect_clicked(move |_| {
                if let Some(e) = ed.upgrade() { e.toggle_underline(); }
                pop.popdown();
            });
        }

        vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let link_btn = gtk::Button::with_label("Insert Link...");
        link_btn.set_has_frame(false);
        let hr_btn = gtk::Button::with_label("Insert Horizontal Rule");
        hr_btn.set_has_frame(false);
        let clear_btn = gtk::Button::with_label("Remove Formatting");
        clear_btn.set_has_frame(false);
        vbox.append(&link_btn);
        vbox.append(&hr_btn);
        vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        vbox.append(&clear_btn);

        // Wire standard buttons
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        cut_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { e.cut_selection_as_html(); }
            pop.popdown();
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        copy_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { e.copy_selection_as_html(); }
            pop.popdown();
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        paste_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { let _ = e.paste_html_from_internal(); }
            pop.popdown();
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        link_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { e.insert_link(); }
            pop.popdown();
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        hr_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { e.insert_hr(); }
            pop.popdown();
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        clear_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { e.remove_formatting(); }
            pop.popdown();
        });

        popover.set_child(Some(&vbox));
        popover.set_parent(&self.view);
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.connect_closed(|p| p.unparent());
        popover.popup();
    }

    fn show_link_context_menu(self: &std::rc::Rc<Self>, x: f64, y: f64, url: &str) {
        let popover = gtk::Popover::new();
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
        vbox.set_margin_top(4);
        vbox.set_margin_bottom(4);
        vbox.set_margin_start(4);
        vbox.set_margin_end(4);

        let url_label = gtk::Label::new(Some(url));
        url_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        url_label.set_max_width_chars(40);
        url_label.set_selectable(true);
        vbox.append(&url_label);
        vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        let edit_btn = gtk::Button::with_label("Edit Link...");
        edit_btn.set_has_frame(false);
        let remove_btn = gtk::Button::with_label("Remove Link");
        remove_btn.set_has_frame(false);
        let open_btn = gtk::Button::with_label("Open Link");
        open_btn.set_has_frame(false);
        vbox.append(&edit_btn);
        vbox.append(&remove_btn);
        vbox.append(&open_btn);

        vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let cut_btn = gtk::Button::with_label("Cut");
        cut_btn.set_has_frame(false);
        let copy_btn = gtk::Button::with_label("Copy");
        copy_btn.set_has_frame(false);
        let paste_btn = gtk::Button::with_label("Paste");
        paste_btn.set_has_frame(false);
        vbox.append(&cut_btn);
        vbox.append(&copy_btn);
        vbox.append(&paste_btn);

        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        edit_btn.connect_clicked(move |_| {
            pop.popdown();
            if let Some(e) = ed.upgrade() { e.insert_link(); }
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        remove_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { e.remove_link_at_cursor(); }
            pop.popdown();
        });
        let pop = popover.clone();
        let url_owned = url.to_string();
        open_btn.connect_clicked(move |btn| {
            pop.popdown();
            let launcher = gtk::UriLauncher::new(&url_owned);
            let root = btn.root().and_then(|r| r.downcast::<gtk::Window>().ok());
            launcher.launch(root.as_ref(), gtk::gio::Cancellable::NONE, |_| {});
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        cut_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { e.cut_selection_as_html(); }
            pop.popdown();
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        copy_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { e.copy_selection_as_html(); }
            pop.popdown();
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        paste_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { let _ = e.paste_html_from_internal(); }
            pop.popdown();
        });

        popover.set_child(Some(&vbox));
        popover.set_parent(&self.view);
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.connect_closed(|p| p.unparent());
        popover.popup();
    }

    fn show_image_context_menu(self: &std::rc::Rc<Self>, x: f64, y: f64, pic: &gtk::Picture) {
        let popover = gtk::Popover::new();
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
        vbox.set_margin_top(4);
        vbox.set_margin_bottom(4);
        vbox.set_margin_start(4);
        vbox.set_margin_end(4);

        let resize_btn = gtk::Button::with_label("Image Properties...");
        resize_btn.set_has_frame(false);
        let delete_btn = gtk::Button::with_label("Delete Image");
        delete_btn.set_has_frame(false);
        vbox.append(&resize_btn);
        vbox.append(&delete_btn);

        vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let cut_btn = gtk::Button::with_label("Cut");
        cut_btn.set_has_frame(false);
        let copy_btn = gtk::Button::with_label("Copy");
        copy_btn.set_has_frame(false);
        let paste_btn = gtk::Button::with_label("Paste");
        paste_btn.set_has_frame(false);
        vbox.append(&cut_btn);
        vbox.append(&copy_btn);
        vbox.append(&paste_btn);

        let pop = popover.clone();
        let pic_ref = pic.clone();
        resize_btn.connect_clicked(move |_| {
            pop.popdown();
            parser::show_resize_dialog(&pic_ref);
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        let pic_ref = pic.clone();
        delete_btn.connect_clicked(move |_| {
            pop.popdown();
            if let Some(e) = ed.upgrade() {
                // Find and delete the child anchor character for this image
                let buffer = e.view.buffer();
                let mut iter = buffer.start_iter();
                loop {
                    if let Some(anchor) = iter.child_anchor() {
                        if anchor.widgets().iter().any(|w| w.eq(&pic_ref)) {
                            let mut end = iter;
                            end.forward_char();
                            buffer.delete(&mut iter, &mut end);
                            e.capture_undo_snapshot();
                            break;
                        }
                    }
                    if !iter.forward_char() { break; }
                }
            }
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        cut_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { e.cut_selection_as_html(); }
            pop.popdown();
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        copy_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { e.copy_selection_as_html(); }
            pop.popdown();
        });
        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        paste_btn.connect_clicked(move |_| {
            if let Some(e) = ed.upgrade() { let _ = e.paste_html_from_internal(); }
            pop.popdown();
        });

        popover.set_child(Some(&vbox));
        popover.set_parent(&self.view);
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.connect_closed(|p| p.unparent());
        popover.popup();
    }

    pub fn remove_link_at_cursor(&self) {
        let buffer = self.view.buffer();
        let cursor = buffer.iter_at_mark(&buffer.get_insert());
        for tag in cursor.tags().iter() {
            if let Some(name) = tag.name() {
                if name.as_str().starts_with("link:") {
                    let mut start = cursor;
                    let mut end = cursor;
                    start.backward_to_tag_toggle(Some(tag));
                    end.forward_to_tag_toggle(Some(tag));
                    buffer.remove_tag(tag, &start, &end);
                    if let Some(a_tag) = buffer.tag_table().lookup("a") {
                        buffer.remove_tag(&a_tag, &start, &end);
                    }
                    self.capture_undo_snapshot();
                    break;
                }
            }
        }
    }

    fn show_link_popover(self: &std::rc::Rc<Self>, x: f64, y: f64, url: &str) {
        let popover = gtk::Popover::new();
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
        vbox.set_margin_top(4);
        vbox.set_margin_bottom(4);
        vbox.set_margin_start(4);
        vbox.set_margin_end(4);

        let url_label = gtk::Label::new(Some(url));
        url_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        url_label.set_max_width_chars(40);
        url_label.set_selectable(true);
        url_label.set_margin_bottom(4);
        vbox.append(&url_label);

        let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        vbox.append(&sep);

        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let edit_btn = gtk::Button::with_label("Edit");
        edit_btn.set_has_frame(false);
        let visit_btn = gtk::Button::with_label("Visit");
        visit_btn.set_has_frame(false);
        hbox.append(&edit_btn);
        hbox.append(&visit_btn);
        vbox.append(&hbox);

        let pop = popover.clone();
        let ed = std::rc::Rc::downgrade(self);
        edit_btn.connect_clicked(move |_| {
            pop.popdown();
            if let Some(e) = ed.upgrade() { e.insert_link(); }
        });

        let pop = popover.clone();
        let url_owned = url.to_string();
        visit_btn.connect_clicked(move |btn| {
            pop.popdown();
            let launcher = gtk::UriLauncher::new(&url_owned);
            let root = btn.root().and_then(|r| r.downcast::<gtk::Window>().ok());
            launcher.launch(root.as_ref(), gtk::gio::Cancellable::NONE, |_| {});
        });

        popover.set_child(Some(&vbox));
        popover.set_parent(&self.view);
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.connect_closed(|p| p.unparent());
        popover.popup();
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

        // Set thread-local so table cell right-click handlers can reach the editor
        parser::set_table_editor(self.self_weak.borrow().clone());

        let resolver = self.cid_resolver.borrow().clone();
        let result = parser::parse_html_to_buffer(&self.view, &dom, &buffer, resolver);
        *self.css_rules_store.borrow_mut() = result.css_rules_store;
        *self.hover_variants.borrow_mut() = result.hover_variants;
        *self.link_hover_tag.borrow_mut() = result.link_hover_tag;
        *self.style_rules.borrow_mut() = result.style_rules;
        *self.hover_style_rules.borrow_mut() = result.hover_rules;
        *self.element_meta.borrow_mut() = result.element_meta;
        *self.element_providers.borrow_mut() = result.element_providers;
        *self.body_attrs.borrow_mut() = result.body_attrs;
    }

    pub fn set_html(&self, html: &str) {
        self.load_html_internal(html);

        // Snapshot initial state for undo
        let initial_html = serializer::serialize_buffer(
            &self.view.buffer(),
            &self.css_rules_store.borrow(),
        );
        self.undo_mgr.borrow_mut().reset(initial_html);

        // Schedule deferred percentage recompute after the view gets allocated
        if self.view.allocated_width() <= 100 {
            if let Some(ref weak) = *self.self_weak.borrow() {
                let weak = weak.clone();
                glib::idle_add_local_once(move || {
                    if let Some(editor) = weak.upgrade() {
                        editor.recompute_percentage_widths();
                    }
                });
            }
        }
    }

    /// Recompute percentage-based widths for all child-anchor widgets
    /// (tables, HRs, images, flex containers) using the actual allocated view width.
    /// Called after initial layout and on resize.
    pub fn recompute_percentage_widths(&self) {
        let view = &self.view;
        let w = view.allocated_width();
        if w <= 100 { return; } // View not allocated yet
        let content_w = w - view.left_margin() - view.right_margin();
        let content_h = {
            let h = view.allocated_height();
            if h > 100 { h - view.top_margin() - view.bottom_margin() } else { return; }
        };

        let buffer = view.buffer();
        let mut iter = buffer.start_iter();
        loop {
            if let Some(anchor) = iter.child_anchor() {
                for widget in anchor.widgets() {
                    let name = widget.widget_name().to_string();

                    // Table (Grid) — widget_name has raw attrs: width="60%" border="1"
                    if widget.is::<gtk::Grid>() {
                        if let Some(w_str) = Self::extract_html_attr(&name, "width") {
                            if w_str.contains('%') {
                                if let Some(mut px) = parser::resolve_dimension(&w_str, content_w) {
                                    // Clamp to max-width if present in style
                                    if let Some(mw) = Self::extract_css_prop(&name, "max-width") {
                                        if let Some(max_px) = parser::resolve_dimension(&mw, content_w) {
                                            px = px.min(max_px);
                                            widget.set_hexpand(false);
                                        }
                                    }
                                    widget.set_size_request(px, -1);
                                }
                            }
                        }
                        // Also recompute cell widths
                        let table_w = Self::extract_html_attr(&name, "width")
                            .and_then(|tw| parser::resolve_dimension(&tw, content_w))
                            .unwrap_or(content_w);
                        let mut child = widget.first_child();
                        while let Some(ref c) = child {
                            let cname = c.widget_name().to_string();
                            if let Some(cw) = Self::extract_html_attr(&cname, "width") {
                                if cw.contains('%') {
                                    if let Some(px) = parser::resolve_dimension(&cw, table_w) {
                                        c.set_size_request(px, -1);
                                    }
                                }
                            }
                            child = c.next_sibling();
                        }
                    }

                    // HR (Separator) — widget_name: hr_rule:width=80%;size=4
                    else if name.starts_with("hr_rule") {
                        if let Some(w_str) = Self::extract_hr_param(&name, "width") {
                            if w_str.contains('%') {
                                if let Some(px) = parser::resolve_dimension(&w_str, content_w) {
                                    widget.set_size_request(px, -1);
                                }
                            }
                        } else {
                            // Default HR: fill entire view
                            widget.set_size_request(w, -1);
                        }
                    }

                    // Image (Picture) — widget_name: img:url|alt:text|pctw:50%|pcth:auto
                    else if name.starts_with("img:") {
                        let pctw = Self::extract_pipe_param(&name, "pctw");
                        let pcth = Self::extract_pipe_param(&name, "pcth");
                        if pctw.is_some() || pcth.is_some() {
                            let cur_w = widget.width_request();
                            let cur_h = widget.height_request();
                            let new_w = pctw.as_ref()
                                .and_then(|v| parser::resolve_dimension(v, content_w));
                            let new_h = pcth.as_ref()
                                .and_then(|v| parser::resolve_dimension(v, content_h));
                            // Maintain aspect ratio if only one dimension is percentage
                            let (fw, fh) = match (new_w, new_h) {
                                (Some(nw), Some(nh)) => (nw, nh),
                                (Some(nw), None) => {
                                    let nh = if cur_w > 0 && cur_h > 0 {
                                        (cur_h as f64 * nw as f64 / cur_w as f64) as i32
                                    } else { cur_h };
                                    (nw, nh)
                                }
                                (None, Some(nh)) => {
                                    let nw = if cur_w > 0 && cur_h > 0 {
                                        (cur_w as f64 * nh as f64 / cur_h as f64) as i32
                                    } else { cur_w };
                                    (nw, nh)
                                }
                                (None, None) => continue,
                            };
                            widget.set_size_request(fw, fh);
                        }
                    }

                    // Flex container (Box or FlowBox) — widget_name: flex:div|style="..."
                    else if name.starts_with("flex:") {
                        // Update container size_request to match actual view width
                        let cur_req = widget.width_request();
                        if cur_req > 0 && cur_req != content_w {
                            widget.set_size_request(content_w, -1);
                        }
                        // Update flex children with percentage widths/heights
                        let mut child = widget.first_child();
                        while let Some(ref c) = child {
                            let cname = c.widget_name().to_string();
                            if cname.starts_with("flexchild:") {
                                let css_w = Self::extract_css_prop(&cname, "width");
                                let css_h = Self::extract_css_prop(&cname, "height");
                                let new_w = css_w.as_ref()
                                    .filter(|v| v.contains('%'))
                                    .and_then(|v| parser::resolve_dimension(v, content_w));
                                let new_h = css_h.as_ref()
                                    .filter(|v| v.contains('%'))
                                    .and_then(|v| parser::resolve_dimension(v, content_h));
                                match (new_w, new_h) {
                                    (Some(nw), Some(nh)) => c.set_size_request(nw, nh),
                                    (Some(nw), None) => c.set_size_request(nw, c.height_request()),
                                    (None, Some(nh)) => c.set_size_request(c.width_request(), nh),
                                    _ => {}
                                }
                            }
                            child = c.next_sibling();
                        }
                    }
                }
            }
            if !iter.forward_char() {
                break;
            }
        }
    }

    /// Extract an HTML attribute value from a raw attrs string like: width="60%" border="1"
    fn extract_html_attr(attrs_str: &str, attr_name: &str) -> Option<String> {
        let pattern = format!("{}=\"", attr_name);
        if let Some(start) = attrs_str.find(&pattern) {
            let value_start = start + pattern.len();
            if let Some(end) = attrs_str[value_start..].find('"') {
                return Some(attrs_str[value_start..value_start + end].to_string());
            }
        }
        None
    }

    /// Extract a parameter from HR widget_name like: hr_rule:width=80%;size=4;color=#2c3e50
    fn extract_hr_param(name: &str, param: &str) -> Option<String> {
        if let Some(params) = name.strip_prefix("hr_rule:") {
            let prefix = format!("{}=", param);
            for part in params.split(';') {
                if let Some(val) = part.strip_prefix(&prefix) {
                    return Some(val.to_string());
                }
            }
        }
        None
    }

    /// Extract a pipe-delimited parameter from widget_name like: img:url|alt:text|pctw:50%
    fn extract_pipe_param(name: &str, param: &str) -> Option<String> {
        let prefix = format!("{}:", param);
        for part in name.split('|') {
            if let Some(val) = part.strip_prefix(&prefix) {
                return Some(val.to_string());
            }
        }
        None
    }

    /// Extract a CSS property value from the style attribute in a widget_name.
    /// e.g. extract_css_prop("flexchild:div|style=\"width: 30%; height: 50%\"", "width") → Some("30%")
    fn extract_css_prop(name: &str, prop: &str) -> Option<String> {
        if let Some(style_start) = name.find("style=\"") {
            let style_content = &name[style_start + 7..];
            if let Some(style_end) = style_content.find('"') {
                let style = &style_content[..style_end];
                let prefix = format!("{}:", prop);
                for decl in style.split(';') {
                    let decl = decl.trim();
                    if let Some(val) = decl.strip_prefix(&prefix) {
                        return Some(val.trim().to_string());
                    }
                }
            }
        }
        None
    }

    pub fn get_html(&self) -> String {
        let inner = serializer::serialize_buffer(&self.view.buffer(), &self.css_rules_store.borrow());
        let ba = self.body_attrs.borrow();
        if ba.bgcolor.is_none() && ba.text_color.is_none() && ba.class.is_none()
            && ba.style.is_none() && ba.link_color.is_none()
        {
            return inner;
        }
        let mut body_tag = String::from("<body");
        if let Some(ref bg) = ba.bgcolor {
            body_tag.push_str(&format!(" bgcolor=\"{}\"", bg));
        }
        if let Some(ref tc) = ba.text_color {
            body_tag.push_str(&format!(" text=\"{}\"", tc));
        }
        if let Some(ref lc) = ba.link_color {
            body_tag.push_str(&format!(" link=\"{}\"", lc));
        }
        if let Some(ref cls) = ba.class {
            body_tag.push_str(&format!(" class=\"{}\"", cls));
        }
        if let Some(ref st) = ba.style {
            body_tag.push_str(&format!(" style=\"{}\"", st));
        }
        body_tag.push('>');
        format!("{}\n{}</body>", body_tag, inner)
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

    /// Paste from the system clipboard with HTML sanitization.
    fn paste_from_system_clipboard(self: &std::rc::Rc<Self>) {
        let clipboard = self.view.clipboard();
        let ed = std::rc::Rc::downgrade(self);
        clipboard.read_text_async(gtk::gio::Cancellable::NONE, move |result| {
            let Some(ed) = ed.upgrade() else { return };
            if let Ok(Some(text)) = result {
                let text_str = text.to_string();
                if text_str.is_empty() { return; }

                let buffer = ed.view.buffer();

                // Delete selection if any
                if let Some((mut s, mut e)) = buffer.selection_bounds() {
                    buffer.delete(&mut s, &mut e);
                }

                // Check if it looks like HTML
                if text_str.contains('<') && (text_str.contains("</") || text_str.contains("/>")) {
                    let sanitized = sanitize_external_html(&text_str);
                    let cursor = buffer.iter_at_mark(&buffer.get_insert());
                    let store = ed.css_rules_store.borrow();
                    let before = serializer::serialize_range(
                        &buffer, &buffer.start_iter(), &cursor, &store,
                    );
                    let after = serializer::serialize_range(
                        &buffer, &cursor, &buffer.end_iter(), &store,
                    );
                    drop(store);
                    let combined = format!("{}{}{}", before, sanitized, after);
                    ed.load_html_internal(&combined);
                } else {
                    // Plain text — insert directly
                    let mut iter = buffer.iter_at_mark(&buffer.get_insert());
                    buffer.insert(&mut iter, &text_str);
                }
                ed.capture_undo_snapshot();
            }
        });
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

    // ── Remove Formatting ───────────────────────────────────────────────────

    pub fn remove_formatting(&self) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);

        // Collect tags to remove (formatting only, keep structural)
        let mut tags_to_remove: Vec<gtk::TextTag> = Vec::new();
        let mut iter = start;
        loop {
            for tag in iter.tags().iter() {
                if let Some(name) = tag.name() {
                    let n = name.as_str();
                    let should_remove = matches!(n,
                        "b" | "strong" | "i" | "em" | "cite" | "var" | "u" | "ins"
                        | "s" | "strike" | "del" | "code" | "tt" | "kbd" | "samp"
                        | "mark" | "sub" | "sup" | "small" | "big" | "pre" | "a"
                        | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "abbr_style"
                    )
                    || n.starts_with("link:")
                    || n.starts_with("css_")
                    || n.starts_with("abbr_title:");
                    if should_remove && !tags_to_remove.iter().any(|t| {
                        t.name().as_deref() == tag.name().as_deref()
                    }) {
                        tags_to_remove.push(tag.clone());
                    }
                }
            }
            if !iter.forward_char() || iter >= end { break; }
        }

        for tag in &tags_to_remove {
            buffer.remove_tag(tag, &start, &end);
        }
        self.capture_undo_snapshot();
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
        let hr = Self::create_hr_widget(&self.view, "hr_rule", 2, "#888888");
        self.view.add_child_at_anchor(&hr, &anchor);
        self.capture_undo_snapshot();
    }

    /// Create an HR separator widget sized to the view's width.
    fn create_hr_widget(view: &gtk::TextView, widget_name: &str, height: i32, color: &str) -> gtk::Separator {
        let hr_line = gtk::Separator::new(gtk::Orientation::Horizontal);
        hr_line.set_margin_top(4);
        hr_line.set_margin_bottom(4);
        hr_line.set_halign(gtk::Align::Fill);
        hr_line.set_widget_name(widget_name);

        // Child anchors don't honor hexpand — use a large width that the
        // TextView will clip to its own allocation.
        let w = view.allocated_width();
        hr_line.set_size_request(if w > 50 { w } else { 4096 }, -1);

        #[allow(deprecated)]
        {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(&format!(
                "separator {{ min-height: {}px; background-color: {}; }}",
                height, color
            ));
            hr_line.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }

        hr_line
    }

    // ── Link ───────────────────────────────────────────────────────────────

    /// Show a dialog to insert or edit a hyperlink.
    pub fn insert_link(self: &std::rc::Rc<Self>) {
        let buffer = self.view.buffer();

        // Detect existing link at cursor
        let cursor_iter = buffer.iter_at_mark(&buffer.get_insert());
        let mut existing_url: Option<String> = None;
        let mut existing_link_tag: Option<gtk::TextTag> = None;
        for tag in cursor_iter.tags().iter() {
            if let Some(name) = tag.name() {
                if let Some(url) = name.as_str().strip_prefix("link:") {
                    existing_url = Some(url.to_string());
                    existing_link_tag = Some(tag.clone());
                    break;
                }
            }
        }

        // Get selected text for pre-fill
        let selected_text = buffer.selection_bounds().map(|(s, e)| {
            buffer.text(&s, &e, false).to_string()
        });

        // Get existing link text if editing
        let (link_start_offset, link_end_offset) = if let Some(ref lt) = existing_link_tag {
            let mut ls = cursor_iter;
            let mut le = cursor_iter;
            ls.backward_to_tag_toggle(Some(lt));
            le.forward_to_tag_toggle(Some(lt));
            (Some(ls.offset()), Some(le.offset()))
        } else {
            (None, None)
        };
        let existing_text = if let (Some(so), Some(eo)) = (link_start_offset, link_end_offset) {
            let s = buffer.iter_at_offset(so);
            let e = buffer.iter_at_offset(eo);
            Some(buffer.text(&s, &e, false).to_string())
        } else {
            None
        };

        // Build dialog
        let toplevel = self.view.root().and_then(|r| r.downcast::<gtk::Window>().ok());
        let dialog = gtk::Window::builder()
            .title("Insert Link")
            .modal(true)
            .resizable(false)
            .default_width(380)
            .build();
        if let Some(ref win) = toplevel {
            dialog.set_transient_for(Some(win));
        }

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_top(12);
        vbox.set_margin_bottom(12);
        vbox.set_margin_start(12);
        vbox.set_margin_end(12);

        let url_label = gtk::Label::new(Some("URL:"));
        url_label.set_halign(gtk::Align::Start);
        let url_entry = gtk::Entry::new();
        url_entry.set_placeholder_text(Some("https://"));
        if let Some(ref url) = existing_url {
            url_entry.set_text(url);
        }

        let text_label = gtk::Label::new(Some("Text:"));
        text_label.set_halign(gtk::Align::Start);
        let text_entry = gtk::Entry::new();
        if let Some(ref t) = selected_text {
            text_entry.set_text(t);
        } else if let Some(ref t) = existing_text {
            text_entry.set_text(t);
        }

        vbox.append(&url_label);
        vbox.append(&url_entry);
        vbox.append(&text_label);
        vbox.append(&text_entry);

        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        btn_box.set_halign(gtk::Align::End);
        btn_box.set_margin_top(8);
        let cancel_btn = gtk::Button::with_label("Cancel");
        let preview_btn = gtk::Button::with_label("Preview");
        preview_btn.set_tooltip_text(Some("Insert a rich preview card for this URL"));
        let insert_btn = gtk::Button::with_label(if existing_url.is_some() { "Update" } else { "Insert" });
        btn_box.append(&cancel_btn);
        btn_box.append(&preview_btn);
        btn_box.append(&insert_btn);
        vbox.append(&btn_box);
        dialog.set_child(Some(&vbox));

        let dlg = dialog.clone();
        cancel_btn.connect_clicked(move |_| dlg.close());

        // Preview button — insert a rich link preview card instead of a plain link
        let dlg_preview = dialog.clone();
        let url_entry_preview = url_entry.clone();
        let ed_preview = std::rc::Rc::downgrade(self);
        preview_btn.connect_clicked(move |_| {
            let url = url_entry_preview.text().to_string();
            if !url.is_empty() {
                if let Some(ed) = ed_preview.upgrade() {
                    ed.insert_link_preview(&url);
                }
            }
            dlg_preview.close();
        });

        let dlg = dialog.clone();
        let ed = std::rc::Rc::downgrade(self);
        let has_selection = selected_text.is_some();
        insert_btn.connect_clicked(move |_| {
            let url = url_entry.text().to_string();
            if url.is_empty() {
                dlg.close();
                return;
            }
            let display_text = text_entry.text().to_string();
            let Some(ed) = ed.upgrade() else { dlg.close(); return; };
            let buffer = ed.view.buffer();

            // Remove old link tags if editing
            if let (Some(so), Some(eo)) = (link_start_offset, link_end_offset) {
                let s = buffer.iter_at_offset(so);
                let e = buffer.iter_at_offset(eo);
                // Remove old link: and a tags
                if let Some(ref lt) = existing_link_tag {
                    buffer.remove_tag(lt, &s, &e);
                }
                if let Some(a_tag) = buffer.tag_table().lookup("a") {
                    buffer.remove_tag(&a_tag, &s, &e);
                }
                // If text changed, replace it
                if let Some(ref old_text) = existing_text {
                    if display_text != *old_text {
                        let mut ms = buffer.iter_at_offset(so);
                        let mut me = buffer.iter_at_offset(eo);
                        buffer.delete(&mut ms, &mut me);
                        let mut ins = buffer.iter_at_offset(so);
                        buffer.insert(&mut ins, &display_text);
                    }
                }
            } else if has_selection {
                // Selection exists — tags will be applied to it
            } else if !display_text.is_empty() {
                // No selection, no existing link — insert text at cursor
                let mut iter = buffer.iter_at_mark(&buffer.get_insert());
                buffer.insert(&mut iter, &display_text);
            }

            // Determine range to tag
            let (tag_start, tag_end) = if has_selection && existing_link_tag.is_none() {
                if let Some((s, e)) = buffer.selection_bounds() {
                    (s, e)
                } else {
                    dlg.close(); return;
                }
            } else if let Some(so) = link_start_offset {
                let s = buffer.iter_at_offset(so);
                let e = buffer.iter_at_offset(so + display_text.len() as i32);
                (s, e)
            } else {
                // Newly inserted text
                let cursor = buffer.iter_at_mark(&buffer.get_insert());
                let s = buffer.iter_at_offset(cursor.offset() - display_text.len() as i32);
                (s, cursor)
            };

            // Apply link tag
            let link_tag_name = format!("link:{}", url);
            let link_tag = if let Some(existing) = buffer.tag_table().lookup(&link_tag_name) {
                existing
            } else {
                let new_tag = gtk::TextTag::new(Some(&link_tag_name));
                buffer.tag_table().add(&new_tag);
                new_tag
            };
            buffer.apply_tag(&link_tag, &tag_start, &tag_end);
            if let Some(a_tag) = buffer.tag_table().lookup("a") {
                buffer.apply_tag(&a_tag, &tag_start, &tag_end);
            }

            ed.capture_undo_snapshot();
            dlg.close();
        });

        dialog.present();
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

        let picture = gtk::Picture::for_file(file);
        let max_w = 600;
        if let Ok(texture) = gtk::gdk::Texture::from_file(file) {
            let natural_w = texture.width();
            let natural_h = texture.height();
            let (display_w, display_h) = if natural_w > max_w {
                let scale = max_w as f64 / natural_w as f64;
                (max_w, (natural_h as f64 * scale) as i32)
            } else {
                (natural_w, natural_h)
            };
            picture.set_size_request(display_w, display_h);
        } else {
            picture.set_size_request(400, 300);
        }

        picture.set_widget_name(&format!("img:{}", path_str));
        parser::setup_image_click_resize(&picture);

        let anchor = buffer.create_child_anchor(&mut iter);
        self.view.add_child_at_anchor(&picture, &anchor);
        self.capture_undo_snapshot();
    }

    /// Resize the image at the current cursor position (if any).
    /// Returns true if an image was found and resized.
    pub fn resize_image_at_cursor(&self, width: i32, height: i32) -> bool {
        let buffer = self.view.buffer();
        let iter = buffer.iter_at_mark(&buffer.get_insert());
        if let Some(anchor) = iter.child_anchor() {
            for widget in anchor.widgets() {
                if let Some(pic) = widget.downcast_ref::<gtk::Picture>() {
                    let name = pic.widget_name().to_string();
                    if name.starts_with("img:") || name.starts_with("svg:") {
                        pic.set_size_request(width.max(16), height.max(16));
                        self.capture_undo_snapshot();
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Delete a Picture widget (and its child anchor) from the buffer.
    fn delete_image_widget(&self, pic: &gtk::Picture) {
        // Snapshot current state (with the image) so undo can restore it
        self.capture_undo_snapshot();
        let buffer = self.view.buffer();
        let mut iter = buffer.start_iter();
        loop {
            if let Some(anchor) = iter.child_anchor() {
                for w in anchor.widgets() {
                    if w.eq(pic.upcast_ref::<gtk::Widget>()) {
                        let mut end = iter;
                        end.forward_char();
                        buffer.delete(&mut iter, &mut end);
                        // Snapshot state after deletion
                        self.capture_undo_snapshot();
                        return;
                    }
                }
            }
            if !iter.forward_char() { break; }
        }
    }

    /// Copy a Picture's HTML representation to the internal clipboard.
    fn copy_image_as_html(&self, pic: &gtk::Picture) {
        let name = pic.widget_name().to_string();
        let html = if name.starts_with("svg:") {
            self.css_rules_store.borrow().get(&name).cloned().unwrap_or_default()
        } else if let Some(rest) = name.strip_prefix("img:") {
            let (src, alt) = if let Some(pipe_pos) = rest.find("|alt:") {
                (&rest[..pipe_pos], Some(&rest[pipe_pos + 5..]))
            } else {
                (rest, None)
            };
            let mut tag = format!("<img src=\"{}\"", src);
            if let Some(a) = alt { tag.push_str(&format!(" alt=\"{}\"", a)); }
            let w = pic.width_request();
            let h = pic.height_request();
            if w > 0 { tag.push_str(&format!(" width=\"{}\"", w)); }
            if h > 0 { tag.push_str(&format!(" height=\"{}\"", h)); }
            tag.push('>');
            tag
        } else {
            return;
        };
        *self.internal_clipboard.borrow_mut() = Some(html.clone());
        self.clipboard_set_by_us.set(true);
        self.view.clipboard().set_text(&name);
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

                    // Re-fetch iterators after buffer mutation
                    let ls = buffer.iter_at_line(line_num).unwrap_or_else(|| buffer.start_iter());
                    let mut le = ls;
                    if !le.ends_line() { le.forward_to_line_end(); }
                    buffer.remove_tag(&li_tag, &ls, &le);
                    buffer.remove_tag(&ul_tag, &ls, &le);
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

    // ── Numbered List ──────────────────────────────────────────────────────

    pub fn insert_numbered_list(&self) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);

        let start_line = start.line();
        let end_line = end.line();

        let tag_table = buffer.tag_table();
        let list_marker_tag = tag_table.lookup("list_marker").unwrap();
        let li_tag = tag_table.lookup("li").unwrap();
        let ol_tag = tag_table.lookup("ol_1").unwrap();

        // Check if first line already has a numbered marker (ol_1 tag)
        let first_line_start = buffer.iter_at_line(start_line).unwrap_or_else(|| buffer.start_iter());
        let has_ol = first_line_start.has_tag(&ol_tag) || {
            let mut check = first_line_start;
            check.forward_chars(2);
            check.has_tag(&ol_tag)
        };

        // Also check for bullet (ul_1) to handle switching
        let ul_tag = tag_table.lookup("ul_1").unwrap();
        let has_ul = first_line_start.has_tag(&ul_tag) || {
            let mut check = first_line_start;
            check.forward_chars(2);
            check.has_tag(&ul_tag)
        };

        let toggling_off = has_ol;

        for line_num in (start_line..=end_line).rev() {
            let line_start = buffer.iter_at_line(line_num).unwrap_or_else(|| buffer.start_iter());
            let mut line_end = line_start;
            if !line_end.ends_line() { line_end.forward_to_line_end(); }

            if toggling_off {
                // Remove numbered marker: scan for digits followed by ". "
                let line_text = buffer.text(&line_start, &line_end, false).to_string();
                let marker_len = find_number_marker_len(&line_text);
                if marker_len > 0 {
                    let mut ms = buffer.iter_at_line(line_num).unwrap_or_else(|| buffer.start_iter());
                    let mut me = buffer.iter_at_offset(ms.offset() + marker_len as i32);
                    buffer.delete(&mut ms, &mut me);

                    let ls = buffer.iter_at_line(line_num).unwrap_or_else(|| buffer.start_iter());
                    let mut le = ls;
                    if !le.ends_line() { le.forward_to_line_end(); }
                    buffer.remove_tag(&li_tag, &ls, &le);
                    buffer.remove_tag(&ol_tag, &ls, &le);
                }
            } else {
                // If switching from bullet, remove bullet marker + ul tag first
                if has_ul {
                    let line_text = buffer.text(&line_start, &line_end, false).to_string();
                    if line_text.starts_with("\u{2022} ") || line_text.starts_with("\u{25AA} ")
                        || line_text.starts_with("\u{25CB} ") {
                        let mut ms = buffer.iter_at_line(line_num).unwrap_or_else(|| buffer.start_iter());
                        let mut me = buffer.iter_at_offset(ms.offset() + 2);
                        buffer.delete(&mut ms, &mut me);
                    }
                    let ls = buffer.iter_at_line(line_num).unwrap_or_else(|| buffer.start_iter());
                    let mut le = ls;
                    if !le.ends_line() { le.forward_to_line_end(); }
                    buffer.remove_tag(&ul_tag, &ls, &le);
                }

                // Insert numbered marker
                let num = (line_num - start_line + 1) as i32;
                let marker = format!("{}. ", num);
                let marker_char_count = marker.len() as i32;
                let mut ins = buffer.iter_at_line(line_num).unwrap_or_else(|| buffer.start_iter());
                let offset = ins.offset();
                buffer.insert(&mut ins, &marker);

                let marker_start = buffer.iter_at_offset(offset);
                let marker_end_iter = buffer.iter_at_offset(offset + marker_char_count - 1);
                buffer.apply_tag(&list_marker_tag, &marker_start, &marker_end_iter);

                let ls = buffer.iter_at_offset(offset);
                let mut le = ls;
                if !le.ends_line() { le.forward_to_line_end(); }
                buffer.apply_tag(&li_tag, &ls, &le);
                buffer.apply_tag(&ol_tag, &ls, &le);
            }
        }
        self.capture_undo_snapshot();
    }

    // ── Blockquote / Quote ─────────────────────────────────────────────────

    pub fn insert_quote(&self) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);

        let current_depth = current_blockquote_depth(&buffer, &start);
        let new_depth = std::cmp::min(current_depth + 1, 5);

        let bq_tag_name = format!("blockquote_{}", new_depth);
        if let Some(tag) = buffer.tag_table().lookup(&bq_tag_name) {
            buffer.apply_tag(&tag, &start, &end);
        }
        self.capture_undo_snapshot();
    }

    pub fn remove_quote(&self) {
        let buffer = self.view.buffer();
        let (start, end) = get_target_bounds(&buffer);

        let depth = current_blockquote_depth(&buffer, &start);
        if depth > 0 {
            let tag_name = format!("blockquote_{}", depth);
            if let Some(tag) = buffer.tag_table().lookup(&tag_name) {
                buffer.remove_tag(&tag, &start, &end);
            }
            self.capture_undo_snapshot();
        }
    }

    // ── Signature ──────────────────────────────────────────────────────────

    pub fn insert_signature(&self, signature_text: &str) {
        let buffer = self.view.buffer();
        let store = self.css_rules_store.borrow();
        let before = serializer::serialize_buffer(&buffer, &store);
        drop(store);

        let escaped = signature_text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('\n', "<br>");
        let sig_html = format!(
            "<div style=\"color: #888888; font-size: 10pt;\">-- <br>{}</div>",
            escaped
        );

        let combined = format!("{}{}", before, sig_html);
        self.load_html_internal(&combined);
        self.capture_undo_snapshot();
    }

    // ── Table ──────────────────────────────────────────────────────────────

    pub fn insert_table(&self, rows: u32, cols: u32) {
        let buffer = self.view.buffer();

        if let Some((mut s, mut e)) = buffer.selection_bounds() {
            buffer.delete(&mut s, &mut e);
        }

        let cursor = buffer.iter_at_mark(&buffer.get_insert());
        let buf_start = buffer.start_iter();
        let buf_end = buffer.end_iter();
        let store = self.css_rules_store.borrow();
        let before = serializer::serialize_range(&buffer, &buf_start, &cursor, &store);
        let after = serializer::serialize_range(&buffer, &cursor, &buf_end, &store);
        drop(store);

        let mut table_html = String::from(
            "<table border=\"1\" style=\"border-collapse: collapse;\" cellpadding=\"4\">\n"
        );
        for _row in 0..rows {
            table_html.push_str("  <tr>");
            for _col in 0..cols {
                table_html.push_str("<td>&nbsp;</td>");
            }
            table_html.push_str("</tr>\n");
        }
        table_html.push_str("</table>\n");

        let combined = format!("{}{}{}", before, table_html, after);
        self.load_html_internal(&combined);
        self.capture_undo_snapshot();
    }

    pub fn show_table_dialog(self: &std::rc::Rc<Self>) {
        let toplevel = self.view.root().and_then(|r| r.downcast::<gtk::Window>().ok());
        let dialog = gtk::Window::builder()
            .title("Insert Table")
            .modal(true)
            .resizable(false)
            .default_width(250)
            .build();
        if let Some(ref win) = toplevel {
            dialog.set_transient_for(Some(win));
        }

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_top(12);
        vbox.set_margin_bottom(12);
        vbox.set_margin_start(12);
        vbox.set_margin_end(12);

        let grid = gtk::Grid::new();
        grid.set_row_spacing(6);
        grid.set_column_spacing(8);

        let rows_label = gtk::Label::new(Some("Rows:"));
        rows_label.set_halign(gtk::Align::End);
        let rows_spin = gtk::SpinButton::with_range(1.0, 20.0, 1.0);
        rows_spin.set_value(3.0);

        let cols_label = gtk::Label::new(Some("Columns:"));
        cols_label.set_halign(gtk::Align::End);
        let cols_spin = gtk::SpinButton::with_range(1.0, 10.0, 1.0);
        cols_spin.set_value(3.0);

        grid.attach(&rows_label, 0, 0, 1, 1);
        grid.attach(&rows_spin, 1, 0, 1, 1);
        grid.attach(&cols_label, 0, 1, 1, 1);
        grid.attach(&cols_spin, 1, 1, 1, 1);

        vbox.append(&grid);

        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        btn_box.set_halign(gtk::Align::End);
        btn_box.set_margin_top(8);
        let cancel_btn = gtk::Button::with_label("Cancel");
        let insert_btn = gtk::Button::with_label("Insert");
        btn_box.append(&cancel_btn);
        btn_box.append(&insert_btn);
        vbox.append(&btn_box);

        dialog.set_child(Some(&vbox));

        let dlg = dialog.clone();
        cancel_btn.connect_clicked(move |_| dlg.close());

        let dlg = dialog.clone();
        let ed = std::rc::Rc::downgrade(self);
        insert_btn.connect_clicked(move |_| {
            let rows = rows_spin.value() as u32;
            let cols = cols_spin.value() as u32;
            if let Some(ed) = ed.upgrade() {
                ed.insert_table(rows, cols);
            }
            dlg.close();
        });

        dialog.present();
    }

    // ── Emoji Picker ──────────────────────────────────────────────────────

    pub fn show_emoji_picker(self: &std::rc::Rc<Self>) {
        let chooser = gtk::EmojiChooser::new();
        let ed = std::rc::Rc::downgrade(self);
        chooser.connect_emoji_picked(move |_chooser, emoji| {
            if let Some(ed) = ed.upgrade() {
                let buffer = ed.view.buffer();
                let mut iter = buffer.iter_at_mark(&buffer.get_insert());
                buffer.insert(&mut iter, emoji);
                ed.capture_undo_snapshot();
            }
        });

        let buffer = self.view.buffer();
        let cursor = buffer.iter_at_mark(&buffer.get_insert());
        let rect = self.view.iter_location(&cursor);
        let (wx, wy) = self.view.buffer_to_window_coords(
            gtk::TextWindowType::Widget, rect.x(), rect.y(),
        );
        chooser.set_parent(&self.view);
        chooser.set_pointing_to(Some(&gtk::gdk::Rectangle::new(wx, wy, 1, rect.height())));
        chooser.connect_closed(|c| c.unparent());
        chooser.popup();
    }

    // ── Table Context Menu ────────────────────────────────────────────────

    pub fn show_table_context_menu(
        self: &std::rc::Rc<Self>,
        grid: &gtk::Grid,
        cell_view: &gtk::TextView,
        x: f64,
        y: f64,
    ) {
        let popover = gtk::Popover::new();
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
        vbox.set_margin_top(4);
        vbox.set_margin_bottom(4);
        vbox.set_margin_start(4);
        vbox.set_margin_end(4);

        let items: &[(&str, TableOp)] = &[
            ("Insert Row Above", TableOp::AddRowAbove),
            ("Insert Row Below", TableOp::AddRowBelow),
            ("Insert Column Left", TableOp::AddColLeft),
            ("Insert Column Right", TableOp::AddColRight),
        ];
        for &(label, ref op) in items {
            let btn = gtk::Button::with_label(label);
            btn.set_has_frame(false);
            let pop = popover.clone();
            let ed = std::rc::Rc::downgrade(self);
            let g = grid.clone();
            let cv = cell_view.clone();
            let op = op.clone();
            btn.connect_clicked(move |_| {
                pop.popdown();
                if let Some(ed) = ed.upgrade() { ed.table_edit_op(&g, &cv, &op); }
            });
            vbox.append(&btn);
        }

        vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        let del_items: &[(&str, TableOp)] = &[
            ("Delete Row", TableOp::DeleteRow),
            ("Delete Column", TableOp::DeleteCol),
        ];
        for &(label, ref op) in del_items {
            let btn = gtk::Button::with_label(label);
            btn.set_has_frame(false);
            let pop = popover.clone();
            let ed = std::rc::Rc::downgrade(self);
            let g = grid.clone();
            let cv = cell_view.clone();
            let op = op.clone();
            btn.connect_clicked(move |_| {
                pop.popdown();
                if let Some(ed) = ed.upgrade() { ed.table_edit_op(&g, &cv, &op); }
            });
            vbox.append(&btn);
        }

        popover.set_child(Some(&vbox));
        popover.set_parent(cell_view);
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.connect_closed(|p| p.unparent());
        popover.popup();
    }

    fn table_edit_op(&self, grid: &gtk::Grid, cell_view: &gtk::TextView, op: &TableOp) {
        // Get cell position via GridLayoutChild
        let Some(layout) = grid.layout_manager() else { return };
        let lc = layout.layout_child(cell_view.upcast_ref::<gtk::Widget>());
        let Some(gc) = lc.downcast_ref::<gtk::GridLayoutChild>() else { return };
        let target_row = gc.row();
        let target_col = gc.column();

        // Serialize the table to HTML
        let store = self.css_rules_store.borrow();
        let mut table_html = String::new();
        serializer::serialize_grid_as_table(grid, &mut table_html, &store);
        drop(store);

        // Apply the operation to get modified table HTML
        let modified = match op {
            TableOp::AddRowAbove => table_html_add_row(&table_html, target_row, true),
            TableOp::AddRowBelow => table_html_add_row(&table_html, target_row, false),
            TableOp::AddColLeft => table_html_add_col(&table_html, target_col, true),
            TableOp::AddColRight => table_html_add_col(&table_html, target_col, false),
            TableOp::DeleteRow => table_html_delete_row(&table_html, target_row),
            TableOp::DeleteCol => table_html_delete_col(&table_html, target_col),
        };

        // Replace old table HTML in full document
        let full_html = self.get_html();
        let new_html = full_html.replacen(&table_html, &modified, 1);
        self.load_html_internal(&new_html);
        self.capture_undo_snapshot();
    }

    // ── Link Preview ──────────────────────────────────────────────────────

    pub fn insert_link_preview(self: &std::rc::Rc<Self>, url: &str) {
        let buffer = self.view.buffer();
        let mut iter = buffer.iter_at_mark(&buffer.get_insert());
        if iter.offset() > 0 && !iter.starts_line() {
            buffer.insert(&mut iter, "\n");
        }
        let anchor = buffer.create_child_anchor(&mut iter);

        let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
        card.set_margin_top(4);
        card.set_margin_bottom(4);
        card.set_margin_start(8);
        card.set_margin_end(8);
        card.set_size_request(400, -1);
        card.set_widget_name(&format!("link_preview:{}", url));

        #[allow(deprecated)]
        {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(
                "box { background-color: alpha(@theme_fg_color, 0.05); \
                 border: 1px solid alpha(@theme_fg_color, 0.15); \
                 border-radius: 8px; padding: 12px; }",
            );
            card.style_context()
                .add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }

        let loading_label = gtk::Label::new(Some("Loading preview..."));
        loading_label.set_halign(gtk::Align::Start);
        card.append(&loading_label);

        self.view.add_child_at_anchor(&card, &anchor);

        let url_owned = url.to_string();
        let card_ref = card;
        let ed = std::rc::Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let url_clone = url_owned.clone();
            let result = gtk::gio::spawn_blocking(move || fetch_og_metadata(&url_clone)).await;

            let metadata = result.ok().flatten();
            // Remove loading label
            while let Some(child) = card_ref.first_child() {
                card_ref.remove(&child);
            }

            if let Some(meta) = metadata {
                if let Some(title) = &meta.title {
                    let title_label = gtk::Label::new(Some(title));
                    title_label.set_halign(gtk::Align::Start);
                    title_label.set_wrap(true);
                    title_label.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(title)));
                    card_ref.append(&title_label);
                }
                if let Some(desc) = &meta.description {
                    let desc_label = gtk::Label::new(Some(desc));
                    desc_label.set_halign(gtk::Align::Start);
                    desc_label.set_wrap(true);
                    desc_label.set_max_width_chars(60);
                    desc_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    desc_label.set_lines(2);
                    card_ref.append(&desc_label);
                }
                let url_label = gtk::Label::new(Some(&url_owned));
                url_label.set_halign(gtk::Align::Start);
                url_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                url_label.add_css_class("dim-label");
                card_ref.append(&url_label);
            } else {
                let fallback = gtk::Label::new(Some(&url_owned));
                fallback.set_halign(gtk::Align::Start);
                card_ref.append(&fallback);
            }

            if let Some(ed) = ed.upgrade() {
                ed.capture_undo_snapshot();
            }
        });
    }

    pub fn show_link_preview_dialog(self: &std::rc::Rc<Self>) {
        let toplevel = self.view.root().and_then(|r| r.downcast::<gtk::Window>().ok());
        let dialog = gtk::Window::builder()
            .title("Insert Link Preview")
            .modal(true)
            .resizable(false)
            .default_width(350)
            .build();
        if let Some(ref win) = toplevel {
            dialog.set_transient_for(Some(win));
        }

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_top(12);
        vbox.set_margin_bottom(12);
        vbox.set_margin_start(12);
        vbox.set_margin_end(12);

        let url_label = gtk::Label::new(Some("URL:"));
        url_label.set_halign(gtk::Align::Start);
        let url_entry = gtk::Entry::new();
        url_entry.set_placeholder_text(Some("https://"));
        vbox.append(&url_label);
        vbox.append(&url_entry);

        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        btn_box.set_halign(gtk::Align::End);
        btn_box.set_margin_top(8);
        let cancel_btn = gtk::Button::with_label("Cancel");
        let insert_btn = gtk::Button::with_label("Insert");
        btn_box.append(&cancel_btn);
        btn_box.append(&insert_btn);
        vbox.append(&btn_box);
        dialog.set_child(Some(&vbox));

        let dlg = dialog.clone();
        cancel_btn.connect_clicked(move |_| dlg.close());

        let dlg = dialog.clone();
        let ed = std::rc::Rc::downgrade(self);
        insert_btn.connect_clicked(move |_| {
            let url = url_entry.text().to_string();
            if !url.is_empty() {
                if let Some(ed) = ed.upgrade() {
                    ed.insert_link_preview(&url);
                }
            }
            dlg.close();
        });

        dialog.present();
    }
}

#[derive(Clone)]
enum TableOp {
    AddRowAbove,
    AddRowBelow,
    AddColLeft,
    AddColRight,
    DeleteRow,
    DeleteCol,
}

impl Default for OHtmlEdit {
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

/// Sanitize HTML pasted from external sources (MS Word, web pages).
// ── Table HTML Manipulation ──────────────────────────────────────────────

fn table_html_add_row(table_html: &str, target_row: i32, above: bool) -> String {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut table_html.as_bytes())
        .unwrap();
    let table = find_first_element(&dom.document, "table");
    let Some(table) = table else { return table_html.to_string() };

    let rows = collect_child_elements(&table, "tr");
    let idx = target_row as usize;
    if idx >= rows.len() { return table_html.to_string(); }

    // Count cells in target row to know how many to insert
    let num_cols = collect_child_elements_multi(&rows[idx], &["td", "th"]).len();

    // Build new row
    let new_tr = create_empty_row(num_cols);

    // Insert into DOM
    let insert_pos = if above { idx } else { idx + 1 };
    let mut children = table.children.borrow_mut();
    // Find the insert_pos-th <tr> among children
    let mut tr_count = 0;
    let mut insert_idx = children.len();
    for (i, child) in children.iter().enumerate() {
        if is_element(child, "tr") {
            if tr_count == insert_pos {
                insert_idx = i;
                break;
            }
            tr_count += 1;
        }
    }
    children.insert(insert_idx, new_tr);
    drop(children);

    serialize_dom(&dom)
}

fn table_html_add_col(table_html: &str, target_col: i32, left: bool) -> String {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut table_html.as_bytes())
        .unwrap();
    let table = find_first_element(&dom.document, "table");
    let Some(table) = table else { return table_html.to_string() };

    let rows = collect_child_elements(&table, "tr");
    let insert_col = if left { target_col } else { target_col + 1 } as usize;

    for row in &rows {
        let cells = collect_child_elements_multi(row, &["td", "th"]);
        let new_td = create_element("td", "&nbsp;");
        let pos = insert_col.min(cells.len());

        let mut children = row.children.borrow_mut();
        let mut cell_count = 0;
        let mut insert_idx = children.len();
        for (i, child) in children.iter().enumerate() {
            if is_element_multi(child, &["td", "th"]) {
                if cell_count == pos {
                    insert_idx = i;
                    break;
                }
                cell_count += 1;
            }
        }
        children.insert(insert_idx, new_td);
    }

    serialize_dom(&dom)
}

fn table_html_delete_row(table_html: &str, target_row: i32) -> String {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut table_html.as_bytes())
        .unwrap();
    let table = find_first_element(&dom.document, "table");
    let Some(table) = table else { return table_html.to_string() };

    let rows = collect_child_elements(&table, "tr");
    if rows.len() <= 1 { return table_html.to_string(); } // Don't delete last row

    let mut children = table.children.borrow_mut();
    let mut tr_count = 0;
    let mut remove_idx = None;
    for (i, child) in children.iter().enumerate() {
        if is_element(child, "tr") {
            if tr_count == target_row as usize {
                remove_idx = Some(i);
                break;
            }
            tr_count += 1;
        }
    }
    if let Some(idx) = remove_idx {
        children.remove(idx);
    }
    drop(children);

    serialize_dom(&dom)
}

fn table_html_delete_col(table_html: &str, target_col: i32) -> String {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut table_html.as_bytes())
        .unwrap();
    let table = find_first_element(&dom.document, "table");
    let Some(table) = table else { return table_html.to_string() };

    let rows = collect_child_elements(&table, "tr");
    // Check that at least one row has > 1 column
    let has_multi_col = rows.iter().any(|r| collect_child_elements_multi(r, &["td", "th"]).len() > 1);
    if !has_multi_col { return table_html.to_string(); }

    for row in &rows {
        let mut children = row.children.borrow_mut();
        let mut cell_count = 0;
        let mut remove_idx = None;
        for (i, child) in children.iter().enumerate() {
            if is_element_multi(child, &["td", "th"]) {
                if cell_count == target_col as usize {
                    remove_idx = Some(i);
                    break;
                }
                cell_count += 1;
            }
        }
        if let Some(idx) = remove_idx {
            children.remove(idx);
        }
    }

    serialize_dom(&dom)
}

// DOM helper functions for table manipulation
fn find_first_element(node: &markup5ever_rcdom::Handle, tag: &str) -> Option<markup5ever_rcdom::Handle> {
    if let NodeData::Element { ref name, .. } = node.data {
        if name.local.as_ref().eq_ignore_ascii_case(tag) {
            return Some(node.clone());
        }
    }
    for child in node.children.borrow().iter() {
        if let Some(found) = find_first_element(child, tag) {
            return Some(found);
        }
    }
    None
}

fn collect_child_elements(node: &markup5ever_rcdom::Handle, tag: &str) -> Vec<markup5ever_rcdom::Handle> {
    let mut result = Vec::new();
    for child in node.children.borrow().iter() {
        if is_element(child, tag) {
            result.push(child.clone());
        }
        // Also check inside tbody/thead/tfoot
        if let NodeData::Element { ref name, .. } = child.data {
            let local = name.local.as_ref();
            if local.eq_ignore_ascii_case("tbody") || local.eq_ignore_ascii_case("thead")
                || local.eq_ignore_ascii_case("tfoot")
            {
                for grandchild in child.children.borrow().iter() {
                    if is_element(grandchild, tag) {
                        result.push(grandchild.clone());
                    }
                }
            }
        }
    }
    result
}

fn collect_child_elements_multi(node: &markup5ever_rcdom::Handle, tags: &[&str]) -> Vec<markup5ever_rcdom::Handle> {
    let mut result = Vec::new();
    for child in node.children.borrow().iter() {
        if is_element_multi(child, tags) {
            result.push(child.clone());
        }
    }
    result
}

fn is_element(node: &markup5ever_rcdom::Handle, tag: &str) -> bool {
    if let NodeData::Element { ref name, .. } = node.data {
        name.local.as_ref().eq_ignore_ascii_case(tag)
    } else {
        false
    }
}

fn is_element_multi(node: &markup5ever_rcdom::Handle, tags: &[&str]) -> bool {
    if let NodeData::Element { ref name, .. } = node.data {
        let local = name.local.as_ref();
        tags.iter().any(|t| local.eq_ignore_ascii_case(t))
    } else {
        false
    }
}

fn create_element(tag: &str, text_content: &str) -> markup5ever_rcdom::Handle {
    use markup5ever_rcdom::Node;
    use std::cell::RefCell as StdRefCell;

    let ns = html5ever::namespace_url!("http://www.w3.org/1999/xhtml");
    let local = html5ever::LocalName::from(tag);
    let name = html5ever::QualName::new(None, ns, local);
    let el = Node::new(NodeData::Element {
        name,
        attrs: StdRefCell::new(Vec::new()),
        template_contents: Default::default(),
        mathml_annotation_xml_integration_point: false,
    });
    let text = Node::new(NodeData::Text {
        contents: StdRefCell::new(text_content.into()),
    });
    el.children.borrow_mut().push(text);
    el
}

fn create_empty_row(num_cols: usize) -> markup5ever_rcdom::Handle {
    use markup5ever_rcdom::Node;
    use std::cell::RefCell as StdRefCell;

    let ns = html5ever::namespace_url!("http://www.w3.org/1999/xhtml");
    let local = html5ever::LocalName::from("tr");
    let name = html5ever::QualName::new(None, ns, local);
    let tr = Node::new(NodeData::Element {
        name,
        attrs: StdRefCell::new(Vec::new()),
        template_contents: Default::default(),
        mathml_annotation_xml_integration_point: false,
    });
    for _ in 0..num_cols.max(1) {
        tr.children.borrow_mut().push(create_element("td", "\u{00a0}"));
    }
    tr
}

fn serialize_dom(dom: &markup5ever_rcdom::RcDom) -> String {
    use markup5ever_rcdom::SerializableHandle;
    let mut buf = Vec::new();
    let handle: SerializableHandle = dom.document.clone().into();
    html5ever::serialize(
        &mut buf,
        &handle,
        html5ever::serialize::SerializeOpts {
            scripting_enabled: false,
            traversal_scope: html5ever::serialize::TraversalScope::ChildrenOnly(None),
            create_missing_parent: false,
        },
    )
    .unwrap_or_default();
    String::from_utf8(buf).unwrap_or_default()
}

// ── OG Metadata Fetching ────────────────────────────────────────────────

struct OgMetadata {
    title: Option<String>,
    description: Option<String>,
    #[allow(dead_code)]
    image_url: Option<String>,
    #[allow(dead_code)]
    site_name: Option<String>,
}

fn fetch_og_metadata(url: &str) -> Option<OgMetadata> {
    let body: String = ureq::get(url)
        .header("User-Agent", "Mozilla/5.0 (compatible; LinkPreview/1.0)")
        .call()
        .ok()?
        .into_body()
        .read_to_string()
        .ok()?;

    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut body.as_bytes())
        .ok()?;

    let mut meta = OgMetadata {
        title: None,
        description: None,
        image_url: None,
        site_name: None,
    };
    extract_og_tags(&dom.document, &mut meta);

    // Fallback: extract <title> if no og:title
    if meta.title.is_none() {
        meta.title = extract_title_text(&dom.document);
    }

    if meta.title.is_some() || meta.description.is_some() {
        Some(meta)
    } else {
        None
    }
}

fn extract_og_tags(node: &markup5ever_rcdom::Handle, meta: &mut OgMetadata) {
    if let NodeData::Element { ref name, ref attrs, .. } = node.data {
        if name.local.as_ref().eq_ignore_ascii_case("meta") {
            let attrs = attrs.borrow();
            let property = attrs
                .iter()
                .find(|a| a.name.local.as_ref().eq_ignore_ascii_case("property"))
                .map(|a| a.value.to_string());
            let content = attrs
                .iter()
                .find(|a| a.name.local.as_ref().eq_ignore_ascii_case("content"))
                .map(|a| a.value.to_string());
            if let (Some(prop), Some(cont)) = (property, content) {
                match prop.as_str() {
                    "og:title" => meta.title = Some(cont),
                    "og:description" => meta.description = Some(cont),
                    "og:image" => meta.image_url = Some(cont),
                    "og:site_name" => meta.site_name = Some(cont),
                    _ => {}
                }
            }
        }
    }
    for child in node.children.borrow().iter() {
        extract_og_tags(child, meta);
    }
}

fn extract_title_text(node: &markup5ever_rcdom::Handle) -> Option<String> {
    if let NodeData::Element { ref name, .. } = node.data {
        if name.local.as_ref().eq_ignore_ascii_case("title") {
            let mut text = String::new();
            for child in node.children.borrow().iter() {
                if let NodeData::Text { ref contents } = child.data {
                    text.push_str(&contents.borrow());
                }
            }
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    for child in node.children.borrow().iter() {
        if let Some(t) = extract_title_text(child) {
            return Some(t);
        }
    }
    None
}

// ── Existing Helpers ────────────────────────────────────────────────────

fn sanitize_external_html(html: &str) -> String {
    use regex::Regex;
    let mut cleaned = html.to_string();

    // Remove <style> blocks
    let style_re = Regex::new(r"(?si)<style[^>]*>.*?</style>").unwrap();
    cleaned = style_re.replace_all(&cleaned, "").to_string();

    // Remove <script> blocks
    let script_re = Regex::new(r"(?si)<script[^>]*>.*?</script>").unwrap();
    cleaned = script_re.replace_all(&cleaned, "").to_string();

    // Remove class attributes
    let class_re = Regex::new(r#"\s+class="[^"]*""#).unwrap();
    cleaned = class_re.replace_all(&cleaned, "").to_string();

    // Strip mso-* CSS properties from inline styles
    let mso_re = Regex::new(r"mso-[a-z-]+:\s*[^;]+;?").unwrap();
    cleaned = mso_re.replace_all(&cleaned, "").to_string();

    // Remove empty style attributes
    let empty_style = Regex::new(r#"\s+style="\s*""#).unwrap();
    cleaned = empty_style.replace_all(&cleaned, "").to_string();

    // Remove HTML comments (Word conditional comments)
    let comment_re = Regex::new(r"(?s)<!--.*?-->").unwrap();
    cleaned = comment_re.replace_all(&cleaned, "").to_string();

    // Remove Office namespace tags (<o:p>, <v:*>, <w:*>)
    let office_re = Regex::new(r"(?si)</?[ovw]:[^>]*>").unwrap();
    cleaned = office_re.replace_all(&cleaned, "").to_string();

    cleaned
}

/// Find the length of a numbered list marker at the start of a line (e.g. "1. " → 3, "12. " → 4).
fn find_number_marker_len(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 { return 0; }
    if i + 2 <= bytes.len() && bytes[i] == b'.' && bytes[i + 1] == b' ' {
        i + 2
    } else {
        0
    }
}

/// Get the current blockquote nesting depth at an iterator position.
fn current_blockquote_depth(buffer: &gtk::TextBuffer, iter: &gtk::TextIter) -> i32 {
    let mut max_depth = 0;
    for tag in iter.tags().iter() {
        if let Some(name) = tag.name() {
            if let Some(depth_str) = name.as_str().strip_prefix("blockquote_") {
                if let Ok(d) = depth_str.parse::<i32>() {
                    max_depth = max_depth.max(d);
                }
            }
        }
    }
    // Also check the "blockquote" base tag as depth 1
    if max_depth == 0 {
        if let Some(tag) = buffer.tag_table().lookup("blockquote") {
            if iter.has_tag(&tag) {
                max_depth = 1;
            }
        }
    }
    max_depth
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
            let line = start.line();
            let mut ins = end;
            buffer.insert(&mut ins, "\u{200B}");
            // Re-fetch start from buffer after mutation
            start = buffer.iter_at_line(line).unwrap_or_else(|| buffer.start_iter());
            end = ins;
        }
        (start, end)
    }
}
