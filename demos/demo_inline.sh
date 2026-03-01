#!/bin/bash
# ─────────────────────────────────────────────
# Demo 1: Inline Widgets — all widget types
# showcased with colorful live event feedback
# ─────────────────────────────────────────────

widget() { printf '\033]1337;Widget=%s\007' "$1"; }

RED='\033[31m'; GREEN='\033[32m'; YELLOW='\033[33m'; BLUE='\033[34m'
MAGENTA='\033[35m'; CYAN='\033[36m'; BOLD='\033[1m'; DIM='\033[2m'; RESET='\033[0m'

clear
stty -echo
trap 'stty sane' EXIT
trap 'exit 0' INT TERM

echo -e "${BOLD}${CYAN}┌──────────────────────────────────────┐${RESET}"
echo -e "${BOLD}${CYAN}│  Demo 1: Inline Widgets              │${RESET}"
echo -e "${BOLD}${CYAN}└──────────────────────────────────────┘${RESET}"
echo ""

echo -n "  Button:     "
widget "type:button;id:btn1;label:Click Me"
echo -n "  "
widget "type:button;id:btn2;label:Another One"
echo ""

echo -n "  Entry:      "
widget "type:entry;id:input1;placeholder:Type here & press Enter;width:30"
echo ""

echo -n "  Checkbox:   "
widget "type:checkbox;id:chk1;label:Enable feature"
echo -n "  "
widget "type:checkbox;id:chk2;label:Dark mode"
echo ""

echo -n "  Switch:     "
widget "type:switch;id:sw1;active:false"
echo -n "  "
widget "type:label;id:lbl_sw;text:Off"
echo ""

echo -n "  Slider:     "
widget "type:slider;id:sl1;min:0;max:100;value:50;width:200"
echo ""

echo -n "  Spin:       "
widget "type:spinbutton;id:spin1;min:0;max:50;value:10;step:1"
echo ""

echo -n "  Dropdown:   "
widget "type:dropdown;id:dd1;items:Apple,Banana,Cherry,Date"
echo ""

echo -n "  Progress:   "
widget "type:progressbar;id:pb1;value:0;width:250;text:0%"
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
            id=""; action=""; value=""
            IFS=';' read -ra parts <<< "$evt"
            for p in "${parts[@]}"; do
                case "$p" in
                    id:*)     id="${p#id:}" ;;
                    action:*) action="${p#action:}" ;;
                    value:*)  value="${p#value:}" ;;
                esac
            done
            count=$((count + 1))
            case "$id" in
                btn1)   echo -e "  ${GREEN}✓ Button 1 clicked! (${count} events)${RESET}" ;;
                btn2)   echo -e "  ${MAGENTA}★ Button 2 clicked! (${count} events)${RESET}" ;;
                input1) echo -e "  ${BLUE}📝 You typed: \"${BOLD}${value}${RESET}${BLUE}\"${RESET}" ;;
                chk1)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${GREEN}☑ Feature enabled${RESET}" \
                        || echo -e "  ${RED}☐ Feature disabled${RESET}" ;;
                chk2)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${YELLOW}🌙 Dark mode ON${RESET}" \
                        || echo -e "  ${CYAN}☀ Light mode ON${RESET}" ;;
                sw1)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${GREEN}🔛 Switch ON${RESET}" \
                        || echo -e "  ${RED}⭘ Switch OFF${RESET}" ;;
                sl1)
                    bar=""; filled=$((value / 5))
                    for ((i=0;i<filled;i++)); do bar+="█"; done
                    for ((i=filled;i<20;i++)); do bar+="░"; done
                    echo -e "  ${CYAN}Slider: [${bar}] ${value}%${RESET}" ;;
                spin1)  echo -e "  ${YELLOW}🔢 Spin: ${BOLD}${value}${RESET}" ;;
                dd1)    echo -e "  ${MAGENTA}📋 Selected: ${BOLD}${value}${RESET}" ;;
            esac
        fi
    fi
done
