#!/bin/bash
# ─────────────────────────────────────────────
# Demo 8: Two-Column GUI Layout
# Controls on the left, live event log on right
# Demonstrates nested panels + WidgetUpdate
# ─────────────────────────────────────────────

widget() { printf '\033]1337;Widget=%s\007' "$1"; }
panel()  { printf '\033]1337;Panel=%s\007'  "$1"; }
update() { printf '\033]1337;WidgetUpdate=%s\007' "$1"; }

clear
stty -echo
trap 'stty sane' EXIT
trap 'exit 0' INT TERM

echo ""

# ── Root: horizontal layout with two columns ──
panel "id:root;layout:horizontal;spacing:12;margin:8"

# ── Left column: controls ──
panel "id:left;layout:vertical;title:🎛 Controls;spacing:8;margin:10;width:350;panel:root"

# Name entry
widget "type:label;id:l_name;text:Your Name:;panel:left"
widget "type:entry;id:name;placeholder:Enter your name;width:28;panel:left"

# Greeting style radios
widget "type:label;id:l_style;text:Greeting Style:;panel:left"
panel "id:radios;layout:horizontal;spacing:12;panel:left"
widget "type:radio;id:r_formal;label:Formal;group:style;panel:radios"
widget "type:radio;id:r_casual;label:Casual;group:style;panel:radios"
widget "type:radio;id:r_pirate;label:Pirate;group:style;panel:radios"

# Theme color
widget "type:label;id:l_color;text:Theme Color:;panel:left"
panel "id:color_row;layout:horizontal;spacing:8;panel:left"
widget "type:colorbutton;id:theme_color;value:#3584e4;title:Pick a theme color;panel:color_row"
widget "type:label;id:color_label;text:#3584e4;panel:color_row"

# Volume slider
widget "type:label;id:l_vol;text:Volume:;panel:left"
widget "type:slider;id:volume;min:0;max:100;value:50;width:280;panel:left"

# Options
panel "id:opts;layout:horizontal;spacing:12;panel:left"
widget "type:checkbox;id:chk_notify;label:Enable notifications;panel:opts"
widget "type:switch;id:sw_sound;active:true;panel:opts"

# Action buttons
widget "type:separator;panel:left"
panel "id:buttons;layout:horizontal;spacing:8;panel:left"
widget "type:button;id:btn_greet;label:👋 Say Hello;panel:buttons"
widget "type:button;id:btn_clear;label:🗑 Clear Log;panel:buttons"
widget "type:button;id:btn_count;label:📊 Show Stats;panel:buttons"
widget "type:close;id:quit;label:✕ Quit;panel:buttons"

# ── Right column: event log (textview) ──
panel "id:right;layout:vertical;title:📋 Event Log;spacing:4;margin:10;width:400;expand:true;panel:root"
widget "type:textview;id:log;width:380;height:400;editable:false;wrap:word;panel:right"
widget "type:label;id:status;text:Ready. Interact with controls on the left.;panel:right"

# Prime the log
update "id:log;text:=== Event Log ===
Waiting for events..."

# ── Event loop ──
greeting_style="Formal"
username=""
evt_count=0
click_count=0
notify_on="false"
sound_on="true"

while IFS= read -r -n 1 ch; do
    if [[ "$ch" == $'\033' ]]; then
        seq=""
        while IFS= read -r -n 1 c; do
            [[ "$c" == $'\007' ]] && break
            seq+="$c"
        done
        if [[ "$seq" == *"WidgetEvent"* ]]; then
            evt="${seq#*WidgetEvent=}"
            id=""; action=""; value=""; group=""
            IFS=';' read -ra parts <<< "$evt"
            for p in "${parts[@]}"; do
                case "$p" in
                    id:*)     id="${p#id:}" ;;
                    action:*) action="${p#action:}" ;;
                    value:*)  value="${p#value:}" ;;
                    group:*)  group="${p#group:}" ;;
                esac
            done

            evt_count=$((evt_count + 1))
            ts=$(date +%H:%M:%S)
            log_line=""

            case "$id" in
                name)
                    username="$value"
                    log_line="[$ts] 📝 Name set: \"$value\"" ;;
                r_formal)
                    greeting_style="Formal"
                    log_line="[$ts] 🎩 Style: Formal" ;;
                r_casual)
                    greeting_style="Casual"
                    log_line="[$ts] 😎 Style: Casual" ;;
                r_pirate)
                    greeting_style="Pirate"
                    log_line="[$ts] 🏴‍☠️ Style: Pirate" ;;
                theme_color)
                    log_line="[$ts] 🎨 Color: $value"
                    update "id:color_label;text:$value" ;;
                volume)
                    log_line="[$ts] 🔊 Volume: ${value}%" ;;
                chk_notify)
                    notify_on="$value"
                    if [[ "$value" == "true" ]]; then
                        log_line="[$ts] 🔔 Notifications: ON"
                    else
                        log_line="[$ts] 🔕 Notifications: OFF"
                    fi ;;
                sw_sound)
                    sound_on="$value"
                    if [[ "$value" == "true" ]]; then
                        log_line="[$ts] 🔈 Sound: ON"
                    else
                        log_line="[$ts] 🔇 Sound: MUTED"
                    fi ;;
                btn_greet)
                    click_count=$((click_count + 1))
                    name="${username:-World}"
                    case "$greeting_style" in
                        Formal) msg="Good day, $name. How do you do?" ;;
                        Casual) msg="Hey $name! What's up?" ;;
                        Pirate) msg="Ahoy, $name! Ye scallywag!" ;;
                        *)      msg="Hello, $name!" ;;
                    esac
                    log_line="[$ts] 👋 $msg"
                    update "id:btn_greet;label:👋 Greeted ${click_count}x" ;;
                btn_clear)
                    update "id:log;text:=== Log cleared at $ts ===
"
                    log_line=""
                    update "id:status;text:Log cleared." ;;
                btn_count)
                    stats="Events: $evt_count | Greets: $click_count | Sound: $sound_on | Notify: $notify_on"
                    log_line="[$ts] 📊 $stats" ;;
                *)
                    log_line="[$ts] ${id}: ${action}=${value}" ;;
            esac

            if [[ -n "$log_line" ]]; then
                update "id:log;append:$log_line"
                update "id:status;text:Event #${evt_count} from ${id} | ${action}"
            fi
        fi
    fi
done
