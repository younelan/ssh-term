#!/bin/bash
# ─────────────────────────────────────────────
# Demo: Menu Bar & Toolbar
# Full app-like UI with menus, toggleable
# menu items, icon toolbar, and event log.
# ─────────────────────────────────────────────

widget() { printf '\033]1337;Widget=%s\007' "$1"; }
panel()  { printf '\033]1337;Panel=%s\007'  "$1"; }
update() { printf '\033]1337;WidgetUpdate=%s\007' "$1"; }

RED='\033[31m'; GREEN='\033[32m'; YELLOW='\033[33m'; BLUE='\033[34m'
MAGENTA='\033[35m'; CYAN='\033[36m'; BOLD='\033[1m'; DIM='\033[2m'; RESET='\033[0m'

clear
stty -echo
trap 'stty sane' EXIT
trap 'exit 0' INT TERM

# ── Root layout ──
panel "id:root;layout:vertical;spacing:4;margin:4;width:620"

# ── Menu Bar ──
widget "type:menubar;id:mbar;panel:root"

# File menu
widget "type:menu;id:file;label:File;menubar:mbar"
widget "type:menuitem;id:mi_new;label:New;menu:file"
widget "type:menuitem;id:mi_open;label:Open;menu:file"
widget "type:menu;id:file_recent;label:Recent Files;menu:file"
widget "type:menuitem;id:mi_rec1;label:document.txt;menu:file_recent"
widget "type:menuitem;id:mi_rec2;label:notes.md;menu:file_recent"
widget "type:menuitem;id:mi_rec3;label:config.yaml;menu:file_recent"
widget "type:menusep;menu:file"
widget "type:menuitem;id:mi_save;label:Save;menu:file"
widget "type:menuitem;id:mi_saveas;label:Save As…;menu:file"
widget "type:menusep;menu:file"
widget "type:menuitem;id:mi_quit;label:Quit;menu:file"

# Edit menu
widget "type:menu;id:edit;label:Edit;menubar:mbar"
widget "type:menuitem;id:mi_undo;label:Undo;menu:edit"
widget "type:menuitem;id:mi_redo;label:Redo;menu:edit"
widget "type:menusep;menu:edit"
widget "type:menuitem;id:mi_cut;label:Cut;menu:edit"
widget "type:menuitem;id:mi_copy;label:Copy;menu:edit"
widget "type:menuitem;id:mi_paste;label:Paste;menu:edit"
widget "type:menusep;menu:edit"
widget "type:menuitem;id:mi_find;label:Find…;menu:edit"

# View menu with check items
widget "type:menu;id:view;label:View;menubar:mbar"
widget "type:menucheck;id:mi_sidebar;label:Sidebar;menu:view;checked:true"
widget "type:menucheck;id:mi_statusbar;label:Status Bar;menu:view;checked:true"
widget "type:menucheck;id:mi_wordwrap;label:Word Wrap;menu:view"
widget "type:menusep;menu:view"
widget "type:menuitem;id:mi_zoomin;label:Zoom In;menu:view"
widget "type:menuitem;id:mi_zoomout;label:Zoom Out;menu:view"
widget "type:menuitem;id:mi_zoomreset;label:Reset Zoom;menu:view"

# Help menu
widget "type:menu;id:help;label:Help;menubar:mbar"
widget "type:menuitem;id:mi_docs;label:Documentation;menu:help"
widget "type:menuitem;id:mi_about;label:About;menu:help"

# ── Toolbar ──
widget "type:toolbar;id:tbar;panel:root"
widget "type:toolbutton;id:tb_new;icon:document-new;tooltip:New;toolbar:tbar"
widget "type:toolbutton;id:tb_open;icon:document-open;tooltip:Open;toolbar:tbar"
widget "type:toolbutton;id:tb_save;icon:document-save;tooltip:Save;toolbar:tbar"
widget "type:separator;id:tsep1;orient:vertical;panel:tbar"
widget "type:toolbutton;id:tb_undo;icon:edit-undo;tooltip:Undo;toolbar:tbar"
widget "type:toolbutton;id:tb_redo;icon:edit-redo;tooltip:Redo;toolbar:tbar"
widget "type:separator;id:tsep2;orient:vertical;panel:tbar"
widget "type:toolbutton;id:tb_cut;icon:edit-cut;tooltip:Cut;toolbar:tbar"
widget "type:toolbutton;id:tb_copy;icon:edit-copy;tooltip:Copy;toolbar:tbar"
widget "type:toolbutton;id:tb_paste;icon:edit-paste;tooltip:Paste;toolbar:tbar"
widget "type:separator;id:tsep3;orient:vertical;panel:tbar"
widget "type:tooltoggle;id:tb_bold;icon:format-text-bold;tooltip:Bold;toolbar:tbar"
widget "type:tooltoggle;id:tb_italic;icon:format-text-italic;tooltip:Italic;toolbar:tbar"
widget "type:tooltoggle;id:tb_underline;icon:format-text-underline;tooltip:Underline;toolbar:tbar"
widget "type:separator;id:tsep4;orient:vertical;panel:tbar"
widget "type:close;id:quit;label:✕ Quit;panel:tbar"

# ── Event Log ──
widget "type:textview;id:log;width:600;height:300;editable:false;wrap:word;panel:root"
widget "type:label;id:status;text:Ready — click menus or toolbar buttons;panel:root"

update "id:log;text:=== Menu & Toolbar Demo ===
Click menu items or toolbar buttons to see events here.
"

# ── Event loop ──
while IFS= read -r -n 1 ch; do
    if [[ "$ch" == $'\033' ]]; then
        seq=""
        while IFS= read -r -n 1 c; do
            [[ "$c" == $'\007' ]] && break
            seq+="$c"
        done
        if [[ "$seq" == *"WidgetEvent"* ]]; then
            evt="${seq#*WidgetEvent=}"
            id=""; action=""; value=""
            IFS=';' read -ra parts <<< "$evt"
            for p in "${parts[@]}"; do
                case "$p" in
                    id:*)     id="${p#id:}" ;;
                    action:*) action="${p#action:}" ;;
                    value:*)  value="${p#value:}" ;;
                esac
            done

            ts=$(date +%H:%M:%S)

            case "$id" in
                mi_new|tb_new)      update "id:log;append:[$ts] 📄 New document created" ;;
                mi_open|tb_open)    update "id:log;append:[$ts] 📂 Open file…" ;;
                mi_rec1)            update "id:log;append:[$ts] 📂 Opened document.txt" ;;
                mi_rec2)            update "id:log;append:[$ts] 📂 Opened notes.md" ;;
                mi_rec3)            update "id:log;append:[$ts] 📂 Opened config.yaml" ;;
                mi_save|tb_save)    update "id:log;append:[$ts] 💾 Saved!" ;;
                mi_saveas)          update "id:log;append:[$ts] 💾 Save As…" ;;
                mi_quit)            exit 0 ;;
                mi_undo|tb_undo)    update "id:log;append:[$ts] ↩ Undo" ;;
                mi_redo|tb_redo)    update "id:log;append:[$ts] ↪ Redo" ;;
                mi_cut|tb_cut)      update "id:log;append:[$ts] ✂ Cut" ;;
                mi_copy|tb_copy)    update "id:log;append:[$ts] 📋 Copy" ;;
                mi_paste|tb_paste)  update "id:log;append:[$ts] 📌 Paste" ;;
                mi_find)            update "id:log;append:[$ts] 🔍 Find…" ;;
                mi_sidebar)
                    if [[ "$value" == "true" ]]; then
                        update "id:log;append:[$ts] ☑ Sidebar shown"
                    else
                        update "id:log;append:[$ts] ☐ Sidebar hidden"
                    fi ;;
                mi_statusbar)
                    if [[ "$value" == "true" ]]; then
                        update "id:log;append:[$ts] ☑ Status bar shown"
                    else
                        update "id:log;append:[$ts] ☐ Status bar hidden"
                    fi ;;
                mi_wordwrap)
                    if [[ "$value" == "true" ]]; then
                        update "id:log;append:[$ts] ☑ Word wrap on"
                    else
                        update "id:log;append:[$ts] ☐ Word wrap off"
                    fi ;;
                mi_zoomin)          update "id:log;append:[$ts] 🔍+ Zoom in" ;;
                mi_zoomout)         update "id:log;append:[$ts] 🔍- Zoom out" ;;
                mi_zoomreset)       update "id:log;append:[$ts] 🔍 Reset zoom" ;;
                mi_docs)            update "id:log;append:[$ts] 📖 Opening documentation…" ;;
                mi_about)           update "id:log;append:[$ts] ℹ️  Terminal Widget Demo v1.0" ;;
                tb_bold)
                    if [[ "$value" == "true" ]]; then
                        update "id:log;append:[$ts] 𝐁 Bold ON"
                    else
                        update "id:log;append:[$ts] B Bold OFF"
                    fi ;;
                tb_italic)
                    if [[ "$value" == "true" ]]; then
                        update "id:log;append:[$ts] 𝐼 Italic ON"
                    else
                        update "id:log;append:[$ts] I Italic OFF"
                    fi ;;
                tb_underline)
                    if [[ "$value" == "true" ]]; then
                        update "id:log;append:[$ts] U̲ Underline ON"
                    else
                        update "id:log;append:[$ts] U Underline OFF"
                    fi ;;
                *)
                    update "id:log;append:[$ts] ${id}: ${action} = ${value}" ;;
            esac

            update "id:status;text:Last event: ${id} (${action})"
        fi
    fi
done
