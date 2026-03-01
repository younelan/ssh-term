use crate::config::{ConnectionSettings, THEMES};
use crate::app_state::update_active_terminals;
use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{glib, Box as GtkBox, Button, CheckButton, ColorButton, DropDown, Entry, Label, ListBox, MenuButton, Orientation, StringList};
use std::sync::{Arc, Mutex};

pub fn populate_list(
    list: &ListBox, 
    sessions: &[ConnectionSettings], 
    name_e: &Entry, 
    host_e: &Entry, 
    port_e: &Entry, 
    user_e: &Entry, 
    pass_e: &Entry, 
    save_p: &CheckButton, 
    fg_b: &ColorButton, 
    bg_b: &ColorButton, 
    font_d: &DropDown, 
    cur_d: &DropDown, 
    blink_c: &CheckButton, 
    scroll_e: &Entry, 
    palette_btns: &[ColorButton], 
    sessions_arc: Arc<Mutex<Vec<ConnectionSettings>>>, 
    key_e: &Entry, 
    theme_d: &DropDown, 
    ka_e: &Entry, 
    ag_c: &CheckButton, 
    method_d: &DropDown,
    lf_e: &Entry,
    rf_e: &Entry
) {
    while let Some(child) = list.first_child() { list.remove(&child); }
    for (index, s) in sessions.iter().enumerate() {
        let row_box = GtkBox::new(Orientation::Horizontal, 10);
        row_box.add_css_class("session-row");
        let label = Label::builder().label(&s.name).halign(gtk::Align::Start).hexpand(true).build();
        row_box.append(&label);
        
        let menu_btn = MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("Session Options")
            .css_classes(["flat"])
            .halign(gtk::Align::End)
            .build();
        row_box.append(&menu_btn);

        let menu = gtk::gio::Menu::new();
        let action_group = gtk::gio::SimpleActionGroup::new();
        menu_btn.insert_action_group("row", Some(&action_group));
        menu_btn.set_menu_model(Some(&menu));

        menu.append(Some("Save Current Settings"), Some("row.save"));
        menu.append(Some("Rename Session"), Some("row.rename"));
        menu.append(Some("Clone Session"), Some("row.clone"));
        menu.append(Some("Delete"), Some("row.delete"));

        let row = gtk::ListBoxRow::builder().child(&row_box).build();
        list.append(&row);
        

        let name_e_weak = name_e.downgrade();
        let h_e_weak = host_e.downgrade();
        let p_e_weak = port_e.downgrade();
        let u_e_weak = user_entry_downgrade(user_e); 
        let ps_e_weak = pass_entry_downgrade(pass_e);
        let save_p_weak = save_p.downgrade();
        let fg_weak = fg_button_downgrade(fg_b);
        let bg_weak = bg_button_downgrade(bg_b);
        let font_weak = font_dropdown_downgrade(font_d);
        let cur_weak = cur_dropdown_downgrade(cur_d);
        let blink_weak = blink_check_downgrade(blink_c);
        let scroll_weak = scroll_entry_downgrade(scroll_e);
        let key_e_weak = key_e.downgrade();
        let theme_d_weak = theme_d.downgrade();
        let ka_e_weak = ka_e.downgrade();
        let ag_c_weak = ag_c.downgrade();
        let method_d_weak = method_d.downgrade();
        let lf_e_weak = lf_e.downgrade();
        let rf_e_weak = rf_e.downgrade();
        let p_buttons = palette_btns.to_vec();
        let name_clone = s.name.clone();

        let s_arc_for_save = sessions_arc.clone();
        let n_e_w2 = name_e_weak.clone();
        let h_e_w2 = h_e_weak.clone();
        let p_e_w2 = p_e_weak.clone();
        let u_e_w2 = u_e_weak.clone();
        let ps_e_w2 = ps_e_weak.clone();
        let sp_w2 = save_p_weak.clone();
        let th_w2 = theme_d_weak.clone();
        let fg_w2 = fg_weak.clone();
        let bg_w2 = bg_weak.clone();
        let f_d_w2 = font_weak.clone();
        let c_d_w2 = cur_weak.clone();
        let bc_w2 = blink_weak.clone();
        let sc_w2 = scroll_weak.clone();
        let ka_w2 = ka_e_weak.clone();
        let ac_w2 = ag_c_weak.clone();
        let ls_w2 = list.downgrade();
        let ke_w2 = key_e_weak.clone();
        let pb_w2 = p_buttons.clone();
        let method_d_w2 = method_d_weak.clone();
        let lf_w2 = lf_e_weak.clone();
        let rf_w2 = rf_e_weak.clone();

        let action_save = gtk::gio::SimpleAction::new("save", None);
        action_save.connect_activate(move |_, _| {
            let h_e = match h_e_w2.upgrade() { Some(v) => v, None => return };
            let p_e = match p_e_w2.upgrade() { Some(v) => v, None => return };
            let u_e = match u_e_w2.upgrade() { Some(v) => v, None => return };
            let ps_e = match ps_e_w2.upgrade() { Some(v) => v, None => return };
            let save_p_c = match sp_w2.upgrade() { Some(v) => v, None => return };
            let theme_d = match th_w2.upgrade() { Some(v) => v, None => return };
            let fg_b = match fg_w2.upgrade() { Some(v) => v, None => return };
            let bg_b = match bg_w2.upgrade() { Some(v) => v, None => return };
            let font_d = match f_d_w2.upgrade() { Some(v) => v, None => return };
            let cur_d = match c_d_w2.upgrade() { Some(v) => v, None => return };
            let blink_c = match bc_w2.upgrade() { Some(v) => v, None => return };
            let scroll_e = match sc_w2.upgrade() { Some(v) => v, None => return };
            let ka_e = match ka_w2.upgrade() { Some(v) => v, None => return };
            let ag_c = match ac_w2.upgrade() { Some(v) => v, None => return };
            let list = match ls_w2.upgrade() { Some(v) => v, None => return };
            let key_e_up = match ke_w2.upgrade() { Some(v) => v, None => return };
            let method_d_up = match method_d_w2.upgrade() { Some(v) => v, None => return };
            let name_e_up = match n_e_w2.upgrade() { Some(v) => v, None => return };
            let lf_e = match lf_w2.upgrade() { Some(v) => v, None => return };
            let rf_e = match rf_w2.upgrade() { Some(v) => v, None => return };
            
            let mut pal = Vec::new();
            for btn in &pb_w2 { pal.push(rgba_to_hex(btn.rgba())); }

            let settings = ConnectionSettings {
                name: name_clone.clone(),
                host: h_e.text().to_string(),
                port: p_e.text().parse().unwrap_or(22),
                username: u_e.text().to_string(),
                password: if save_p_c.is_active() && method_d_up.selected() == 0 { Some(ps_e.text().to_string()) } else { None },
                fg_color: rgba_to_hex(fg_b.rgba()),
                bg_color: rgba_to_hex(bg_b.rgba()),
                font_size: font_d.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().parse().unwrap_or(14)).unwrap_or(14),
                palette: pal,
                cursor_style: cur_d.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_else(|| "Block".to_string()),
                cursor_blink: blink_c.is_active(),
                scrollback: scroll_e.text().parse().unwrap_or(1000),
                private_key: if method_d_up.selected() == 1 { Some(key_e_up.text().to_string()) } else { None },
                keepalive: ka_e.text().parse().unwrap_or(0),
                agent_forwarding: ag_c.is_active(),
                theme: theme_d.selected_item().and_then(|i| i.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_else(|| "Custom".to_string()),
                method: method_d_up.selected(),
                term_type: s_arc_for_save.lock().unwrap().get(index).map(|s| s.term_type.clone()).unwrap_or_else(|| "xterm-256color".to_string()),
                local_forwards: lf_e.text().to_string(),
                remote_forwards: rf_e.text().to_string(),
            };

            let mut s_vec = s_arc_for_save.lock().unwrap();
            if index < s_vec.len() {
                s_vec[index] = settings.clone();
                crate::config::save_sessions(&s_vec);
                populate_list(&list, &s_vec, &name_e_up, &h_e, &p_e, &u_e, &ps_e, &save_p_c, &fg_b, &bg_b, &font_d, &cur_d, &blink_c, &scroll_e, &pb_w2, s_arc_for_save.clone(), &key_e_up, &theme_d, &ka_e, &ag_c, &method_d_up, &lf_e, &rf_e);
                update_active_terminals(&settings.name, &settings);
            }
        });
        action_group.add_action(&action_save);

        let action_delete = gtk::gio::SimpleAction::new("delete", None);
        let list_w3 = list.downgrade();
        let n_e_w_del = name_e_weak.clone();
        let h_e_w_del = h_e_weak.clone();
        let p_e_w_del = p_e_weak.clone();
        let u_e_w_del = u_e_weak.clone();
        let ps_e_w_del = ps_e_weak.clone();
        let sp_w_del = save_p_weak.clone();
        let fg_w_del = fg_weak.clone();
        let bg_w_del = bg_weak.clone();
        let f_d_w_del = font_weak.clone();
        let c_d_w_del = cur_weak.clone();
        let bc_w_del = blink_weak.clone();
        let sc_w_del = scroll_weak.clone();
        let ke_w_del = key_e_weak.clone();
        let th_w_del = theme_d_weak.clone();
        let ka_w_del = ka_e_weak.clone();
        let ac_w_del = ag_c_weak.clone();
        let md_w_del = method_d_weak.clone();
        let s_arc_for_del = sessions_arc.clone();
        let pb_w_del = p_buttons.clone();
        let lf_w_del = lf_e_weak.clone();
        let rf_w_del = rf_e_weak.clone();
        action_delete.connect_activate(move |_, _| {
            let list_up = match list_w3.upgrade() { Some(v) => v, None => return };
            let name_up = match n_e_w_del.upgrade() { Some(v) => v, None => return };
            let h_e = match h_e_w_del.upgrade() { Some(v) => v, None => return };
            let p_e = match p_e_w_del.upgrade() { Some(v) => v, None => return };
            let u_e = match u_e_w_del.upgrade() { Some(v) => v, None => return };
            let ps_e = match ps_e_w_del.upgrade() { Some(v) => v, None => return };
            let save_p_up = match sp_w_del.upgrade() { Some(v) => v, None => return };
            let fg = match fg_w_del.upgrade() { Some(v) => v, None => return };
            let bg = match bg_w_del.upgrade() { Some(v) => v, None => return };
            let font = match f_d_w_del.upgrade() { Some(v) => v, None => return };
            let cur = match c_d_w_del.upgrade() { Some(v) => v, None => return };
            let blink = match bc_w_del.upgrade() { Some(v) => v, None => return };
            let scroll = match sc_w_del.upgrade() { Some(v) => v, None => return };
            let key_up = match ke_w_del.upgrade() { Some(v) => v, None => return };
            let theme_up = match th_w_del.upgrade() { Some(v) => v, None => return };
            let ka_up = match ka_w_del.upgrade() { Some(v) => v, None => return };
            let ag_up = match ac_w_del.upgrade() { Some(v) => v, None => return };
            let method_up = match md_w_del.upgrade() { Some(v) => v, None => return };
            let lf_e = match lf_w_del.upgrade() { Some(v) => v, None => return };
            let rf_e = match rf_w_del.upgrade() { Some(v) => v, None => return };
            
            let mut s = s_arc_for_del.lock().unwrap();
            if index < s.len() {
                s.remove(index);
                crate::config::save_sessions(&s);
                populate_list(&list_up, &s, &name_up, &h_e, &p_e, &u_e, &ps_e, &save_p_up, &fg, &bg, &font, &cur, &blink, &scroll, &pb_w_del, s_arc_for_del.clone(), &key_up, &theme_up, &ka_up, &ag_up, &method_up, &lf_e, &rf_e);
            }
        });
        action_group.add_action(&action_delete);

        // Rename Action
        let s_arc_ren = sessions_arc.clone();
        let list_w_ren = list.downgrade();
        let n_e_w_ren = name_e_weak.clone();
        let win_weak_ren = find_parent_window(list); 
        let action_rename = gtk::gio::SimpleAction::new("rename", None);
        let current_name = s.name.clone();
        
        // Re-clone UI references for the dialog closure
        let h_e_ren = h_e_weak.clone();
        let p_e_ren = p_e_weak.clone();
        let u_e_ren = u_e_weak.clone();
        let ps_e_ren = ps_e_weak.clone();
        let sp_ren = save_p_weak.clone();
        let fg_ren = fg_weak.clone();
        let bg_ren = bg_weak.clone();
        let f_d_ren = font_weak.clone();
        let c_d_ren = cur_weak.clone();
        let bc_ren = blink_weak.clone();
        let sc_ren = scroll_weak.clone();
        let pb_ren = p_buttons.clone();
        let ke_ren = key_e_weak.clone();
        let th_ren = theme_d_weak.clone();
        let ka_ren = ka_e_weak.clone();
        let ac_ren = ag_c_weak.clone();
        let md_ren = method_d_weak.clone();
        let lf_ren = lf_e_weak.clone();
        let rf_ren = rf_e_weak.clone();

        action_rename.connect_activate(move |_, _| {
            let win = match win_weak_ren.upgrade() { Some(w) => w, None => return };
            let dialog = gtk::MessageDialog::builder()
                .transient_for(&win)
                .modal(true)
                .message_type(gtk::MessageType::Question)
                .buttons(gtk::ButtonsType::OkCancel)
                .text("Rename Session")
                .secondary_text("Enter the new name for this session:")
                .build();
                
            let entry = Entry::builder().text(&current_name).margin_top(10).margin_bottom(10).margin_start(10).margin_end(10).build();
            dialog.content_area().append(&entry);
            entry.grab_focus();
            
            let s_arc_ren_inner = s_arc_ren.clone();
            let n_e_weak_ren_inner = n_e_w_ren.clone();
            let l_w_ren_inner = list_w_ren.clone();
            
            let h_e_ren_i = h_e_ren.clone();
            let p_e_ren_i = p_e_ren.clone();
            let u_e_ren_i = u_e_ren.clone();
            let ps_e_ren_i = ps_e_ren.clone();
            let sp_ren_i = sp_ren.clone();
            let fg_ren_i = fg_ren.clone();
            let bg_ren_i = bg_ren.clone();
            let f_d_ren_i = f_d_ren.clone();
            let c_d_ren_i = c_d_ren.clone();
            let bc_ren_i = bc_ren.clone();
            let sc_ren_i = sc_ren.clone();
            let pb_ren_i = pb_ren.clone();
            let ke_ren_i = ke_ren.clone();
            let th_ren_i = th_ren.clone();
            let ka_ren_i = ka_ren.clone();
            let ac_ren_i = ac_ren.clone();
            let md_ren_i = md_ren.clone();
            let lf_ren_i = lf_ren.clone();
            let rf_ren_i = rf_ren.clone();

            dialog.connect_response(move |d, res| {
                if res == gtk::ResponseType::Ok {
                    let new_name = entry.text().to_string();
                    if !new_name.trim().is_empty() {
                        let mut s = s_arc_ren_inner.lock().unwrap();
                        if index < s.len() {
                            s[index].name = new_name;
                            crate::config::save_sessions(&s);
                            
                            if let (Some(ls_up), Some(ne_up), Some(he_up), Some(pe_up), Some(ue_up), Some(pse_up), Some(sp_up), Some(fg_up), Some(bg_up), Some(fd_up), Some(cd_up), Some(bc_up), Some(sc_up), Some(ke_up), Some(th_up), Some(ka_up), Some(ac_up), Some(md_up), Some(lf_up), Some(rf_up)) = (
                                l_w_ren_inner.upgrade(), n_e_weak_ren_inner.upgrade(), h_e_ren_i.upgrade(), p_e_ren_i.upgrade(), u_e_ren_i.upgrade(), ps_e_ren_i.upgrade(), sp_ren_i.upgrade(), fg_ren_i.upgrade(), bg_ren_i.upgrade(), f_d_ren_i.upgrade(), c_d_ren_i.upgrade(), bc_ren_i.upgrade(), sc_ren_i.upgrade(), ke_ren_i.upgrade(), th_ren_i.upgrade(), ka_ren_i.upgrade(), ac_ren_i.upgrade(), md_ren_i.upgrade(), lf_ren_i.upgrade(), rf_ren_i.upgrade()
                            ) {
                                populate_list(&ls_up, &s, &ne_up, &he_up, &pe_up, &ue_up, &pse_up, &sp_up, &fg_up, &bg_up, &fd_up, &cd_up, &bc_up, &sc_up, &pb_ren_i, s_arc_ren_inner.clone(), &ke_up, &th_up, &ka_up, &ac_up, &md_up, &lf_up, &rf_up);
                            }
                        }
                    }
                }
                d.destroy();
            });
            dialog.show();
        });
        action_group.add_action(&action_rename);

        // Clone Action
        let s_arc_cl = sessions_arc.clone();
        let list_w_cl = list.downgrade();
        let n_e_w_cl = name_e_weak.clone();
        let win_weak_cl = find_parent_window(list); 
        let action_clone = gtk::gio::SimpleAction::new("clone", None);
        let s_to_clone = s.clone();
        
        let h_e_cl = h_e_weak.clone();
        let p_e_cl = p_e_weak.clone();
        let u_e_cl = u_e_weak.clone();
        let ps_e_cl = ps_e_weak.clone();
        let sp_cl = save_p_weak.clone();
        let fg_cl = fg_weak.clone();
        let bg_cl = bg_weak.clone();
        let f_d_cl = font_weak.clone();
        let c_d_cl = cur_weak.clone();
        let bc_cl = blink_weak.clone();
        let sc_cl = scroll_weak.clone();
        let pb_cl = p_buttons.clone();
        let ke_cl = key_e_weak.clone();
        let th_cl = theme_d_weak.clone();
        let ka_cl = ka_e_weak.clone();
        let ac_cl = ag_c_weak.clone();
        let md_cl = method_d_weak.clone();
        let lf_cl = lf_e_weak.clone();
        let rf_cl = rf_e_weak.clone();

        action_clone.connect_activate(move |_, _| {
            let win = match win_weak_cl.upgrade() { Some(w) => w, None => return };
            let dialog = gtk::MessageDialog::builder()
                .transient_for(&win)
                .modal(true)
                .message_type(gtk::MessageType::Question)
                .buttons(gtk::ButtonsType::OkCancel)
                .text("Clone Session")
                .secondary_text("Enter a name for the new cloned session:")
                .build();
                
            let default_clone_name = format!("{} (Copy)", s_to_clone.name);
            let entry = Entry::builder().text(&default_clone_name).margin_top(10).margin_bottom(10).margin_start(10).margin_end(10).build();
            dialog.content_area().append(&entry);
            entry.grab_focus();
            
            let s_arc_cl_inner = s_arc_cl.clone();
            let stc_inner = s_to_clone.clone();
            let l_w_cl_inner = list_w_cl.clone();
            let n_e_w_cl_inner = n_e_w_cl.clone();
            
            let h_e_cl_i = h_e_cl.clone();
            let p_e_cl_i = p_e_cl.clone();
            let u_e_cl_i = u_e_cl.clone();
            let ps_e_cl_i = ps_e_cl.clone();
            let sp_cl_i = sp_cl.clone();
            let fg_cl_i = fg_cl.clone();
            let bg_cl_i = bg_cl.clone();
            let f_d_cl_i = f_d_cl.clone();
            let c_d_cl_i = c_d_cl.clone();
            let bc_cl_i = bc_cl.clone();
            let sc_cl_i = sc_cl.clone();
            let pb_cl_i = pb_cl.clone();
            let ke_cl_i = ke_cl.clone();
            let th_cl_i = th_cl.clone();
            let ka_cl_i = ka_cl.clone();
            let ac_cl_i = ac_cl.clone();
            let md_cl_i = md_cl.clone();
            let lf_cl_i = lf_cl.clone();
            let rf_cl_i = rf_cl.clone();

            dialog.connect_response(move |d, res| {
                if res == gtk::ResponseType::Ok {
                    let new_name = entry.text().to_string();
                    if !new_name.trim().is_empty() {
                        let mut cloned_settings = stc_inner.clone();
                        cloned_settings.name = new_name;
                        
                        let mut s = s_arc_cl_inner.lock().unwrap();
                        s.push(cloned_settings);
                        crate::config::save_sessions(&s);
                        
                        if let (Some(ls_up), Some(ne_up), Some(he_up), Some(pe_up), Some(ue_up), Some(pse_up), Some(sp_up), Some(fg_up), Some(bg_up), Some(fd_up), Some(cd_up), Some(bc_up), Some(sc_up), Some(ke_up), Some(th_up), Some(ka_up), Some(ac_up), Some(md_up), Some(lf_up), Some(rf_up)) = (
                            l_w_cl_inner.upgrade(), n_e_w_cl_inner.upgrade(), h_e_cl_i.upgrade(), p_e_cl_i.upgrade(), u_e_cl_i.upgrade(), ps_e_cl_i.upgrade(), sp_cl_i.upgrade(), fg_cl_i.upgrade(), bg_cl_i.upgrade(), f_d_cl_i.upgrade(), c_d_cl_i.upgrade(), bc_cl_i.upgrade(), sc_cl_i.upgrade(), ke_cl_i.upgrade(), th_cl_i.upgrade(), ka_cl_i.upgrade(), ac_cl_i.upgrade(), md_cl_i.upgrade(), lf_cl_i.upgrade(), rf_cl_i.upgrade()
                        ) {
                            populate_list(&ls_up, &s, &ne_up, &he_up, &pe_up, &ue_up, &pse_up, &sp_up, &fg_up, &bg_up, &fd_up, &cd_up, &bc_up, &sc_up, &pb_cl_i, s_arc_cl_inner.clone(), &ke_up, &th_up, &ka_up, &ac_up, &md_up, &lf_up, &rf_up);
                        }
                    }
                }
                d.destroy();
            });
            dialog.show();
        });
        action_group.add_action(&action_clone);

        // Right-Click Context Menu for Theme
        row_box.insert_action_group("row", Some(&action_group));
        let context_menu = gtk::gio::Menu::new();
        let theme_submenu = gtk::gio::Menu::new();
        
        for (theme_idx, theme) in THEMES.iter().enumerate() {
            let action_name = format!("set_theme_{}", theme_idx);
            theme_submenu.append(Some(theme.name), Some(&format!("row.{}", action_name)));

            let action_theme = gtk::gio::SimpleAction::new(&action_name, None);
            let s_arc_th = sessions_arc.clone();
            let theme_name = theme.name.to_string();
            let theme_fg = theme.fg.to_string();
            let theme_bg = theme.bg.to_string();
            let theme_pal = theme.palette.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            
            let l_w_th = list.downgrade();
            let n_e_w_th = name_e_weak.clone();
            let h_e_w_th = h_e_weak.clone();
            let p_e_w_th = p_e_weak.clone();
            let u_e_w_th = u_e_weak.clone();
            let ps_e_w_th = ps_e_weak.clone();
            let sp_w_th = save_p_weak.clone();
            let fg_w_th = fg_weak.clone();
            let bg_w_th = bg_weak.clone();
            let f_d_w_th = font_weak.clone();
            let c_d_w_th = cur_weak.clone();
            let bc_w_th = blink_weak.clone();
            let sc_w_th = scroll_weak.clone();
            let pb_w_th = p_buttons.clone();
            let ke_w_th = key_e_weak.clone();
            let th_w_th = theme_d_weak.clone();
            let ka_w_th = ka_e_weak.clone();
            let ac_w_th = ag_c_weak.clone();
            let md_w_th = method_d_weak.clone();
            let lf_w_th = lf_e_weak.clone();
            let rf_w_th = rf_e_weak.clone();

            action_theme.connect_activate(move |_, _| {
                let mut s = s_arc_th.lock().unwrap();
                if index < s.len() {
                    s[index].theme = theme_name.clone();
                    s[index].fg_color = theme_fg.clone();
                    s[index].bg_color = theme_bg.clone();
                    s[index].palette = theme_pal.clone();
                    crate::config::save_sessions(&s);
                    
                    if let (Some(ls_up), Some(ne_up), Some(he_up), Some(pe_up), Some(ue_up), Some(pse_up), Some(sp_up), Some(fg_up), Some(bg_up), Some(fd_up), Some(cd_up), Some(bc_up), Some(sc_up), Some(ke_up), Some(th_up), Some(ka_up), Some(ac_up), Some(md_up), Some(lf_up), Some(rf_up)) = (
                        l_w_th.upgrade(), n_e_w_th.upgrade(), h_e_w_th.upgrade(), p_e_w_th.upgrade(), u_e_w_th.upgrade(), ps_e_w_th.upgrade(), sp_w_th.upgrade(), fg_w_th.upgrade(), bg_w_th.upgrade(), f_d_w_th.upgrade(), c_d_w_th.upgrade(), bc_w_th.upgrade(), sc_w_th.upgrade(), ke_w_th.upgrade(), th_w_th.upgrade(), ka_w_th.upgrade(), ac_w_th.upgrade(), md_w_th.upgrade(), lf_w_th.upgrade(), rf_w_th.upgrade()
                    ) {
                        populate_list(&ls_up, &s, &ne_up, &he_up, &pe_up, &ue_up, &pse_up, &sp_up, &fg_up, &bg_up, &fd_up, &cd_up, &bc_up, &sc_up, &pb_w_th, s_arc_th.clone(), &ke_up, &th_up, &ka_up, &ac_up, &md_up, &lf_up, &rf_up);
                    }
                }
            });
            action_group.add_action(&action_theme);
        }
        context_menu.append_submenu(Some("Quick Set Theme"), &theme_submenu);

        let popover = gtk::PopoverMenu::from_model(Some(&context_menu));
        popover.set_parent(&row_box);
        popover.set_has_arrow(false);
        
        let gesture = gtk::GestureClick::new();
        gesture.set_button(3); // Right click
        let p_weak = popover.downgrade();
        gesture.connect_pressed(move |_, _, _, _| {
            if let Some(p) = p_weak.upgrade() {
                p.popup();
            }
        });
        row_box.add_controller(gesture);
    }
    
    let sessions_vec = sessions.to_vec();
    let name_e_weak = name_e.downgrade();
    let h_e_weak = host_e.downgrade();
    let p_e_weak = port_e.downgrade();
    let u_e_weak = user_entry_downgrade(user_e); 
    let ps_e_weak = pass_entry_downgrade(pass_e);
    let save_p_weak = save_p.downgrade();
    let fg_weak = fg_button_downgrade(fg_b);
    let bg_weak = bg_button_downgrade(bg_b);
    let font_weak = font_dropdown_downgrade(font_d);
    let cur_weak = cur_dropdown_downgrade(cur_d);
    let blink_weak = blink_check_downgrade(blink_c);
    let scroll_weak = scroll_entry_downgrade(scroll_e);
    let key_e_weak = key_e.downgrade();
    let theme_d_weak = theme_d.downgrade();
    let ka_e_weak = ka_e.downgrade();
    let ag_c_weak = ag_c.downgrade();
    let method_d_weak = method_d.downgrade();
    let lf_e_weak = lf_e.downgrade();
    let rf_e_weak = rf_e.downgrade();
    let pal_buttons = palette_btns.to_vec();
    
    list.connect_row_activated(move |_, row| {
        let h_e = match h_e_weak.upgrade() { Some(v) => v, None => return };
        let p_e = match p_e_weak.upgrade() { Some(v) => v, None => return };
        let u_e = match upgrade_user_e(&u_e_weak) { Some(v) => v, None => return };
        let ps_e = match upgrade_pass_e(&ps_e_weak) { Some(v) => v, None => return };
        let save_p = match save_p_weak.upgrade() { Some(v) => v, None => return };
        let fg = match upgrade_fg_b(&fg_weak) { Some(v) => v, None => return };
        let bg = match upgrade_bg_b(&bg_weak) { Some(v) => v, None => return };
        let font = match upgrade_font_d(&font_weak) { Some(v) => v, None => return };
        let cur = match upgrade_cur_d(&cur_weak) { Some(v) => v, None => return };
        let blink = match upgrade_blink_c(&blink_weak) { Some(v) => v, None => return };
        let scroll = match upgrade_scroll_e(&scroll_weak) { Some(v) => v, None => return };
        let key_up = match key_e_weak.upgrade() { Some(v) => v, None => return };
        let theme_up = match theme_d_weak.upgrade() { Some(v) => v, None => return };
        let ka_up = match ka_e_weak.upgrade() { Some(v) => v, None => return };
        let ag_up = match ag_c_weak.upgrade() { Some(v) => v, None => return };
        let method_up = match method_d_weak.upgrade() { Some(v) => v, None => return };
        let lf_up = match lf_e_weak.upgrade() { Some(v) => v, None => return };
        let rf_up = match rf_e_weak.upgrade() { Some(v) => v, None => return };

        if let Some(s) = sessions_vec.get(row.index() as usize) {
            let n_e = match name_e_weak.upgrade() { Some(v) => v, None => return };
            n_e.set_text(&s.name);
            h_e.set_text(&s.host); p_e.set_text(&s.port.to_string()); u_e.set_text(&s.username);
            ps_e.set_text(s.password.as_deref().unwrap_or(""));
            key_up.set_text(s.private_key.as_deref().unwrap_or(""));
            save_p.set_active(s.password.is_some());
            method_up.set_selected(s.method);
            fg.set_rgba(&hex_to_rgba(&s.fg_color));
            bg.set_rgba(&hex_to_rgba(&s.bg_color));
            
            if let Some(model) = font.model().and_then(|m| m.downcast::<StringList>().ok()) {
                for i in 0..model.n_items() {
                    if let Some(str_obj) = model.string(i) {
                        if str_obj == s.font_size.to_string() { font.set_selected(i); break; }
                    }
                }
            }
            if let Some(model) = cur.model().and_then(|m| m.downcast::<StringList>().ok()) {
                for i in 0..model.n_items() {
                    if let Some(str_obj) = model.string(i) {
                        if str_obj == s.cursor_style { cur.set_selected(i); break; }
                    }
                }
            }
            if let Some(model) = theme_up.model().and_then(|m| m.downcast::<StringList>().ok()) {
                for i in 0..model.n_items() {
                    if let Some(str_obj) = model.string(i) {
                        if str_obj == s.theme { theme_up.set_selected(i); break; }
                    }
                }
            }
            blink.set_active(s.cursor_blink);
            scroll.set_text(&s.scrollback.to_string());
            ka_up.set_text(&s.keepalive.to_string());
            ag_up.set_active(s.agent_forwarding);
            lf_up.set_text(&s.local_forwards);
            rf_up.set_text(&s.remote_forwards);
            for i in 0..16 {
                if i < s.palette.len() && i < pal_buttons.len() {
                    pal_buttons[i].set_rgba(&hex_to_rgba(&s.palette[i]));
                }
            }
        }
    });
}

fn find_parent_window<W: glib::prelude::IsA<gtk::Widget>>(widget: &W) -> glib::object::WeakRef<gtk::ApplicationWindow> {
    let mut current = widget.parent();
    while let Some(parent) = current {
        if let Ok(win) = parent.clone().downcast::<gtk::ApplicationWindow>() {
            return win.downgrade();
        }
        current = parent.parent();
    }
    glib::object::WeakRef::new()
}

pub fn user_entry_downgrade(e: &Entry) -> glib::object::WeakRef<Entry> { e.downgrade() }
pub fn pass_entry_downgrade(e: &Entry) -> glib::object::WeakRef<Entry> { e.downgrade() }
pub fn fg_button_downgrade(b: &ColorButton) -> glib::object::WeakRef<ColorButton> { b.downgrade() }
pub fn bg_button_downgrade(b: &ColorButton) -> glib::object::WeakRef<ColorButton> { b.downgrade() }
pub fn font_dropdown_downgrade(d: &DropDown) -> glib::object::WeakRef<DropDown> { d.downgrade() }
pub fn cur_dropdown_downgrade(d: &DropDown) -> glib::object::WeakRef<DropDown> { d.downgrade() }
pub fn blink_check_downgrade(c: &CheckButton) -> glib::object::WeakRef<CheckButton> { c.downgrade() }
pub fn scroll_entry_downgrade(e: &Entry) -> glib::object::WeakRef<Entry> { e.downgrade() }

pub fn upgrade_user_e(w: &glib::object::WeakRef<Entry>) -> Option<Entry> { w.upgrade() }
pub fn upgrade_pass_e(w: &glib::object::WeakRef<Entry>) -> Option<Entry> { w.upgrade() }
pub fn upgrade_fg_b(w: &glib::object::WeakRef<ColorButton>) -> Option<ColorButton> { w.upgrade() }
pub fn upgrade_bg_b(w: &glib::object::WeakRef<ColorButton>) -> Option<ColorButton> { w.upgrade() }
pub fn upgrade_font_d(w: &glib::object::WeakRef<DropDown>) -> Option<DropDown> { w.upgrade() }
pub fn upgrade_cur_d(w: &glib::object::WeakRef<DropDown>) -> Option<DropDown> { w.upgrade() }
pub fn upgrade_blink_c(w: &glib::object::WeakRef<CheckButton>) -> Option<CheckButton> { w.upgrade() }
pub fn upgrade_scroll_e(w: &glib::object::WeakRef<Entry>) -> Option<Entry> { w.upgrade() }

pub fn rgba_to_hex(rgba: gtk::gdk::RGBA) -> String {
    format!("#{:02x}{:02x}{:02x}", 
        (rgba.red() * 255.0) as u8, 
        (rgba.green() * 255.0) as u8, 
        (rgba.blue() * 255.0) as u8)
}

pub fn hex_to_rgba(hex: &str) -> gtk::gdk::RGBA {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
    gtk::gdk::RGBA::builder().red(r).green(g).blue(b).alpha(1.0).build()
}

pub fn populate_known_hosts_list(list: &ListBox) {
    while let Some(child) = list.first_child() { list.remove(&child); }
    let hosts = crate::config::load_known_hosts();
    for (index, h) in hosts.iter().enumerate() {
        let row_box = GtkBox::new(Orientation::Horizontal, 10);
        row_box.set_margin_start(10); row_box.set_margin_end(10);
        row_box.set_margin_top(5); row_box.set_margin_bottom(5);
        
        let text_box = GtkBox::new(Orientation::Vertical, 2);
        text_box.set_hexpand(true);
        let host_label = Label::builder().label(&format!("{}:{}", h.host, h.port)).halign(gtk::Align::Start).css_classes(["title-4"]).build();
        let fp_label = Label::builder().label(&h.fingerprint).halign(gtk::Align::Start).css_classes(["caption", "dim-label"]).selectable(true).build();
        text_box.append(&host_label);
        text_box.append(&fp_label);
        row_box.append(&text_box);
        
        let del_btn = Button::builder().icon_name("user-trash-symbolic").css_classes(["flat", "error"]).build();
        let list_weak = list.downgrade();
        del_btn.connect_clicked(move |_| {
            let mut current = crate::config::load_known_hosts();
            if index < current.len() {
                current.remove(index);
                crate::config::save_known_hosts(&current);
                if let Some(l) = list_weak.upgrade() {
                    populate_known_hosts_list(&l);
                }
            }
        });
        row_box.append(&del_btn);
        
        let row = gtk::ListBoxRow::builder().child(&row_box).build();
        list.append(&row);
    }
}

