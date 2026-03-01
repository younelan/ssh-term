#!/bin/bash
# ─────────────────────────────────────────────
# Demo 3: Grid Layout — login form + settings
# with colorful reactions to interactions.
# ─────────────────────────────────────────────

widget() { printf '\033]1337;Widget=%s\007' "$1"; }
panel()  { printf '\033]1337;Panel=%s\007'  "$1"; }

RED='\033[31m'; GREEN='\033[32m'; YELLOW='\033[33m'; BLUE='\033[34m'
MAGENTA='\033[35m'; CYAN='\033[36m'; BOLD='\033[1m'; DIM='\033[2m'
RESET='\033[0m'; BG_GREEN='\033[42m'; BG_RED='\033[41m'

clear
stty -echo
trap 'stty sane' EXIT
trap 'exit 0' INT TERM

echo -e "${BOLD}${GREEN}┌──────────────────────────────────────┐${RESET}"
echo -e "${BOLD}${GREEN}│  Demo 3: Grid Form Layout            │${RESET}"
echo -e "${BOLD}${GREEN}└──────────────────────────────────────┘${RESET}"
echo ""

# Login form
echo -n "  "
panel "id:login;layout:grid;title:🔐 Login;spacing:8;margin:16;width:400"
widget "type:label;id:lbl_user;text:Username;panel:login;row:0;col:0"
widget "type:entry;id:in_user;placeholder:admin;width:20;panel:login;row:0;col:1"
widget "type:label;id:lbl_pass;text:Password;panel:login;row:1;col:0"
widget "type:password;id:in_pass;placeholder:secret;panel:login;row:1;col:1"
widget "type:label;id:lbl_role;text:Role;panel:login;row:2;col:0"
widget "type:dropdown;id:dd_role;items:User,Admin,Root;panel:login;row:2;col:1"
widget "type:checkbox;id:chk_remember;label:Remember me;panel:login;row:3;col:0;colspan:2"
widget "type:button;id:btn_login;label:🔐 Login;panel:login;row:4;col:0;colspan:2"
echo ""
echo ""

# Display settings
echo -n "  "
panel "id:config;layout:grid;title:🖥 Display Settings;spacing:8;margin:16;width:400"
widget "type:label;id:lbl_bright;text:Brightness;panel:config;row:0;col:0"
widget "type:slider;id:sl_bright;min:0;max:100;value:75;width:200;panel:config;row:0;col:1"
widget "type:label;id:lbl_vol;text:Volume;panel:config;row:1;col:0"
widget "type:slider;id:sl_vol;min:0;max:100;value:50;width:200;panel:config;row:1;col:1"
widget "type:label;id:lbl_font;text:Font Size;panel:config;row:2;col:0"
widget "type:spinbutton;id:sp_font;min:8;max:72;value:14;step:1;panel:config;row:2;col:1"
widget "type:label;id:lbl_dark;text:Dark Mode;panel:config;row:3;col:0"
widget "type:switch;id:sw_dark;active:true;panel:config;row:3;col:1"
echo ""
echo ""
echo -n "  "
widget "type:close;id:quit;label:✕ Quit"
echo ""
echo ""
echo -e "  ${DIM}─── Status ────────────────────────────${RESET}"
echo ""

user=""; role="User"
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
                in_user)
                    user="$value"
                    echo -e "  ${BLUE}👤 Username: ${BOLD}${value}${RESET}" ;;
                in_pass)
                    echo -e "  ${DIM}🔑 Password set (${#value} chars)${RESET}" ;;
                dd_role)
                    role="$value"
                    case "$value" in
                        Root)  echo -e "  ${RED}${BOLD}⚠ Role: ROOT — Full access${RESET}" ;;
                        Admin) echo -e "  ${YELLOW}🛡 Role: Admin — Elevated${RESET}" ;;
                        *)     echo -e "  ${GREEN}👤 Role: User — Standard${RESET}" ;;
                    esac ;;
                chk_remember)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${CYAN}🍪 Remember me: yes${RESET}" \
                        || echo -e "  ${DIM}🍪 Remember me: no${RESET}" ;;
                btn_login)
                    if [[ -n "$user" ]]; then
                        echo -e "  ${BG_GREEN}${BOLD} ✓ Welcome, ${user} (${role})! ${RESET}"
                    else
                        echo -e "  ${BG_RED}${BOLD} ✗ Please enter a username! ${RESET}"
                    fi ;;
                sl_bright)
                    level=$((value / 10))
                    bar=""; for ((i=0;i<level;i++)); do bar+="☀"; done
                    echo -e "  ${YELLOW}Brightness: ${bar} ${value}%${RESET}" ;;
                sl_vol)
                    level=$((value / 10))
                    bar=""; for ((i=0;i<level;i++)); do bar+="▮"; done
                    for ((i=level;i<10;i++)); do bar+="▯"; done
                    echo -e "  ${CYAN}Volume: ${bar} ${value}%${RESET}" ;;
                sp_font)
                    echo -e "  ${MAGENTA}🔤 Font size: ${BOLD}${value}pt${RESET}" ;;
                sw_dark)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${BOLD}${DIM}🌙 Dark mode enabled${RESET}" \
                        || echo -e "  ${YELLOW}☀ Light mode enabled${RESET}" ;;
            esac
        fi
    fi
done
