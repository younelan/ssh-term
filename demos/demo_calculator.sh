#!/bin/bash
# ─────────────────────────────────────────────
# Demo: Calculator
# A fully functional GUI calculator using
# panels, grid layout, buttons & WidgetUpdate
# ─────────────────────────────────────────────

widget() { printf '\033]1337;Widget=%s\007' "$1"; }
panel()  { printf '\033]1337;Panel=%s\007'  "$1"; }
update() { printf '\033]1337;WidgetUpdate=%s\007' "$1"; }

clear
stty -echo
trap 'stty sane' EXIT
trap 'exit 0' INT TERM

echo ""

# ── Calculator frame ──
panel "id:calc;layout:vertical;title:🔢 Calculator;spacing:4;margin:12;width:300"

# Display
widget "type:entry;id:display;placeholder:0;width:26;panel:calc"

# Button grid
panel "id:grid;layout:grid;spacing:4;margin:4;panel:calc"

# Row 0: C, ±, %, ÷
widget "type:button;id:btn_c;label:C;panel:grid;row:0;col:0"
widget "type:button;id:btn_sign;label:±;panel:grid;row:0;col:1"
widget "type:button;id:btn_pct;label:%;panel:grid;row:0;col:2"
widget "type:button;id:btn_div;label:÷;panel:grid;row:0;col:3"

# Row 1: 7 8 9 ×
widget "type:button;id:btn_7;label:7;panel:grid;row:1;col:0"
widget "type:button;id:btn_8;label:8;panel:grid;row:1;col:1"
widget "type:button;id:btn_9;label:9;panel:grid;row:1;col:2"
widget "type:button;id:btn_mul;label:×;panel:grid;row:1;col:3"

# Row 2: 4 5 6 −
widget "type:button;id:btn_4;label:4;panel:grid;row:2;col:0"
widget "type:button;id:btn_5;label:5;panel:grid;row:2;col:1"
widget "type:button;id:btn_6;label:6;panel:grid;row:2;col:2"
widget "type:button;id:btn_sub;label:−;panel:grid;row:2;col:3"

# Row 3: 1 2 3 +
widget "type:button;id:btn_1;label:1;panel:grid;row:3;col:0"
widget "type:button;id:btn_2;label:2;panel:grid;row:3;col:1"
widget "type:button;id:btn_3;label:3;panel:grid;row:3;col:2"
widget "type:button;id:btn_add;label:+;panel:grid;row:3;col:3"

# Row 4: 0 (span 2), ., =
widget "type:button;id:btn_0;label:0;panel:grid;row:4;col:0;colspan:2"
widget "type:button;id:btn_dot;label:.;panel:grid;row:4;col:2"
widget "type:button;id:btn_eq;label:=;panel:grid;row:4;col:3"

# History log
panel "id:hist;layout:vertical;title:📜 History;spacing:2;margin:4;panel:calc"
widget "type:textview;id:history;width:270;height:120;editable:false;wrap:word;panel:hist"
update "id:history;text:Calculations will appear here."

# Close button
widget "type:close;id:quit;label:✕ Quit;panel:calc"

# ── State ──
current=""
operator=""
operand=""
fresh="true"

show() {
    local val="$1"
    [[ -z "$val" ]] && val="0"
    update "id:display;text:$val"
}

calc() {
    local a="$1" op="$2" b="$3"
    if [[ -z "$a" || -z "$b" ]]; then echo ""; return; fi
    case "$op" in
        "+")  echo "$a + $b" | bc -l ;;
        "-")  echo "$a - $b" | bc -l ;;
        "*")  echo "$a * $b" | bc -l ;;
        "/")
            if [[ "$b" == "0" || "$b" == "0.0" ]]; then
                echo "Error"
            else
                echo "$a / $b" | bc -l
            fi ;;
        *)    echo "$a" ;;
    esac
}

# Clean trailing zeros: 3.00 -> 3, 3.10 -> 3.1
clean() {
    local n="$1"
    if [[ "$n" == *"."* ]]; then
        n="${n%%*(0)}"   # zsh: strip trailing zeros
        n="${n%.}"       # strip trailing dot
    fi
    # bc may give .5 instead of 0.5
    [[ "$n" == .* ]] && n="0$n"
    [[ "$n" == -.* ]] && n="-0${n#-}"
    echo "$n"
}

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

            case "$id" in
                # Digit buttons
                btn_0) d="0" ;; btn_1) d="1" ;; btn_2) d="2" ;;
                btn_3) d="3" ;; btn_4) d="4" ;; btn_5) d="5" ;;
                btn_6) d="6" ;; btn_7) d="7" ;; btn_8) d="8" ;;
                btn_9) d="9" ;; btn_dot) d="." ;; *) d="" ;;
            esac

            # Digit/dot pressed
            if [[ -n "$d" ]]; then
                if [[ "$fresh" == "true" ]]; then
                    current="$d"
                    fresh="false"
                else
                    # Prevent multiple dots
                    if [[ "$d" == "." && "$current" == *"."* ]]; then
                        :
                    else
                        current="${current}${d}"
                    fi
                fi
                show "$current"
                continue
            fi

            case "$id" in
                btn_c)
                    current=""
                    operator=""
                    operand=""
                    fresh="true"
                    show "0"
                    ;;
                btn_sign)
                    if [[ -n "$current" && "$current" != "0" ]]; then
                        if [[ "$current" == -* ]]; then
                            current="${current#-}"
                        else
                            current="-${current}"
                        fi
                        show "$current"
                    fi
                    ;;
                btn_pct)
                    if [[ -n "$current" ]]; then
                        current=$(echo "$current / 100" | bc -l)
                        current=$(clean "$current")
                        show "$current"
                    fi
                    ;;
                btn_add|btn_sub|btn_mul|btn_div)
                    if [[ -n "$operand" && -n "$current" && -n "$operator" ]]; then
                        result=$(calc "$operand" "$operator" "$current")
                        result=$(clean "$result")
                        operand="$result"
                        show "$result"
                    elif [[ -n "$current" ]]; then
                        operand="$current"
                    fi
                    case "$id" in
                        btn_add) operator="+" ;;
                        btn_sub) operator="-" ;;
                        btn_mul) operator="*" ;;
                        btn_div) operator="/" ;;
                    esac
                    fresh="true"
                    ;;
                btn_eq)
                    if [[ -n "$operand" && -n "$current" && -n "$operator" ]]; then
                        result=$(calc "$operand" "$operator" "$current")
                        result=$(clean "$result")
                        # Log to history
                        op_sym="$operator"
                        case "$operator" in
                            "*") op_sym="×" ;; "/") op_sym="÷" ;;
                        esac
                        update "id:history;append:$operand $op_sym $current = $result"
                        current="$result"
                        operand=""
                        operator=""
                        fresh="true"
                        show "$result"
                    fi
                    ;;
                display)
                    # User typed directly into the entry and pressed Enter
                    if [[ -n "$value" ]]; then
                        # Try to evaluate the expression
                        result=$(echo "$value" | bc -l 2>/dev/null)
                        if [[ -n "$result" ]]; then
                            result=$(clean "$result")
                            update "id:history;append:$value = $result"
                            current="$result"
                            fresh="true"
                            show "$result"
                        fi
                    fi
                    ;;
            esac
        fi
    fi
done
