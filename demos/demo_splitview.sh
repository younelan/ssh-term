#!/bin/bash
# ─────────────────────────────────────────────────────────────────
#  Split View Demo  —  Toolbar + Menubar + Tree/List split pane
# ─────────────────────────────────────────────────────────────────

W()  { printf "\033]1337;Widget=%s\007" "$1"; }
P()  { printf "\033]1337;Panel=%s\007" "$1"; }
WU() { printf "\033]1337;WidgetUpdate=%s\007" "$1"; }

clear
stty -echo
trap 'stty sane; exit 0' INT TERM EXIT

# ── Root vertical panel — explicit width makes all children the same width ──
P "id:root;layout:vertical;spacing:0;margin:0;width:720"

# ── Title bar ────────────────────────────────────────────────────
W "type:titlebar;id:title;title:File Explorer;icon:system-file-manager-symbolic;panel:root"

# ── Menu bar ─────────────────────────────────────────────────────
W "type:menubar;id:mbar;panel:root"

W "type:menu;id:m_file;label:File;menubar:mbar"
W "type:menuitem;id:mi_new;label:New;menu:m_file"
W "type:menuitem;id:mi_open;label:Open;menu:m_file"
W "type:menusep;id:sep1;menu:m_file"
W "type:menuitem;id:mi_quit;label:Quit;menu:m_file"

W "type:menu;id:m_edit;label:Edit;menubar:mbar"
W "type:menuitem;id:mi_copy;label:Copy;menu:m_edit"
W "type:menuitem;id:mi_paste;label:Paste;menu:m_edit"

W "type:menu;id:m_view;label:View;menubar:mbar"
W "type:menucheck;id:mv_details;label:Show Details;menu:m_view;checked:true"
W "type:menucheck;id:mv_hidden;label:Show Hidden Files;menu:m_view"

# ── Toolbar ──────────────────────────────────────────────────────
W "type:toolbar;id:tb;panel:root"
W "type:toolbutton;id:tb_new;label:New;icon:document-new-symbolic;toolbar:tb"
W "type:toolbutton;id:tb_open;label:Open;icon:document-open-symbolic;toolbar:tb"
W "type:toolbutton;id:tb_refresh;label:Refresh;icon:view-refresh-symbolic;toolbar:tb"
W "type:toolbutton;id:tb_delete;label:Delete;icon:edit-delete-symbolic;toolbar:tb"

# ── Split pane (tree left, list right) ───────────────────────────
W "type:splitview;id:split;layout:horizontal;pos:220;height:420;panel:root"

W "type:treeview;id:tree;panel:split-start"
W "type:listview;id:list;cols:Name|Size|Type|Modified;panel:split-end"

# ── Status bar ───────────────────────────────────────────────────
W "type:statusbar;id:status;text:4 items — Documents;panel:root"

# ── Populate tree ─────────────────────────────────────────────────
WU "id:tree;action:addrow;rowid:docs;label:Documents"
WU "id:tree;action:addrow;rowid:reports;label:Reports;parent:docs"
WU "id:tree;action:addrow;rowid:notes;label:Notes;parent:docs"

WU "id:tree;action:addrow;rowid:downloads;label:Downloads"
WU "id:tree;action:addrow;rowid:apps;label:Applications;parent:downloads"

WU "id:tree;action:addrow;rowid:music;label:Music"
WU "id:tree;action:addrow;rowid:jazz;label:Jazz;parent:music"
WU "id:tree;action:addrow;rowid:rock;label:Rock;parent:music"

WU "id:tree;action:addrow;rowid:pictures;label:Pictures"
WU "id:tree;action:expand_all"

# ── Pre-populate list with Documents files ────────────────────────
populate_documents() {
    WU "id:list;action:clear"
    WU "id:list;action:addrow;cols:README.txt|12 KB|Text|Today"
    WU "id:list;action:addrow;cols:Notes.docx|48 KB|Word|Yesterday"
    WU "id:list;action:addrow;cols:Project Plan.pdf|320 KB|PDF|2026-02-28"
    WU "id:list;action:addrow;cols:Budget.xlsx|55 KB|Spreadsheet|2026-02-20"
    WU "id:status;text:4 items — Documents"
}

populate_reports() {
    WU "id:list;action:clear"
    WU "id:list;action:addrow;cols:Q1-Report.pdf|256 KB|PDF|2026-02-01"
    WU "id:list;action:addrow;cols:Q2-Report.pdf|180 KB|PDF|2026-03-01"
    WU "id:list;action:addrow;cols:Annual-2025.pdf|1.2 MB|PDF|2025-12-31"
    WU "id:status;text:3 items — Reports"
}

populate_notes() {
    WU "id:list;action:clear"
    WU "id:list;action:addrow;cols:meeting-2026-02.md|4 KB|Markdown|2026-02-14"
    WU "id:list;action:addrow;cols:todo.txt|1 KB|Text|Today"
    WU "id:list;action:addrow;cols:ideas.txt|8 KB|Text|2026-01-10"
    WU "id:status;text:3 items — Notes"
}

populate_downloads() {
    WU "id:list;action:clear"
    WU "id:list;action:addrow;cols:installer.dmg|150 MB|DMG|Today"
    WU "id:list;action:addrow;cols:archive.zip|45 MB|ZIP|Yesterday"
    WU "id:list;action:addrow;cols:update.pkg|88 MB|Package|2026-02-27"
    WU "id:status;text:3 items — Downloads"
}

populate_apps() {
    WU "id:list;action:clear"
    WU "id:list;action:addrow;cols:Firefox.dmg|212 MB|DMG|2026-01-15"
    WU "id:list;action:addrow;cols:VSCode.zip|94 MB|ZIP|2026-02-03"
    WU "id:status;text:2 items — Applications"
}

populate_music() {
    WU "id:list;action:clear"
    WU "id:list;action:addrow;cols:playlist.m3u|2 KB|M3U|2026-01-15"
    WU "id:list;action:addrow;cols:001 - Intro.mp3|4.2 MB|MP3|2025-12-01"
    WU "id:status;text:2 items — Music"
}

populate_jazz() {
    WU "id:list;action:clear"
    WU "id:list;action:addrow;cols:Kind of Blue.flac|220 MB|FLAC|2025-11-01"
    WU "id:list;action:addrow;cols:So What.mp3|9.1 MB|MP3|2025-11-01"
    WU "id:list;action:addrow;cols:Blue in Green.mp3|6.4 MB|MP3|2025-11-01"
    WU "id:status;text:3 items — Jazz"
}

populate_rock() {
    WU "id:list;action:clear"
    WU "id:list;action:addrow;cols:Stairway to Heaven.mp3|8.0 MB|MP3|2025-10-01"
    WU "id:list;action:addrow;cols:Hotel California.mp3|6.5 MB|MP3|2025-10-01"
    WU "id:status;text:2 items — Rock"
}

populate_pictures() {
    WU "id:list;action:clear"
    WU "id:list;action:addrow;cols:vacation-2025.jpg|3.1 MB|JPEG|2025-08-15"
    WU "id:list;action:addrow;cols:profile.png|512 KB|PNG|2026-01-02"
    WU "id:list;action:addrow;cols:screenshot.png|256 KB|PNG|Today"
    WU "id:status;text:3 items — Pictures"
}

# Start with Documents selected
populate_documents

# ── Event loop ────────────────────────────────────────────────────
while IFS= read -r -n1 ch; do
    if [[ "$ch" == $'\033' ]]; then
        seq=""
        while IFS= read -r -n1 c; do
            [[ "$c" == $'\007' ]] && break
            seq+="$c"
        done
        if [[ "$seq" == *"WidgetEvent"* ]]; then
            evt="${seq#*WidgetEvent=}"
            ev_id=""; ev_action=""; ev_value=""
            IFS=';' read -ra parts <<< "$evt"
            for p in "${parts[@]}"; do
                case "$p" in
                    id:*)     ev_id="${p#id:}" ;;
                    action:*) ev_action="${p#action:}" ;;
                    value:*)  ev_value="${p#value:}" ;;
                esac
            done

            # Tree selection → populate list
            if [[ "$ev_id" == "tree" && "$ev_action" == "selected" ]]; then
                case "$ev_value" in
                    "Documents")  populate_documents  ;;
                    "Reports")    populate_reports    ;;
                    "Notes")      populate_notes      ;;
                    "Downloads")  populate_downloads  ;;
                    "Applications") populate_apps     ;;
                    "Music")      populate_music      ;;
                    "Jazz")       populate_jazz       ;;
                    "Rock")       populate_rock       ;;
                    "Pictures")   populate_pictures   ;;
                esac
            fi

            # Toolbar / menu actions
            case "$ev_id" in
                tb_refresh|mi_reload)
                    WU "id:list;action:clear"
                    WU "id:list;action:addrow;cols:(refreshed)|—|—|Now"
                    ;;
                tb_delete) WU "id:list;action:clear" ;;
                mi_quit|tb_close)
                    stty sane; exit 0 ;;
            esac
        fi
    fi
done
