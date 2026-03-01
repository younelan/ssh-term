#!/bin/bash
# ─────────────────────────────────────────────
# Demo: Notebook / Tabs
# Multiple panels in a tabbed interface
# ─────────────────────────────────────────────

widget() { printf '\033]1337;Widget=%s\007' "$1"; }
panel()  { printf '\033]1337;Panel=%s\007'  "$1"; }
update() { printf '\033]1337;WidgetUpdate=%s\007' "$1"; }

clear
stty -echo
trap 'stty sane' EXIT
trap 'exit 0' INT TERM

echo ""

# ── Root layout: notebook on left, log on right ──
panel "id:root;layout:horizontal;spacing:12;margin:8"

# Create the notebook
widget "type:notebook;id:nb;width:400;height:350;panel:root"

# ── Tab 1: Profile ──
widget "type:tab;id:tab_profile;notebook:nb;label:👤 Profile"
widget "type:label;id:lp1;text:Name:;panel:tab_profile"
widget "type:entry;id:prof_name;placeholder:Your name;width:25;panel:tab_profile"
widget "type:label;id:lp2;text:Email:;panel:tab_profile"
widget "type:entry;id:prof_email;placeholder:you@example.com;width:25;panel:tab_profile"
widget "type:label;id:lp3;text:Role:;panel:tab_profile"
widget "type:dropdown;id:prof_role;items:Developer,Designer,Manager,QA;panel:tab_profile"

# ── Tab 2: Preferences ──
widget "type:tab;id:tab_prefs;notebook:nb;label:⚙ Settings"
widget "type:checkbox;id:pref_dark;label:Dark mode;panel:tab_prefs"
widget "type:checkbox;id:pref_notify;label:Notifications;panel:tab_prefs"
widget "type:label;id:ls1;text:Font size:;panel:tab_prefs"
widget "type:spinbutton;id:pref_font;min:8;max:32;value:14;step:1;panel:tab_prefs"
widget "type:label;id:ls2;text:Volume:;panel:tab_prefs"
widget "type:slider;id:pref_vol;min:0;max:100;value:50;width:250;panel:tab_prefs"
widget "type:label;id:ls3;text:Theme:;panel:tab_prefs"
widget "type:colorbutton;id:pref_color;value:#3584e4;title:Theme color;panel:tab_prefs"

# ── Tab 3: About ──
widget "type:tab;id:tab_about;notebook:nb;label:ℹ About"
widget "type:label;id:la1;text:Terminal Widget Demo v1.0;panel:tab_about"
widget "type:label;id:la2;text:Built with GTK4 + Rust;panel:tab_about"
widget "type:link;id:repo;uri:https://github.com;label:View on GitHub;panel:tab_about"
widget "type:separator;panel:tab_about"
widget "type:label;id:la3;text:Custom OSC 1337 protocol;panel:tab_about"

# ── Right side: event log + actions ──
panel "id:right;layout:vertical;title:📋 Activity;spacing:4;margin:8;expand:true;panel:root"
widget "type:textview;id:log;width:300;height:280;editable:false;wrap:word;panel:right"
update "id:log;text:Switch tabs and interact with controls."
widget "type:separator;panel:right"
panel "id:actions;layout:horizontal;spacing:8;panel:right"
widget "type:button;id:btn_save;label:💾 Save All;panel:actions"
widget "type:close;id:quit;label:✕ Quit;panel:actions"

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
            line=""

            case "$id" in
                prof_name)   line="[$ts] 👤 Name: $value" ;;
                prof_email)  line="[$ts] 📧 Email: $value" ;;
                prof_role)   line="[$ts] 🏷 Role: $value" ;;
                pref_dark)
                    [[ "$value" == "true" ]] \
                        && line="[$ts] 🌙 Dark mode ON" \
                        || line="[$ts] ☀ Dark mode OFF" ;;
                pref_notify)
                    [[ "$value" == "true" ]] \
                        && line="[$ts] 🔔 Notifications ON" \
                        || line="[$ts] 🔕 Notifications OFF" ;;
                pref_font)   line="[$ts] 🔤 Font: ${value}pt" ;;
                pref_vol)    line="[$ts] 🔊 Volume: ${value}%" ;;
                pref_color)  line="[$ts] 🎨 Theme: $value" ;;
                tab_*)       line="[$ts] 📑 Switched to tab $value" ;;
                btn_save)    line="[$ts] 💾 Settings saved!" ;;
                quit)        ;; # close button handles exit
                *)           line="[$ts] $id: $value" ;;
            esac

            if [[ -n "$line" ]]; then
                update "id:log;append:$line"
            fi
        fi
    fi
done
