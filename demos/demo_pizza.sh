#!/bin/bash
# ─────────────────────────────────────────────
# Demo 5: Interactive Pizza Order Form
# Full order workflow with live price calc.
# ─────────────────────────────────────────────

widget() { printf '\033]1337;Widget=%s\007' "$1"; }
panel()  { printf '\033]1337;Panel=%s\007'  "$1"; }

RED='\033[31m'; GREEN='\033[32m'; YELLOW='\033[33m'; BLUE='\033[34m'
MAGENTA='\033[35m'; CYAN='\033[36m'; BOLD='\033[1m'; DIM='\033[2m'
RESET='\033[0m'; BG_GREEN='\033[42m'; BG_YELLOW='\033[43m'

clear
stty -echo
trap 'stty sane' EXIT
trap 'exit 0' INT TERM

echo -e "${BOLD}${RED}┌──────────────────────────────────────┐${RESET}"
echo -e "${BOLD}${RED}│  🍕 Pizza Palace — Order Form        │${RESET}"
echo -e "${BOLD}${RED}└──────────────────────────────────────┘${RESET}"
echo ""

echo -n "  "
panel "id:order;layout:grid;title:Build Your Pizza;spacing:10;margin:16;width:420"

widget "type:label;id:lbl_size;text:Size;panel:order;row:0;col:0"
widget "type:dropdown;id:dd_size;items:Small (\$8),Medium (\$12),Large (\$16);panel:order;row:0;col:1"

widget "type:label;id:lbl_crust;text:Crust;panel:order;row:1;col:0"
widget "type:dropdown;id:dd_crust;items:Thin,Regular,Stuffed (+\$2);panel:order;row:1;col:1"

widget "type:label;id:lbl_top;text:Toppings;panel:order;row:2;col:0"

# Toppings sub-panel inside the grid
echo -n ""
panel "id:toppings;layout:vertical;spacing:2;margin:0"
widget "type:checkbox;id:chk_cheese;label:Extra Cheese (+\$1);panel:toppings"
widget "type:checkbox;id:chk_pepperoni;label:Pepperoni (+\$1.50);panel:toppings"
widget "type:checkbox;id:chk_mushroom;label:Mushrooms (+\$1);panel:toppings"
widget "type:checkbox;id:chk_olive;label:Olives (+\$0.75);panel:toppings"
widget "type:checkbox;id:chk_pepper;label:Bell Peppers (+\$0.75);panel:toppings"
echo ""

echo ""
echo -n "  "
panel "id:details;layout:grid;title:Order Details;spacing:10;margin:16;width:420"
widget "type:label;id:lbl_qty;text:Quantity;panel:details;row:0;col:0"
widget "type:spinbutton;id:sp_qty;min:1;max:20;value:1;step:1;panel:details;row:0;col:1"
widget "type:label;id:lbl_notes;text:Notes;panel:details;row:1;col:0"
widget "type:entry;id:in_notes;placeholder:Special instructions...;width:25;panel:details;row:1;col:1"
widget "type:label;id:lbl_tip;text:Tip %;panel:details;row:2;col:0"
widget "type:slider;id:sl_tip;min:0;max:30;value:15;width:180;panel:details;row:2;col:1"
widget "type:separator;id:sep_btn;panel:details;row:3;col:0;colspan:2"
widget "type:button;id:btn_order;label:🛒 Place Order;panel:details;row:4;col:0;colspan:2"
widget "type:close;id:quit;label:✕ Quit;panel:details;row:5;col:0;colspan:2"
echo ""
echo ""

echo -e "  ${DIM}─── Order Updates ─────────────────────${RESET}"
echo ""

# Track state
base_price=8; crust_extra=0; qty=1; tip=15
cheese=0; pepperoni=0; mushroom=0; olive=0; pepper=0

calc_total() {
    toppings=$(echo "$cheese + $pepperoni + $mushroom + $olive + $pepper" | bc)
    subtotal=$(echo "($base_price + $crust_extra + $toppings) * $qty" | bc)
    tip_amt=$(echo "scale=2; $subtotal * $tip / 100" | bc)
    total=$(echo "scale=2; $subtotal + $tip_amt" | bc)
    echo -e "  ${YELLOW}💰 Subtotal: \$${subtotal}  |  Tip: \$${tip_amt}  |  ${BOLD}Total: \$${total}${RESET}"
}

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
                dd_size)
                    case "$value" in
                        *8*)  base_price=8;  echo -e "  ${CYAN}📏 Small pizza${RESET}" ;;
                        *12*) base_price=12; echo -e "  ${CYAN}📏 Medium pizza${RESET}" ;;
                        *16*) base_price=16; echo -e "  ${CYAN}📏 Large pizza${RESET}" ;;
                    esac
                    calc_total ;;
                dd_crust)
                    case "$value" in
                        Stuffed*) crust_extra=2; echo -e "  ${MAGENTA}🍞 Stuffed crust (+\$2)${RESET}" ;;
                        *)        crust_extra=0; echo -e "  ${MAGENTA}🍞 ${value} crust${RESET}" ;;
                    esac
                    calc_total ;;
                chk_cheese)
                    [[ "$value" == "true" ]] && cheese="1" || cheese="0"
                    calc_total ;;
                chk_pepperoni)
                    [[ "$value" == "true" ]] && pepperoni="1.50" || pepperoni="0"
                    calc_total ;;
                chk_mushroom)
                    [[ "$value" == "true" ]] && mushroom="1" || mushroom="0"
                    calc_total ;;
                chk_olive)
                    [[ "$value" == "true" ]] && olive="0.75" || olive="0"
                    calc_total ;;
                chk_pepper)
                    [[ "$value" == "true" ]] && pepper="0.75" || pepper="0"
                    calc_total ;;
                sp_qty)
                    qty="$value"
                    echo -e "  ${BLUE}📦 Quantity: ${qty}${RESET}"
                    calc_total ;;
                sl_tip)
                    tip="$value"
                    echo -e "  ${DIM}💵 Tip: ${tip}%${RESET}"
                    calc_total ;;
                in_notes)
                    echo -e "  ${DIM}📝 Note: \"${value}\"${RESET}" ;;
                btn_order)
                    echo ""
                    echo -e "  ${BG_GREEN}${BOLD}                                      ${RESET}"
                    echo -e "  ${BG_GREEN}${BOLD}  🎉 ORDER PLACED!                    ${RESET}"
                    echo -e "  ${BG_GREEN}${BOLD}  Your pizza is being prepared...      ${RESET}"
                    echo -e "  ${BG_GREEN}${BOLD}                                      ${RESET}"
                    echo ""
                    for step in "🔥 Firing up the oven" "🧑‍🍳 Kneading the dough" "🧀 Adding toppings" "⏱ Baking..." "📦 Boxing up"; do
                        sleep 0.5
                        echo -e "  ${CYAN}${step}${RESET}"
                    done
                    echo ""
                    echo -e "  ${GREEN}${BOLD}🛵 Your pizza is on the way!${RESET}"
                    echo "" ;;
            esac
        fi
    fi
done
