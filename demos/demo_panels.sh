#!/bin/bash
# ─────────────────────────────────────────────
# Demo 2: Panel Layouts — vertical settings
# panel + horizontal toolbar, with live color
# feedback on every interaction.
# ─────────────────────────────────────────────

widget() { printf '\033]1337;Widget=%s\007' "$1"; }
panel()  { printf '\033]1337;Panel=%s\007'  "$1"; }

RED='\033[31m'; GREEN='\033[32m'; YELLOW='\033[33m'; BLUE='\033[34m'
MAGENTA='\033[35m'; CYAN='\033[36m'; BOLD='\033[1m'; DIM='\033[2m'; RESET='\033[0m'

clear
stty -echo
trap 'stty sane' EXIT
trap 'exit 0' INT TERM

echo -e "${BOLD}${BLUE}┌──────────────────────────────────────┐${RESET}"
echo -e "${BOLD}${BLUE}│  Demo 2: Panel Layouts               │${RESET}"
echo -e "${BOLD}${BLUE}└──────────────────────────────────────┘${RESET}"
echo ""

# Vertical settings panel
echo -n "  "
panel "id:settings;layout:vertical;title:⚙ Settings;spacing:10;margin:12;width:320"
widget "type:label;id:lbl_name;text:Your Name;panel:settings"
widget "type:entry;id:input_name;placeholder:Enter name...;width:25;panel:settings"
widget "type:separator;id:sep1;panel:settings"
widget "type:label;id:lbl_theme;text:Theme;panel:settings"
widget "type:dropdown;id:dd_theme;items:Light,Dark,Solarized,Nord,Dracula;panel:settings"
widget "type:separator;id:sep2;panel:settings"
widget "type:checkbox;id:chk_auto;label:Auto-save;checked:true;panel:settings"
widget "type:checkbox;id:chk_notify;label:Notifications;panel:settings"
widget "type:separator;id:sep3;panel:settings"
widget "type:button;id:btn_save;label:💾 Save Settings;panel:settings"
echo ""
echo ""

# Horizontal toolbar
echo -n "  "
panel "id:toolbar;layout:horizontal;title:🔧 Toolbar;spacing:8;margin:8"
widget "type:button;id:btn_new;label:📄 New;panel:toolbar"
widget "type:button;id:btn_open;label:📂 Open;panel:toolbar"
widget "type:button;id:btn_save2;label:💾 Save;panel:toolbar"
widget "type:separator;id:sep4;orient:vertical;panel:toolbar"
widget "type:button;id:btn_undo;label:↩ Undo;panel:toolbar"
widget "type:button;id:btn_redo;label:↪ Redo;panel:toolbar"
widget "type:close;id:quit;label:✕ Quit;panel:toolbar"
echo ""
echo ""

echo -e "  ${DIM}─── Activity Log ──────────────────────${RESET}"
echo ""

theme_colors=("${RESET}" "${BOLD}${DIM}" "${YELLOW}" "${CYAN}" "${MAGENTA}")
theme_names=("Light" "Dark" "Solarized" "Nord" "Dracula")

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

            case "$id" in
                input_name)
                    echo -e "  ${BLUE}👤 Hello, ${BOLD}${value}${RESET}${BLUE}!${RESET}" ;;
                dd_theme)
                    for i in "${!theme_names[@]}"; do
                        if [[ "${theme_names[$i]}" == "$value" ]]; then
                            echo -e "  ${theme_colors[$i]}🎨 Theme → ${BOLD}${value}${RESET}"
                            break
                        fi
                    done ;;
                chk_auto)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${GREEN}✓ Auto-save enabled${RESET}" \
                        || echo -e "  ${YELLOW}✗ Auto-save disabled${RESET}" ;;
                chk_notify)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${GREEN}🔔 Notifications on${RESET}" \
                        || echo -e "  ${RED}🔕 Notifications off${RESET}" ;;
                btn_save)
                    echo -e "  ${GREEN}${BOLD}💾 Settings saved!${RESET}" ;;
                btn_new)
                    echo -e "  ${CYAN}📄 New document created${RESET}" ;;
                btn_open)
                    echo -e "  ${BLUE}📂 Opening file browser...${RESET}" ;;
                btn_save2)
                    echo -e "  ${GREEN}💾 File saved${RESET}" ;;
                btn_undo)
                    echo -e "  ${YELLOW}↩ Undo${RESET}" ;;
                btn_redo)
                    echo -e "  ${YELLOW}↪ Redo${RESET}" ;;
            esac
        fi
    fi
done
