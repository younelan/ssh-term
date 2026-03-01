pub mod config;
pub mod app_state;
pub mod ssh;
pub mod terminal_state;
pub mod terminal_tab;
pub mod session_manager_ui;

use crate::config::{ConnectionSettings, THEMES, AppConfig, load_app_config, save_app_config};
use crate::app_state::{CONN_WIN, TARGET_NB};
use crate::terminal_tab::add_terminal_tab;
use crate::session_manager_ui::{
    populate_list, rgba_to_hex, hex_to_rgba,
    fg_button_downgrade, bg_button_downgrade
};

use gtk4 as gtk;
use gtk::gio;
use gtk::prelude::*;
use gtk::{
    glib, Application, ApplicationWindow, Box as GtkBox, Button, Entry, ListBox, Orientation,
    HeaderBar, ColorButton, DropDown, StringList, Notebook, MenuButton, CheckButton, Grid,
    Label, ScrolledWindow, CssProvider, gdk
};
use std::sync::{Arc, Mutex};

fn main() {
    let app = Application::builder().application_id("com.github.example.terminal-ssh").build();
    app.connect_startup(|app| setup_app(app));
    app.connect_activate(move |app| {
        if app.active_window().is_none() {
            ensure_connect_window(app, None);
        }
    });

    let args: Vec<String> = std::env::args().collect();
    if args.is_empty() {
        app.run();
    } else {
        app.run_with_args(&args);
    }
}

fn setup_app(app: &Application) {
    let css = r#"
        .connection-box { background-color: @theme_bg_color; }
        .session-row { padding: 6px; border-radius: 4px; border-bottom: 1px solid alpha(currentColor, 0.1); }
        .session-row:hover { background-color: alpha(currentColor, 0.05); }
        notebook > header { background-color: alpha(currentColor, 0.03); border-bottom: 1px solid alpha(currentColor, 0.1); }
        notebook > header tab { padding: 4px 12px; border-radius: 6px 6px 0 0; min-width: 120px; border: none; background: transparent; }
        notebook > header tab:checked { background-color: @theme_bg_color; box-shadow: inset 0 3px alpha(currentColor, 0.2); }
        textview { padding: 4px; }
        popover { background-color: @theme_bg_color; border: 1px solid alpha(currentColor, 0.1); border-radius: 8px; box-shadow: 0 4px 12px alpha(black, 0.15); padding: 8px; }
        popover button.flat { border-radius: 6px; padding: 6px 12px; }
        popover button.flat:hover { background-color: alpha(currentColor, 0.05); }
    "#;
    let provider = CssProvider::new();
    provider.load_from_data(css);
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    
    let app_config = load_app_config();
    apply_app_theme(&app_config.theme);

    let menubar = gio::Menu::new();
    
    // Application/File Menu
    let file_menu = gio::Menu::new();
    file_menu.append(Some("New Connection"), Some("app.new_connection"));
    
    let sessions_item = gio::MenuItem::new(Some("Saved Sessions"), None);
    let sessions_submenu = gio::Menu::new();
    setup_sessions_actions(app, &sessions_submenu);
    sessions_item.set_submenu(Some(&sessions_submenu));
    file_menu.append_item(&sessions_item);
    
    file_menu.append(Some("Quit"), Some("app.quit"));
    menubar.append_submenu(Some("File"), &file_menu);

    // Settings Menu
    let settings_menu = gio::Menu::new();
    settings_menu.append(Some("Preferences..."), Some("app.settings"));
    menubar.append_submenu(Some("Settings"), &settings_menu);

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

    let action_settings = gio::SimpleAction::new("settings", None);
    let app_weak3 = app.downgrade();
    action_settings.connect_activate(move |_, _| {
        if let Some(app) = app_weak3.upgrade() {
            show_settings_window(&app);
        }
    });
    app.add_action(&action_settings);
    app.set_accels_for_action("app.settings", &["<Primary>comma"]);
}

fn apply_app_theme(theme_name: &str) {
    let display = gdk::Display::default().expect("Could not connect to a display.");
    let settings = gtk::Settings::for_display(&display);
    
    // Explicit UI colors for reliable switching
    let (bg, fg, hb_bg, is_dark) = match theme_name {
        "Light" => ("#ffffff", "#000000", "#f6f6f6", false),
        "Coffee" => ("#2c211b", "#f3e5ab", "#1e1511", true),
        "Dark Blue" => ("#0a192f", "#ffffff", "#020c1b", true),
        _ => ("#242424", "#ffffff", "#303030", true), // Default to Dark
    };

    settings.set_gtk_application_prefer_dark_theme(is_dark);

    let css = format!(
        r#"
        @define-color window_bg_color {0};
        @define-color window_fg_color {1};
        @define-color theme_bg_color {0};
        @define-color theme_fg_color {1};
        @define-color headerbar_bg_color {2};
        @define-color headerbar_fg_color {1};
        @define-color card_bg_color {0};
        @define-color popover_bg_color {0};

        /* Force theme on major UI containers */
        window, .background, .main-app-window, .connection-box, 
        box, grid, notebook, stack, scrolledwindow, viewport,
        list, row, entry, entry > text,
        popover, popover contents {{ 
            background-color: {0}; 
            color: {1}; 
            background-image: none;
            box-shadow: none;
        }}

        headerbar, headerbar > box, headerbar label, headerbar .title {{ 
            background-color: {2}; 
            color: {1}; 
            background-image: none; 
        }}

        /* Specific widget fixes */
        entry, dropdown > button {{
            background-color: {0};
            color: {1};
            border: 1px solid alpha({1}, 0.2);
            border-radius: 4px;
            background-image: none;
        }}
        
        dropdown, dropdown:hover {{
            background-color: transparent;
            border: none;
            box-shadow: none;
        }}
        
        dropdown > button > box, 
        dropdown > button > stack, 
        dropdown > button label,
        dropdown > button image {{
            background-color: transparent;
        }}

        button {{
            border: 1px solid alpha({1}, 0.1);
            background-color: alpha({1}, 0.05);
            color: {1};
            border-radius: 4px;
        }}
        button:hover, dropdown > button:hover {{
            background-color: alpha({1}, 0.1);
        }}

        /* Notebook and Tab specific styling */
        notebook > header {{
            background-color: alpha({1}, 0.03);
            border-bottom: 1px solid alpha({1}, 0.1);
        }}
        notebook tab {{
            background-color: transparent;
            color: alpha({1}, 0.6);
            border: none;
            padding: 8px 12px;
        }}
        notebook tab:checked {{
            color: {1};
            background-color: alpha({1}, 0.1);
            border-bottom: 2px solid {1};
        }}

        label, label.title {{ 
            background-color: transparent; 
            color: inherit; 
        }}
        "#,
        bg, fg, hb_bg
    );

    thread_local! {
        static APP_THEME_PROVIDER: CssProvider = CssProvider::new();
        static PROVIDER_ADDED: std::cell::Cell<bool> = std::cell::Cell::new(false);
    }

    APP_THEME_PROVIDER.with(|provider| {
        provider.load_from_data(&css);
        if !PROVIDER_ADDED.get() {
            gtk::style_context_add_provider_for_display(
                &display,
                provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
            PROVIDER_ADDED.set(true);
        }
    });
}

fn show_settings_window(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("App Settings")
        .default_width(310)
        .default_height(220)
        .modal(true)
        .build();
    window.add_css_class("main-app-window");

    let vbox = GtkBox::new(Orientation::Vertical, 12);
    vbox.set_margin_top(20); vbox.set_margin_bottom(20); 
    vbox.set_margin_start(20); vbox.set_margin_end(20);

    let theme_label = Label::new(Some("Select App Theme:"));
    vbox.append(&theme_label);

    let theme_model = StringList::new(&["Light", "Dark", "Dark Blue", "Coffee"]);
    let theme_dropdown = DropDown::builder().model(&theme_model).build();
    
    let current_cfg = load_app_config();
    let idx = match current_cfg.theme.as_str() {
        "Light" => 0,
        "Dark" => 1,
        "Dark Blue" => 2,
        "Coffee" => 3,
        _ => 1,
    };
    theme_dropdown.set_selected(idx);
    vbox.append(&theme_dropdown);

    let save_btn = Button::builder().label("Apply & Save").css_classes(["suggested-action"]).build();
    let win_weak = window.downgrade();
    save_btn.connect_clicked(move |_| {
        if let Some(item) = theme_dropdown.selected_item() {
            if let Ok(strobj) = item.downcast::<gtk::StringObject>() {
                let text = strobj.string().to_string();
                save_app_config(&AppConfig { theme: text.clone() });
                
                // Change theme in next idle loop to avoid "Broken accounting" during window closure
                glib::idle_add_local(move || {
                    apply_app_theme(&text);
                    glib::ControlFlow::Break
                });
            }
        }
        if let Some(w) = win_weak.upgrade() {
            w.close();
        }
    });

    vbox.append(&save_btn);
    window.set_child(Some(&vbox));
    window.present();
}

fn setup_sessions_actions(app: &Application, menu: &gio::Menu) {
    let sessions = crate::config::load_sessions();
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
    window.add_css_class("main-app-window");
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

    let settings_nb = Notebook::builder().vexpand(true).build();
    connection_box.append(&settings_nb);

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

    let row1 = GtkBox::new(Orientation::Horizontal, 10);
    let host_entry = Entry::builder().placeholder_text("Host Address (e.g. 1.2.3.4)").hexpand(true).build();
    let port_entry = Entry::builder().placeholder_text("Port").max_length(5).width_chars(5).max_width_chars(5).text("22").halign(gtk::Align::Start).build();
    let method_model = StringList::new(&["Password", "Private Key"]);
    let method_dropdown = DropDown::builder().model(&method_model).build();

    row1.append(&host_entry);
    row1.append(&port_entry);
    row1.append(&method_dropdown);
    conn_page.append(&row1);

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
    key_box.set_visible(false); 
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

    let list = ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);

    let sessions_data = crate::config::load_sessions();
    let sessions_arc = Arc::new(Mutex::new(sessions_data.clone()));

    let app_clone = app.clone();
    let host_e = host_entry.clone();
    let port_e = port_entry.clone();
    let user_e = user_entry.clone();
    let pass_e = pass_entry.clone();
    let name_e = name_entry.clone();
    
    let style_page = GtkBox::new(Orientation::Vertical, 12);
    style_page.set_margin_top(20); style_page.set_margin_bottom(20); style_page.set_margin_start(20); style_page.set_margin_end(20);
    
    let font_model = StringList::new(&["6", "8", "10", "12", "14", "16", "18", "20", "24", "28", "32"]);
    let font_d = DropDown::builder().model(&font_model).build();
    let mut default_font_idx = 4;
    for i in 0..font_model.n_items() {
        if let Some(s) = font_model.string(i) {
            if s == "14" { default_font_idx = i; break; }
        }
    }
    font_d.set_selected(default_font_idx);
    
    let cur_model = StringList::new(&["Block", "Underline", "I-Beam"]);
    let cur_d = DropDown::builder().model(&cur_model).build();
    
    let theme_modelstr = THEMES.iter().map(|t| t.name).collect::<Vec<&str>>();
    let theme_model = StringList::new(&theme_modelstr);
    let theme_d = DropDown::builder().model(&theme_model).build();
    
    let fg_b = ColorButton::with_rgba(&hex_to_rgba("#00ff00"));
    let bg_b = ColorButton::with_rgba(&hex_to_rgba("#000000"));
    let blink_c = CheckButton::builder().label("Blinking Cursor").active(true).build();
    
    let basic_grid = Grid::builder().row_spacing(10).column_spacing(10).build();
    basic_grid.attach(&Label::new(Some("Theme:")), 0, 0, 1, 1);
    basic_grid.attach(&theme_d, 1, 0, 1, 1);
    basic_grid.attach(&Label::new(Some("Text Color:")), 0, 1, 1, 1);
    basic_grid.attach(&fg_b, 1, 1, 1, 1);
    basic_grid.attach(&Label::new(Some("Background:")), 0, 2, 1, 1);
    basic_grid.attach(&bg_b, 1, 2, 1, 1);
    basic_grid.attach(&Label::new(Some("Font Size:")), 0, 3, 1, 1);
    basic_grid.attach(&font_d, 1, 3, 1, 1);
    basic_grid.attach(&Label::new(Some("Cursor Shape:")), 0, 4, 1, 1);
    basic_grid.attach(&cur_d, 1, 4, 1, 1);
    basic_grid.attach(&blink_c, 1, 5, 1, 1);
    style_page.append(&basic_grid);

    let pal_grid = Grid::builder().row_spacing(5).column_spacing(5).margin_top(15).build();
    pal_grid.attach(&Label::builder().label("<b>ANSI Color Palette</b>").use_markup(true).margin_bottom(5).build(), 0, 0, 8, 1);
    
    let mut palette_btns = Vec::new();
    let default_pal = THEMES[0].palette;
    for i in 0..16 {
        let btn = ColorButton::with_rgba(&hex_to_rgba(default_pal[i]));
        pal_grid.attach(&btn, (i % 8) as i32, (1 + i / 8) as i32, 1, 1);
        palette_btns.push(btn);
    }
    style_page.append(&pal_grid);
    
    let fg_weak = fg_button_downgrade(&fg_b);
    let bg_weak = bg_button_downgrade(&bg_b);
    let pal_weaks: Vec<_> = palette_btns.iter().map(|b| b.downgrade()).collect();
    
    theme_d.connect_selected_notify(move |d| {
        let idx = d.selected() as usize;
        if idx < THEMES.len() {
            let t = &THEMES[idx];
            if let Some(fg) = fg_weak.upgrade() { fg.set_rgba(&hex_to_rgba(t.fg)); }
            if let Some(bg) = bg_weak.upgrade() { bg.set_rgba(&hex_to_rgba(t.bg)); }
            for (i, pw) in pal_weaks.iter().enumerate() {
                if let Some(pb) = pw.upgrade() {
                    pb.set_rgba(&hex_to_rgba(t.palette[i]));
                }
            }
        }
    });

    let extra_page = GtkBox::new(Orientation::Vertical, 12);
    extra_page.set_margin_top(20); extra_page.set_margin_bottom(20); extra_page.set_margin_start(20); extra_page.set_margin_end(20);
    
    let scroll_grid = Grid::builder().row_spacing(10).column_spacing(10).build();
    let scroll_e = Entry::builder().text("1000").build();
    scroll_grid.attach(&Label::new(Some("Scrollback Lines:")), 0, 0, 1, 1);
    scroll_grid.attach(&scroll_e, 1, 0, 1, 1);
    
    let ka_e = Entry::builder().text("0").tooltip_text("Seconds between keepalive packets (0 to disable)").build();
    scroll_grid.attach(&Label::new(Some("Keepalive Interval:")), 0, 1, 1, 1);
    scroll_grid.attach(&ka_e, 1, 1, 1, 1);
    
    let ag_c = CheckButton::builder().label("Enable SSH Agent Forwarding").build();
    scroll_grid.attach(&ag_c, 1, 2, 1, 1);
    
    let lf_e = Entry::builder().placeholder_text("ex. 8080:localhost:80").build();
    scroll_grid.attach(&Label::new(Some("-L Local Forwards:")), 0, 3, 1, 1);
    scroll_grid.attach(&lf_e, 1, 3, 1, 1);

    let rf_e = Entry::builder().placeholder_text("ex. 8080:localhost:80").build();
    scroll_grid.attach(&Label::new(Some("-R Remote Forwards:")), 0, 4, 1, 1);
    scroll_grid.attach(&rf_e, 1, 4, 1, 1);
    
    extra_page.append(&scroll_grid);

    settings_nb.append_page(&conn_page, Some(&Label::new(Some("Connection"))));
    settings_nb.append_page(&style_page, Some(&Label::new(Some("Appearance"))));
    settings_nb.append_page(&extra_page, Some(&Label::new(Some("Advanced"))));

    let scroll_list = ScrolledWindow::builder().child(&list).vexpand(true).min_content_height(250).build();
    conn_page.append(&scroll_list);

    let list_clone = list.clone();
    let sp_clone = save_pass_check.clone();
    let fg_clone = fg_b.clone();
    let bg_clone = bg_b.clone();
    let fd_clone = font_d.clone();
    let cd_clone = cur_d.clone();
    let bc_clone = blink_c.clone();
    let sc_clone = scroll_e.clone();
    let arc_for_populate = sessions_arc.clone();
    let pal_clone = palette_btns.clone();
    let key_e_pop = key_entry.clone();
    let theme_d_pop = theme_d.clone();
    let ka_e_pop = ka_e.clone();
    let ag_c_pop = ag_c.clone();
    let method_d_pop = method_dropdown.clone();
    let lf_e_pop = lf_e.clone();
    let rf_e_pop = rf_e.clone();
    
    populate_list(&list, &sessions_data, &name_e, &host_e, &port_e, &user_e, &pass_e, &sp_clone, &fg_clone, &bg_clone, &fd_clone, &cd_clone, &bc_clone, &sc_clone, &pal_clone, arc_for_populate, &key_e_pop, &theme_d_pop, &ka_e_pop, &ag_c_pop, &method_d_pop, &lf_e_pop, &rf_e_pop);

    let s_arc_save = sessions_arc.clone();
    let n_e_save = name_entry.clone();
    let h_e_save = host_entry.clone();
    let p_e_save = port_entry.clone();
    let u_e_save = user_entry.clone();
    let ps_e_save = pass_entry.clone();
    let sp_save = save_pass_check.clone();
    let th_save = theme_d.clone();
    let fg_save = fg_b.clone();
    let bg_save = bg_b.clone();
    let fd_save = font_d.clone();
    let cd_save = cur_d.clone();
    let bc_save = blink_c.clone();
    let sc_save = scroll_e.clone();
    let ka_save = ka_e.clone();
    let ac_save = ag_c.clone();
    let pal_save = palette_btns.clone();
    let method_save = method_dropdown.clone();
    let term_save = term_dropdown.clone();
    let key_e_save = key_entry.clone();
    let lf_save = lf_e.clone();
    let rf_save = rf_e.clone();

    save_btn.connect_clicked(move |_| {
        let name = n_e_save.text().to_string();
        let host = h_e_save.text().to_string();
        let user = u_e_save.text().to_string();
        if host.is_empty() || user.is_empty() { return; }
        let session_name = if name.is_empty() { format!("{}@{}", user, host) } else { name };

        let mut pal = Vec::new();
        for btn in &pal_save { pal.push(rgba_to_hex(btn.rgba())); }

        let settings = ConnectionSettings {
            name: session_name,
            host: host,
            port: p_e_save.text().parse().unwrap_or(22),
            username: user,
            password: if sp_save.is_active() && method_save.selected() == 0 { Some(ps_e_save.text().to_string()) } else { None },
            fg_color: rgba_to_hex(fg_save.rgba()),
            bg_color: rgba_to_hex(bg_save.rgba()),
            font_size: fd_save.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().parse().unwrap_or(14)).unwrap_or(14),
            palette: pal,
            cursor_style: cd_save.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_else(|| "Block".to_string()),
            cursor_blink: bc_save.is_active(),
            scrollback: sc_save.text().parse().unwrap_or(1000),
            private_key: if method_save.selected() == 1 { Some(key_e_save.text().to_string()) } else { None },
            keepalive: ka_save.text().parse().unwrap_or(0),
            agent_forwarding: ac_save.is_active(),
            theme: th_save.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_else(|| "Custom".to_string()),
            method: method_save.selected(),
            term_type: term_save.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_else(|| "xterm-256color".to_string()),
            local_forwards: lf_save.text().to_string(),
            remote_forwards: rf_save.text().to_string(),
        };

        let mut s_vec = s_arc_save.lock().unwrap();
        if let Some(existing) = s_vec.iter_mut().find(|s| s.name == settings.name) {
            *existing = settings.clone();
        } else {
            s_vec.push(settings.clone());
        }
        crate::config::save_sessions(&s_vec);
        populate_list(&list_clone, &s_vec, &n_e_save, &h_e_save, &p_e_save, &u_e_save, &ps_e_save, &sp_save, &fg_save, &bg_save, &fd_save, &cd_save, &bc_save, &sc_save, &pal_save, s_arc_save.clone(), &key_e_save, &th_save, &ka_save, &ac_save, &method_save, &lf_save, &rf_save);
    });

    let pal_weaks: Vec<_> = palette_btns.iter().map(|b| b.downgrade()).collect();
    let term_weak = term_dropdown.downgrade();
    let lf_conn = lf_e.clone();
    let rf_conn = rf_e.clone();
    connect_btn.connect_clicked(move |_| {
        let name_str = name_e.text().to_string();
        let host = host_e.text().to_string();
        let name = if name_str.is_empty() { format!("{}@{}", user_e.text(), host) } else { name_str };
        let port = port_e.text().parse::<u16>().unwrap_or(22);
        let user = user_e.text().to_string();
        let pass = pass_e.text().to_string();
        let key = key_e_pop.text().to_string();
        let lf_str = lf_conn.text().to_string();
        let rf_str = rf_conn.text().to_string();
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
        
        let term_item = term_weak.upgrade().and_then(|d| d.selected_item()).and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_else(|| "xterm-256color".to_string());
        
        handle_connect(&app_clone, &ConnectionSettings {
            name: name, host, port, username: user,
            password: if pass.is_empty() { None } else { Some(pass.clone()) },
            fg_color: fg, bg_color: bg, font_size,
            palette: if pal.len() == 16 { pal } else { crate::config::THEMES[0].palette.iter().map(|s| s.to_string()).collect() },
            cursor_style: cur_style, cursor_blink: blink, scrollback: scroll,
            private_key: if method_dropdown.selected() == 1 { Some(key) } else { None },
            keepalive, agent_forwarding: agent, theme: theme_name,
            method: method_dropdown.selected(),
            term_type: term_item,
            local_forwards: lf_str,
            remote_forwards: rf_str,
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
