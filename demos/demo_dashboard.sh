#!/bin/bash
# ─────────────────────────────────────────────
# Demo 4: Dashboard — system monitors with
# live-updating progress bars and controls.
# ─────────────────────────────────────────────

widget() { printf '\033]1337;Widget=%s\007' "$1"; }
panel()  { printf '\033]1337;Panel=%s\007'  "$1"; }

RED='\033[31m'; GREEN='\033[32m'; YELLOW='\033[33m'; BLUE='\033[34m'
MAGENTA='\033[35m'; CYAN='\033[36m'; BOLD='\033[1m'; DIM='\033[2m'
RESET='\033[0m'; BG_GREEN='\033[42m'; BG_RED='\033[41m'; BG_YELLOW='\033[43m'

clear
stty -echo
trap 'stty sane' EXIT
trap 'exit 0' INT TERM

echo -e "${BOLD}${MAGENTA}╔══════════════════════════════════════════════════════╗${RESET}"
echo -e "${BOLD}${MAGENTA}║              SmartTerm Dashboard                     ║${RESET}"
echo -e "${BOLD}${MAGENTA}╚══════════════════════════════════════════════════════╝${RESET}"
echo ""

# Status panel
echo -n "  "
panel "id:status;layout:vertical;title:📊 System Status;spacing:6;margin:10;width:350"
widget "type:label;id:lbl_cpu;text:CPU Usage;panel:status"
widget "type:progressbar;id:pb_cpu;value:73;width:300;text:73%;panel:status"
widget "type:label;id:lbl_mem;text:Memory;panel:status"
widget "type:progressbar;id:pb_mem;value:45;width:300;text:45%;panel:status"
widget "type:label;id:lbl_disk;text:Disk;panel:status"
widget "type:progressbar;id:pb_disk;value:88;width:300;text:88%;panel:status"
echo ""
echo ""

# Controls
echo -n "  "
panel "id:controls;layout:horizontal;title:⚡ Quick Actions;spacing:8;margin:10"
widget "type:button;id:btn_refresh;label:🔄 Refresh;panel:controls"
widget "type:button;id:btn_restart;label:🔁 Restart;panel:controls"
widget "type:button;id:btn_stop;label:🛑 Stop;panel:controls"
widget "type:button;id:btn_logs;label:📜 Logs;panel:controls"
widget "type:button;id:btn_deploy;label:🚀 Deploy;panel:controls"
widget "type:close;id:quit;label:✕ Quit;panel:controls"
echo ""
echo ""

# Tuning grid
echo -n "  "
panel "id:tuning;layout:grid;title:🔧 Performance Tuning;spacing:8;margin:10;width:400"
widget "type:label;id:lbl_workers;text:Workers;panel:tuning;row:0;col:0"
widget "type:spinbutton;id:sp_workers;min:1;max:64;value:8;step:1;panel:tuning;row:0;col:1"
widget "type:label;id:lbl_cache;text:Cache (MB);panel:tuning;row:1;col:0"
widget "type:spinbutton;id:sp_cache;min:64;max:4096;value:512;step:64;panel:tuning;row:1;col:1"
widget "type:label;id:lbl_loglevel;text:Log Level;panel:tuning;row:2;col:0"
widget "type:dropdown;id:dd_log;items:Debug,Info,Warning,Error;panel:tuning;row:2;col:1"
widget "type:label;id:lbl_compress;text:Compression;panel:tuning;row:3;col:0"
widget "type:switch;id:sw_compress;active:true;panel:tuning;row:3;col:1"
widget "type:separator;id:sep_apply;panel:tuning;row:4;col:0;colspan:2"
widget "type:button;id:btn_apply;label:✅ Apply Changes;panel:tuning;row:5;col:0;colspan:2"
echo ""
echo ""

echo -e "  ${DIM}─── System Log ────────────────────────${RESET}"
echo ""

ts() { date +%H:%M:%S; }

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
                btn_refresh)
                    echo -e "  ${DIM}[$(ts)]${RESET} ${CYAN}🔄 Refreshing metrics...${RESET}"
                    sleep 0.3
                    echo -e "  ${DIM}[$(ts)]${RESET} ${GREEN}✓ Metrics updated${RESET}" ;;
                btn_restart)
                    echo -e "  ${DIM}[$(ts)]${RESET} ${YELLOW}🔁 Restarting services...${RESET}"
                    for i in 1 2 3; do
                        sleep 0.3
                        echo -e "  ${DIM}[$(ts)]${RESET} ${DIM}  Service $i restarted${RESET}"
                    done
                    echo -e "  ${DIM}[$(ts)]${RESET} ${GREEN}${BOLD}✓ All services running${RESET}" ;;
                btn_stop)
                    echo -e "  ${DIM}[$(ts)]${RESET} ${RED}${BOLD}🛑 Services stopped!${RESET}" ;;
                btn_logs)
                    echo -e "  ${DIM}[$(ts)]${RESET} ${BLUE}📜 Recent log entries:${RESET}"
                    echo -e "  ${DIM}  INFO  Connection from 10.0.0.1${RESET}"
                    echo -e "  ${DIM}  INFO  Query completed in 23ms${RESET}"
                    echo -e "  ${YELLOW}  WARN  Cache miss rate: 12%${RESET}"
                    echo -e "  ${DIM}  INFO  Healthcheck passed${RESET}" ;;
                btn_deploy)
                    echo -e "  ${DIM}[$(ts)]${RESET} ${MAGENTA}🚀 Deploying...${RESET}"
                    for step in "Building" "Testing" "Packaging" "Uploading"; do
                        sleep 0.4
                        echo -e "  ${DIM}[$(ts)]${RESET} ${CYAN}  ▸ ${step}...${RESET}"
                    done
                    echo -e "  ${DIM}[$(ts)]${RESET} ${BG_GREEN}${BOLD} ✓ Deployed successfully! ${RESET}" ;;
                sp_workers)
                    echo -e "  ${DIM}[$(ts)]${RESET} ${YELLOW}Workers → ${BOLD}${value}${RESET}" ;;
                sp_cache)
                    echo -e "  ${DIM}[$(ts)]${RESET} ${YELLOW}Cache → ${BOLD}${value} MB${RESET}" ;;
                dd_log)
                    case "$value" in
                        Debug)   echo -e "  ${DIM}[$(ts)]${RESET} ${DIM}Log level → DEBUG (verbose)${RESET}" ;;
                        Info)    echo -e "  ${DIM}[$(ts)]${RESET} ${GREEN}Log level → INFO${RESET}" ;;
                        Warning) echo -e "  ${DIM}[$(ts)]${RESET} ${YELLOW}Log level → WARNING${RESET}" ;;
                        Error)   echo -e "  ${DIM}[$(ts)]${RESET} ${RED}Log level → ERROR only${RESET}" ;;
                    esac ;;
                sw_compress)
                    [[ "$value" == "true" ]] \
                        && echo -e "  ${DIM}[$(ts)]${RESET} ${GREEN}Compression enabled${RESET}" \
                        || echo -e "  ${DIM}[$(ts)]${RESET} ${YELLOW}Compression disabled${RESET}" ;;
                btn_apply)
                    echo -e "  ${DIM}[$(ts)]${RESET} ${BG_GREEN}${BOLD} ✓ Configuration applied ${RESET}" ;;
            esac
        fi
    fi
done
