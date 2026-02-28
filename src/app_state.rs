use crate::config::ConnectionSettings;
use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{glib, ApplicationWindow, Notebook, TextView, CssProvider};
use std::sync::{Arc, Mutex};
use crate::terminal_state::TerminalState;

thread_local! {
    pub static CONN_WIN: std::cell::RefCell<Option<ApplicationWindow>> = std::cell::RefCell::new(None);
    pub static TARGET_NB: std::cell::RefCell<glib::object::WeakRef<Notebook>> = std::cell::RefCell::new(glib::object::WeakRef::new());
    pub static ACTIVE_TERMINALS: std::cell::RefCell<Vec<ActiveTerminal>> = std::cell::RefCell::new(Vec::new());
    pub static IS_PROGRAMMATIC: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

pub struct ActiveTerminal {
    pub session_id: String,
    pub text_view: glib::object::WeakRef<TextView>,
    pub css_provider: CssProvider,
    pub term_state: Arc<Mutex<TerminalState>>,
}

pub fn update_active_terminals(session_id: &str, settings: &ConnectionSettings) {
    ACTIVE_TERMINALS.with(|at| {
        let mut list = at.borrow_mut();
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
