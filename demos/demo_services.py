#!/usr/bin/env python3
"""
demo_services.py — Linux systemd service manager.

Left panel : filterable table of all services (active/failed/inactive).
Right panel: selected service details + Start / Stop / Restart /
             Enable / Disable / Status / Journal buttons + output log.

Requires systemd (Linux). Mutating commands (start/stop/…) use sudo
if the current user is not root; the command is printed before running so
you can see exactly what's being executed.
"""
import sys, os, tty, termios, time, subprocess, select

ESC, BEL = "\033", "\007"

# ── OSC helpers ───────────────────────────────────────────────────────────────
def osc(cmd):
    sys.stdout.write(f"{ESC}]1337;{cmd}{BEL}")
    sys.stdout.flush()

def enc_val(s: str) -> str:
    """Escape a string for embedding in an OSC property value."""
    out = []
    for ch in s:
        if   ch == "\\": out.append("\\\\")
        elif ch == "\n": out.append("\\n")
        elif ch == "\r": out.append("\\r")
        elif ch == ";":  out.append("\\;")
        else:            out.append(ch)
    return "".join(out)

def _split_props(s: str) -> list:
    parts, cur, i = [], [], 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s) and s[i + 1] == ";":
            cur.append(";"); i += 2
        elif s[i] == ";":
            parts.append("".join(cur)); cur = []; i += 1
        else:
            cur.append(s[i]); i += 1
    parts.append("".join(cur))
    return parts

def P(s):    osc(f"Panel={s}")
def W(s):    osc(f"Widget={s}")
def WU(s):   osc(f"WidgetUpdate={s}")
def GV(wid): osc(f"GetWidgetValue=id:{wid}")

# ── Colour palette ────────────────────────────────────────────────────────────
BG  = "#0f1117"; BG2 = "#1a1f2e"; BG3 = "#0d1021"
FG  = "#e2e8f0"; DIM = "#4b5563"
GRN = "#10b981"; RED = "#ef4444"; YEL = "#f59e0b"
BLU = "#3b82f6"; PRP = "#8b5cf6"; CYN = "#06b6d4"; ORG = "#f97316"

STATE_COLOR = {
    "active":        GRN,
    "running":       GRN,
    "failed":        RED,
    "inactive":      DIM,
    "dead":          DIM,
    "activating":    YEL,
    "deactivating":  YEL,
    "reloading":     CYN,
    "waiting":       BLU,
    "enabled":       GRN,
    "disabled":      DIM,
}

def state_col(s: str) -> str:
    return STATE_COLOR.get(s.lower(), FG)

# ── Non-blocking event reader ─────────────────────────────────────────────────
def read_event_timeout(timeout: float):
    deadline = time.monotonic() + timeout
    buf = ""; in_seq = False
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return None
        r, _, _ = select.select([sys.stdin], [], [], min(remaining, 0.05))
        if not r:
            continue
        ch = sys.stdin.read(1)
        if ch == "\x03":
            raise SystemExit(0)
        if not in_seq:
            if ch == ESC:
                in_seq = True; buf = ""
        else:
            if ch in (BEL, "\n"):
                in_seq = False
                if "WidgetEvent=" in buf:
                    d = {}
                    for p in _split_props(buf.split("WidgetEvent=", 1)[1]):
                        if ":" in p:
                            k, v = p.split(":", 1)
                            d[k] = v
                    return d
                buf = ""
            else:
                buf += ch

# ── Systemd helpers ───────────────────────────────────────────────────────────
SUDO = [] if os.getuid() == 0 else ["sudo", "-n"]   # -n = no interactive password

# Strip ANSI/VT escape sequences from text so GTK TextBuffer never receives them.
_ANSI_RE = None
def strip_ansi(s: str) -> str:
    global _ANSI_RE
    if _ANSI_RE is None:
        import re
        _ANSI_RE = re.compile(r'\x1b(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])')
    return _ANSI_RE.sub('', s)

def run_cmd(args: list, timeout: int = 15) -> str:
    """Run a command, return combined stdout+stderr as string (ANSI-stripped)."""
    # Force plain text output from systemd tools.
    env = os.environ.copy()
    env["SYSTEMD_COLORS"] = "0"
    env["NO_COLOR"]        = "1"
    env["TERM"]            = "dumb"
    try:
        result = subprocess.run(
            args,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
            text=True,
            timeout=timeout,
            env=env,
        )
        return strip_ansi(result.stdout.strip()) or f"(exit code {result.returncode})"
    except subprocess.TimeoutExpired:
        return f"[timeout after {timeout}s]"
    except FileNotFoundError as e:
        return f"[command not found: {e}]"
    except Exception as e:
        return f"[error: {e}]"

def systemctl(*args) -> tuple:
    """Returns (cmd_str, output)."""
    cmd = ["systemctl", "--no-pager"] + list(args)
    cmd_str = " ".join(cmd)
    return cmd_str, run_cmd(cmd)

def systemctl_mut(*args) -> tuple:
    """Mutating systemctl command — uses sudo if not root."""
    cmd = SUDO + ["systemctl", "--no-pager"] + list(args)
    cmd_str = " ".join(cmd)
    return cmd_str, run_cmd(cmd)

def journalctl(unit: str, lines: int = 80) -> tuple:
    cmd = ["journalctl", "-u", unit, f"-n{lines}", "--no-pager", "--output=short-iso"]
    return " ".join(cmd), run_cmd(cmd)

Service = tuple  # (unit, load, active, sub, description)

def list_services() -> list:
    """Return list of (unit, load, active, sub, description)."""
    out = run_cmd([
        "systemctl", "list-units",
        "--type=service", "--all",
        "--no-pager", "--plain", "--no-legend",
    ])
    services = []
    for line in out.splitlines():
        parts = line.split(None, 4)
        if len(parts) >= 4:
            unit  = parts[0].replace("●", "").strip()
            load  = parts[1]
            active = parts[2]
            sub   = parts[3]
            desc  = parts[4].strip() if len(parts) > 4 else ""
            if unit.endswith(".service"):
                services.append((unit, load, active, sub, desc))
    # Also add failed units that might not appear above
    failed_out = run_cmd([
        "systemctl", "list-units",
        "--type=service", "--state=failed",
        "--no-pager", "--plain", "--no-legend",
    ])
    existing = {s[0] for s in services}
    for line in failed_out.splitlines():
        parts = line.split(None, 4)
        if len(parts) >= 4:
            unit = parts[0].replace("●", "").strip()
            if unit.endswith(".service") and unit not in existing:
                services.append((unit, parts[1], parts[2], parts[3],
                                  parts[4].strip() if len(parts) > 4 else ""))
    return sorted(services, key=lambda s: (s[2] != "failed", s[2] != "active", s[0]))

def service_summary(services: list) -> str:
    active   = sum(1 for s in services if s[2] == "active")
    failed   = sum(1 for s in services if s[2] == "failed")
    inactive = sum(1 for s in services if s[2] == "inactive")
    total    = len(services)
    return (f"total {total}   "
            f"● {active} active   "
            f"✕ {failed} failed   "
            f"○ {inactive} inactive")

# ══════════════════════════════════════════════════════════════════════════════
# UI build
# ══════════════════════════════════════════════════════════════════════════════
os.system("clear")
fd    = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)

try:
    # ── Check systemctl is available ──────────────────────────────────────────
    if not os.path.exists("/usr/bin/systemctl") and not os.path.exists("/bin/systemctl"):
        # Show a simple error panel and exit
        P(f"id:root;layout:vertical;spacing:20;margin:40;bg:{BG}")
        W(f"type:label;id:err;panel:root;"
          f"text:systemctl not found — this tool requires systemd (Linux);fg:{RED};size:14")
        W(f"type:button;id:btn_quit;panel:root;label:Close;bg:{RED};fg:#fff")
        while True:
            ev = read_event_timeout(60)
            if ev is None or ev.get("id") == "btn_quit":
                break
        raise SystemExit(0)

    # ── Root layout ───────────────────────────────────────────────────────────
    P(f"id:root;layout:vertical;spacing:6;margin:8;bg:{BG}")

    # ── Header bar ────────────────────────────────────────────────────────────
    P(f"id:header;panel:root;layout:horizontal;spacing:8")
    W(f"type:label;id:lbl_title;panel:header;"
      f"text:🔧 Service Manager;fg:{FG};font:Sans Bold;size:14;hexpand:true")
    W(f"type:label;id:lbl_summary;panel:header;"
      f"text:loading…;fg:{DIM};font:Monospace;size:10")
    W(f"type:button;id:btn_refresh;panel:header;"
      f"label:⟳ Refresh;tooltip:Reload service list")
    W(f"type:button;id:btn_quit;panel:header;"
      f"label:✕;bg:{RED};fg:#fff;tooltip:Quit")

    # ── Filter bar ────────────────────────────────────────────────────────────
    P(f"id:filterbar;panel:root;layout:horizontal;spacing:6")
    W(f"type:label;id:lbl_flt;panel:filterbar;text:Filter:;fg:{DIM};size:11")
    W(f"type:entry;id:filter_entry;panel:filterbar;"
      f"placeholder:name or state (active / failed / inactive…);hexpand:true")
    W(f"type:button;id:btn_search;panel:filterbar;label:🔍 Search")
    W(f"type:button;id:btn_clear_flt;panel:filterbar;label:✕ Clear;tooltip:Clear filter")
    # Quick filter toggles
    W(f"type:button;id:btn_flt_active;panel:filterbar;"
      f"label:Active;fg:{GRN};tooltip:Show only active services")
    W(f"type:button;id:btn_flt_failed;panel:filterbar;"
      f"label:Failed;fg:{RED};tooltip:Show only failed services")
    W(f"type:button;id:btn_flt_all;panel:filterbar;"
      f"label:All;tooltip:Show all services")

    # ── Main body: service table (left) + detail panel (right) ────────────────
    P(f"id:body;panel:root;layout:horizontal;spacing:8")

    # — Left: service table —
    P(f"id:left;panel:body;layout:vertical;spacing:4")
    W(f"type:table;id:svc_list;panel:left;"
      f"cols:Service|Active|Sub|Load;"
      f"widths:230|70|90|65;"
      f"sortable:false;"
      f"width:465;height:500")

    # — Right: detail + actions + output —
    P(f"id:right;panel:body;layout:vertical;spacing:6;margin:6;bg:{BG2};width:460")

    # Selected service label
    W(f"type:label;id:lbl_sel;panel:right;"
      f"text:← Select a service;fg:{DIM};font:Sans Italic;size:12")

    # Enable state badge
    W(f"type:label;id:lbl_enabled;panel:right;"
      f"text: ;fg:{DIM};font:Monospace;size:10")

    # Action buttons row 1: read-only
    P(f"id:act_row1;panel:right;layout:horizontal;spacing:6")
    W(f"type:button;id:btn_status;panel:act_row1;"
      f"label:📋 Status;tooltip:Show service status")
    W(f"type:button;id:btn_journal;panel:act_row1;"
      f"label:📜 Journal;tooltip:Show recent journal entries")
    W(f"type:button;id:btn_deps;panel:act_row1;"
      f"label:🔗 Deps;tooltip:Show service dependencies")

    # Action buttons row 2: mutating
    P(f"id:act_row2;panel:right;layout:horizontal;spacing:6")
    W(f"type:button;id:btn_start;panel:act_row2;"
      f"label:▶ Start;bg:{GRN};fg:#fff;tooltip:Start service (sudo)")
    W(f"type:button;id:btn_stop;panel:act_row2;"
      f"label:■ Stop;bg:{RED};fg:#fff;tooltip:Stop service (sudo)")
    W(f"type:button;id:btn_restart;panel:act_row2;"
      f"label:↺ Restart;bg:{BLU};fg:#fff;tooltip:Restart service (sudo)")
    W(f"type:button;id:btn_reload;panel:act_row2;"
      f"label:⟳ Reload;bg:{CYN};fg:#fff;tooltip:Reload service config (sudo)")

    # Action buttons row 3: unit-file
    P(f"id:act_row3;panel:right;layout:horizontal;spacing:6")
    W(f"type:button;id:btn_enable;panel:act_row3;"
      f"label:✓ Enable;bg:{PRP};fg:#fff;tooltip:Enable at boot (sudo)")
    W(f"type:button;id:btn_disable;panel:act_row3;"
      f"label:✗ Disable;bg:{ORG};fg:#fff;tooltip:Disable at boot (sudo)")
    W(f"type:button;id:btn_mask;panel:act_row3;"
      f"label:🚫 Mask;fg:{RED};tooltip:Mask service (prevent start) (sudo)")
    W(f"type:button;id:btn_unmask;panel:act_row3;"
      f"label:🔓 Unmask;tooltip:Unmask service (sudo)")

    # Output log
    W(f"type:label;id:lbl_cmdline;panel:right;"
      f"text: ;fg:{DIM};font:Monospace;size:9")
    W(f"type:textarea;id:output;panel:right;"
      f"editable:false;wrap:wordchar;font:Monospace;size:11;"
      f"width:446;height:250;bg:{BG3};fg:{FG};"
      f"text:Select a service and use the buttons above.")

    # ── State ─────────────────────────────────────────────────────────────────
    all_services    = [[]]   # mutable reference  (list of Service tuples)
    shown_services  = [[]]
    selected_svc    = [None]
    pending_filter  = [False]

    def populate_table(services):
        shown_services[0] = services
        WU("id:svc_list;action:clear")
        for (unit, load, active, sub, desc) in services:
            c = state_col(active) if active != "active" else state_col(sub)
            short = unit.replace(".service", "")
            WU(f"id:svc_list;action:addrow;"
               f"cols:{short}|{active}|{sub}|{load};fg:{c}")

    def set_output(cmd_str: str, text: str):
        WU(f"id:lbl_cmdline;text:{enc_val('$ ' + cmd_str)};fg:{DIM}")
        WU(f"id:output;text:{enc_val(text)}")

    def show_selected_info():
        svc = selected_svc[0]
        if not svc:
            return
        WU(f"id:lbl_sel;text:{svc};fg:{CYN};font:Monospace Bold;size:12")
        # Get enabled state
        _, enabled_out = systemctl("is-enabled", svc)
        enabled_out = enabled_out.strip()
        ec = state_col("enabled" if "enabled" in enabled_out else "disabled")
        WU(f"id:lbl_enabled;text:unit-file: {enabled_out};fg:{ec}")

    def do_action(action_fn, *args):
        """Run action_fn(*args), display result."""
        svc = selected_svc[0]
        if not svc:
            set_output("(no service selected)", "← Select a service from the list first.")
            return
        cmd_str, out = action_fn(svc, *args)
        set_output(cmd_str, out if out.strip() else "(command completed successfully)")
        show_selected_info()

    def reload_services(filter_str: str = ""):
        WU(f"id:lbl_summary;text:loading…;fg:{YEL}")
        services = list_services()
        all_services[0] = services
        apply_filter(filter_str, services)
        WU(f"id:lbl_summary;text:{service_summary(services)};fg:{DIM}")

    def apply_filter(flt: str, services=None):
        src = services if services is not None else all_services[0]
        flt = flt.strip().lower()
        if not flt:
            filtered = src
        else:
            filtered = [s for s in src
                        if flt in s[0].lower()     # unit name
                        or flt in s[2].lower()     # active state
                        or flt in s[3].lower()     # sub state
                        or flt in s[4].lower()]    # description
        populate_table(filtered)

    # ── Initial load ──────────────────────────────────────────────────────────
    reload_services()

    # ── Event loop ────────────────────────────────────────────────────────────
    while True:
        ev  = read_event_timeout(60)  # long wait — no background refresh needed
        if ev is None:
            continue
        eid = ev.get("id", "")
        act = ev.get("action", "")

        # ── Quit
        if eid == "btn_quit":
            break

        # ── Service selected in table
        elif eid == "svc_list" and act == "selected":
            # value = "shortname|active|sub|load"
            cols = ev.get("value", "").split("|")
            if cols:
                short = cols[0].strip()
                # Find full unit name in shown_services
                match = [s for s in shown_services[0] if s[0].replace(".service","") == short]
                if match:
                    selected_svc[0] = match[0][0]
                    show_selected_info()
                    # Auto-show status on selection
                    cmd_str, out = systemctl("status", "--lines=20", selected_svc[0])
                    set_output(cmd_str, out)

        # ── Read-only actions
        elif eid == "btn_status":
            do_action(lambda s: systemctl("status", "--lines=40", s))

        elif eid == "btn_journal":
            do_action(lambda s: journalctl(s, 80))

        elif eid == "btn_deps":
            do_action(lambda s: systemctl("list-dependencies", s))

        # ── Mutating actions
        elif eid == "btn_start":
            do_action(lambda s: systemctl_mut("start", s))
            reload_services()

        elif eid == "btn_stop":
            do_action(lambda s: systemctl_mut("stop", s))
            reload_services()

        elif eid == "btn_restart":
            do_action(lambda s: systemctl_mut("restart", s))
            reload_services()

        elif eid == "btn_reload":
            do_action(lambda s: systemctl_mut("reload", s))

        elif eid == "btn_enable":
            do_action(lambda s: systemctl_mut("enable", s))
            show_selected_info()

        elif eid == "btn_disable":
            do_action(lambda s: systemctl_mut("disable", s))
            show_selected_info()

        elif eid == "btn_mask":
            do_action(lambda s: systemctl_mut("mask", s))
            reload_services()

        elif eid == "btn_unmask":
            do_action(lambda s: systemctl_mut("unmask", s))
            reload_services()

        # ── Refresh
        elif eid == "btn_refresh":
            reload_services()

        # ── Quick filter buttons
        elif eid == "btn_flt_active":
            apply_filter("active")
        elif eid == "btn_flt_failed":
            apply_filter("failed")
        elif eid == "btn_flt_all":
            apply_filter("")

        # ── Search button → request filter entry value
        elif eid in ("btn_search", "btn_clear_flt"):
            if eid == "btn_clear_flt":
                WU("id:filter_entry;text:")
                apply_filter("")
            else:
                pending_filter[0] = True
                GV("filter_entry")

        # ── GV response for filter entry
        elif eid == "filter_entry" and act == "value":
            if pending_filter[0]:
                pending_filter[0] = False
                flt = ev.get("value", "")
                apply_filter(flt)

finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
    print()
