#!/bin/bash
# ─────────────────────────────────────────────
# Demo 7: User Profile — combines grids, radio
# groups, calendar, color picker, password, etc.
# into a realistic form layout.
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

echo -e "${BOLD}${BLUE}┌──────────────────────────────────────┐${RESET}"
echo -e "${BOLD}${BLUE}│  Demo 7: User Profile Setup          │${RESET}"
echo -e "${BOLD}${BLUE}└──────────────────────────────────────┘${RESET}"
echo ""

# ── Account Info Grid ──
echo -n "  "
panel "id:account;layout:grid;title:👤 Account Information;spacing:8;margin:12;width:450"
widget "type:label;id:l1;text:Display Name;panel:account;row:0;col:0"
widget "type:entry;id:name;placeholder:John Doe;width:22;panel:account;row:0;col:1"
widget "type:label;id:l2;text:Email;panel:account;row:1;col:0"
widget "type:entry;id:email;placeholder:john@example.com;width:22;panel:account;row:1;col:1"
widget "type:label;id:l3;text:Password;panel:account;row:2;col:0"
widget "type:password;id:pass;placeholder:Min 8 characters;panel:account;row:2;col:1"
widget "type:label;id:l4;text:Birthday;panel:account;row:3;col:0"
widget "type:calendar;id:birthday;panel:account;row:3;col:1"
echo ""
echo ""

# ── Subscription Plan ──
echo -n "  "
panel "id:plan;layout:grid;title:📦 Subscription Plan;spacing:8;margin:12;width:450"
widget "type:radio;id:tier_free;label:Free — \$0/mo;group:tier;panel:plan;row:0;col:0;colspan:2"
widget "type:radio;id:tier_plus;label:Plus — \$9/mo;group:tier;panel:plan;row:1;col:0;colspan:2"
widget "type:radio;id:tier_pro;label:Pro — \$19/mo;group:tier;panel:plan;row:2;col:0;colspan:2"
widget "type:radio;id:tier_ent;label:Enterprise — Custom;group:tier;panel:plan;row:3;col:0;colspan:2"
widget "type:label;id:l_bill;text:Billing;panel:plan;row:4;col:0"
widget "type:dropdown;id:billing;items:Monthly,Yearly (save 20%);panel:plan;row:4;col:1"
echo ""
echo ""

# ── Theme Settings ──
echo -n "  "
panel "id:theme;layout:grid;title:🎨 Theme Preferences;spacing:8;margin:12;width:450"
widget "type:label;id:lt1;text:Accent Color;panel:theme;row:0;col:0"
widget "type:colorbutton;id:accent;value:#3584e4;title:Choose accent color;panel:theme;row:0;col:1"
widget "type:label;id:lt2;text:Background;panel:theme;row:1;col:0"
widget "type:colorbutton;id:bg_color;value:#1e1e2e;title:Choose background;panel:theme;row:1;col:1"
widget "type:label;id:lt3;text:Dark Mode;panel:theme;row:2;col:0"
widget "type:switch;id:darkmode;active:true;panel:theme;row:2;col:1"
widget "type:label;id:lt4;text:Font Size;panel:theme;row:3;col:0"
widget "type:spinbutton;id:fontsize;min:8;max:32;value:14;step:1;panel:theme;row:3;col:1"
echo ""
echo ""

# ── Notifications ──
echo -n "  "
panel "id:notifs;layout:vertical;title:🔔 Notifications;spacing:6;margin:12;width:450"
widget "type:checkbox;id:n_email;label:Email notifications;panel:notifs"
widget "type:checkbox;id:n_push;label:Push notifications;panel:notifs"
widget "type:checkbox;id:n_sms;label:SMS alerts;panel:notifs"
widget "type:label;id:n_freq_lbl;text:Digest frequency:;panel:notifs"
widget "type:dropdown;id:n_freq;items:Instant,Hourly,Daily,Weekly;panel:notifs"
echo ""
echo ""

# ── Actions ──
echo -n "  "
panel "id:actions;layout:horizontal;spacing:12;margin:8"
widget "type:button;id:btn_save;label:💾 Save Profile;panel:actions"
widget "type:button;id:btn_reset;label:↺ Reset;panel:actions"
widget "type:link;id:lnk_terms;uri:https://example.com/terms;label:Terms of Service;panel:actions"
widget "type:close;id:quit;label:✕ Quit;panel:actions"
echo ""
echo ""

echo -e "  ${BOLD}${DIM}─── Profile Events ────────────────────${RESET}"

username=""; tier="Free"; bday=""
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
                name)
                    username="$value"
                    echo -e "  ${BLUE}👤 Name: ${BOLD}${value}${RESET}" ;;
                email)
                    echo -e "  ${CYAN}📧 Email: ${BOLD}${value}${RESET}" ;;
                pass)
                    len=${#value}
                    if (( len >= 8 )); then
                        echo -e "  ${GREEN}🔐 Password: strong (${len} chars) ✓${RESET}"
                    else
                        echo -e "  ${RED}🔐 Password: too short (${len}/8 min) ✗${RESET}"
                    fi ;;
                birthday)
                    bday="$value"
                    echo -e "  ${MAGENTA}🎂 Birthday: ${BOLD}${value}${RESET}" ;;

                tier_free)  tier="Free";       echo -e "  ${GREEN}📦 Plan: ${BOLD}Free${RESET}" ;;
                tier_plus)  tier="Plus";       echo -e "  ${YELLOW}📦 Plan: ${BOLD}Plus — \$9/mo${RESET}" ;;
                tier_pro)   tier="Pro";        echo -e "  ${CYAN}📦 Plan: ${BOLD}Pro — \$19/mo${RESET}" ;;
                tier_ent)   tier="Enterprise"; echo -e "  ${MAGENTA}📦 Plan: ${BOLD}Enterprise${RESET}" ;;
                billing)
                    echo -e "  ${DIM}💳 Billing: ${value}${RESET}" ;;

                accent)
                    echo -e "  ${BOLD}🎨 Accent: ${value}${RESET}" ;;
                bg_color)
                    echo -e "  ${BOLD}🖌 Background: ${value}${RESET}" ;;
                darkmode)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${DIM}🌙 Dark mode enabled${RESET}" \
                        || echo -e "  ${YELLOW}☀ Light mode${RESET}" ;;
                fontsize)
                    echo -e "  ${MAGENTA}🔤 Font: ${BOLD}${value}pt${RESET}" ;;

                n_email|n_push|n_sms)
                    lbl="${id#n_}"
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${GREEN}🔔 ${lbl}: enabled${RESET}" \
                        || echo -e "  ${DIM}🔕 ${lbl}: disabled${RESET}" ;;
                n_freq)
                    echo -e "  ${CYAN}📬 Digest: ${BOLD}${value}${RESET}" ;;

                btn_save)
                    if [[ -n "$username" ]]; then
                        echo -e "  ${BG_GREEN}${BOLD} ✓ Profile saved for ${username} (${tier}) ${RESET}"
                    else
                        echo -e "  ${BG_RED}${BOLD} ✗ Enter a name first! ${RESET}"
                    fi ;;
                btn_reset)
                    echo -e "  ${YELLOW}↺ Form reset requested${RESET}" ;;

                *)
                    echo -e "  ${DIM}${id}: ${action}=${value}${RESET}" ;;
            esac
        fi
    fi
done
