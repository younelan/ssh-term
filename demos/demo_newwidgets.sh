#!/bin/bash
# ─────────────────────────────────────────────
# Demo 6: New Widget Types — password, calendar,
# color picker, radio groups, toggle, level bar,
# link, expander
# ─────────────────────────────────────────────

widget() { printf '\033]1337;Widget=%s\007' "$1"; }
panel()  { printf '\033]1337;Panel=%s\007'  "$1"; }

RED='\033[31m'; GREEN='\033[32m'; YELLOW='\033[33m'; BLUE='\033[34m'
MAGENTA='\033[35m'; CYAN='\033[36m'; BOLD='\033[1m'; DIM='\033[2m'
RESET='\033[0m'; BG_GREEN='\033[42m'; BG_RED='\033[41m'; BG_BLUE='\033[44m'

clear
stty -echo
trap 'stty sane' EXIT
trap 'exit 0' INT TERM

echo -e "${BOLD}${MAGENTA}┌──────────────────────────────────────┐${RESET}"
echo -e "${BOLD}${MAGENTA}│  Demo 6: New Widget Types            │${RESET}"
echo -e "${BOLD}${MAGENTA}└──────────────────────────────────────┘${RESET}"
echo ""

# ── Section 1: Password ──
echo -e "  ${BOLD}${CYAN}Password Entry${RESET} ${DIM}(peek icon, submit on Enter)${RESET}"
echo -n "  "
widget "type:password;id:pw1;placeholder:Enter your password"
echo ""
echo ""

# ── Section 2: Radio Groups ──
echo -e "  ${BOLD}${YELLOW}Radio Groups${RESET} ${DIM}(mutually exclusive within group)${RESET}"
echo -n "  Plan: "
widget "type:radio;id:plan_free;label:Free;group:plan"
echo -n " "
widget "type:radio;id:plan_pro;label:Pro;group:plan"
echo -n " "
widget "type:radio;id:plan_team;label:Team;group:plan"
echo ""
echo -n "  Ship: "
widget "type:radio;id:ship_std;label:Standard;group:shipping"
echo -n " "
widget "type:radio;id:ship_exp;label:Express;group:shipping"
echo -n " "
widget "type:radio;id:ship_over;label:Overnight;group:shipping"
echo ""
echo ""

# ── Section 3: Toggle Buttons ──
echo -e "  ${BOLD}${GREEN}Toggle Buttons${RESET}"
echo -n "  "
widget "type:togglebutton;id:tgl_bold;label:Bold"
echo -n " "
widget "type:togglebutton;id:tgl_italic;label:Italic"
echo -n " "
widget "type:togglebutton;id:tgl_under;label:Underline"
echo ""
echo ""

# ── Section 4: Color Picker ──
echo -e "  ${BOLD}${RED}Color Picker${RESET}"
echo -n "  Foreground: "
widget "type:colorbutton;id:clr_fg;value:#ffffff;title:Choose foreground"
echo -n "  Background: "
widget "type:colorbutton;id:clr_bg;value:#1e1e2e;title:Choose background"
echo ""
echo ""

# ── Section 5: Calendar ──
echo -e "  ${BOLD}${BLUE}Calendar / Date Picker${RESET}"
echo -n "  "
widget "type:calendar;id:cal1"
echo ""
echo ""

# ── Section 6: Level Bar ──
echo -e "  ${BOLD}${CYAN}Level Bars${RESET}"
echo -n "  Health:   "
widget "type:levelbar;id:lvl_hp;min:0;max:100;value:80;width:200"
echo ""
echo -n "  Mana:     "
widget "type:levelbar;id:lvl_mp;min:0;max:100;value:45;width:200"
echo ""
echo ""

# ── Section 7: Link Button ──
echo -e "  ${BOLD}${MAGENTA}Links${RESET}"
echo -n "  "
widget "type:link;id:lnk1;uri:https://gtk-rs.org;label:GTK-rs Docs"
echo -n "    "
widget "type:link;id:lnk2;uri:https://docs.rs/vte;label:VTE Parser"
echo ""
echo ""

# ── Section 8: Expander (container) ──
echo -e "  ${BOLD}${YELLOW}Expander${RESET} ${DIM}(collapsible container with widgets inside)${RESET}"
echo -n "  "
widget "type:expander;id:exp1;label:Advanced Options;expanded:false"
widget "type:checkbox;id:exp_debug;label:Debug logging;panel:exp1"
widget "type:checkbox;id:exp_verbose;label:Verbose output;panel:exp1"
widget "type:slider;id:exp_threads;min:1;max:16;value:4;width:180;panel:exp1"
echo ""
echo ""
echo -n "  "
widget "type:close;id:quit;label:✕ Quit"
echo ""
echo ""
echo -e "  ${BOLD}${DIM}─── Events ────────────────────────────${RESET}"

count=0
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

            count=$((count + 1))

            case "$id" in
                # Password
                pw1)
                    stars=$(printf '%*s' "${#value}" '' | tr ' ' '*')
                    echo -e "  ${RED}🔐 Password submitted (${#value} chars): ${DIM}${stars}${RESET}" ;;

                # Radio groups
                plan_free)  echo -e "  ${GREEN}📦 Plan: ${BOLD}Free${RESET} ${DIM}— \$0/mo${RESET}" ;;
                plan_pro)   echo -e "  ${YELLOW}📦 Plan: ${BOLD}Pro${RESET} ${DIM}— \$12/mo${RESET}" ;;
                plan_team)  echo -e "  ${CYAN}📦 Plan: ${BOLD}Team${RESET} ${DIM}— \$25/mo${RESET}" ;;
                ship_std)   echo -e "  ${GREEN}🚚 Shipping: ${BOLD}Standard${RESET} ${DIM}(5-7 days)${RESET}" ;;
                ship_exp)   echo -e "  ${YELLOW}🚚 Shipping: ${BOLD}Express${RESET} ${DIM}(2-3 days)${RESET}" ;;
                ship_over)  echo -e "  ${RED}🚚 Shipping: ${BOLD}Overnight${RESET} ${DIM}(next day!)${RESET}" ;;

                # Toggle buttons — show active formatting
                tgl_bold)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${BOLD}B${RESET} Bold ${GREEN}ON${RESET}" \
                        || echo -e "  B Bold ${RED}OFF${RESET}" ;;
                tgl_italic)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${DIM}I${RESET} Italic ${GREEN}ON${RESET}" \
                        || echo -e "  I Italic ${RED}OFF${RESET}" ;;
                tgl_under)
                    [[ "$value" == "true" ]] \
                        && echo -e "  U̲ Underline ${GREEN}ON${RESET}" \
                        || echo -e "  U Underline ${RED}OFF${RESET}" ;;

                # Color picker
                clr_fg)
                    echo -e "  ${BOLD}🎨 Foreground: ${value}${RESET}" ;;
                clr_bg)
                    echo -e "  ${BOLD}🖌 Background: ${value}${RESET}" ;;

                # Calendar
                cal1)
                    echo -e "  ${BLUE}📅 Date selected: ${BOLD}${value}${RESET}" ;;

                # Expander children
                exp_debug)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${YELLOW}🐛 Debug logging: enabled${RESET}" \
                        || echo -e "  ${DIM}🐛 Debug logging: disabled${RESET}" ;;
                exp_verbose)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${CYAN}📢 Verbose output: enabled${RESET}" \
                        || echo -e "  ${DIM}📢 Verbose output: disabled${RESET}" ;;
                exp_threads)
                    echo -e "  ${MAGENTA}⚡ Threads: ${BOLD}${value}${RESET}" ;;

                *)
                    echo -e "  ${DIM}Event #${count}: ${id} ${action}=${value}${RESET}" ;;
            esac
        fi
    fi
done
