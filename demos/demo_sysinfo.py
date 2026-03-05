#!/usr/bin/env python3
"""
demo_sysinfo.py  –  Live system monitor using the OSC 1337 widget protocol.
Works on macOS and Linux with zero external dependencies.

Layout:
  ┌ title bar ──────────────────────────────────────────────────────┐
  │ CPU card  │  RAM card  │  Disk card  │  Uptime / Net / Load     │
  │ CPU history line graph              │  Top processes table      │
  └ info bar (kernel / arch / Python / CPUs / timestamp) ───────────┘
"""
import sys, os, tty, termios, time, platform, subprocess, select, re
from collections import deque

ESC, BEL = "\033", "\007"

def osc(cmd):
    sys.stdout.write(f"{ESC}]1337;{cmd}{BEL}")
    sys.stdout.flush()

def P(s):  osc(f"Panel={s}")
def W(s):  osc(f"Widget={s}")
def WU(s): osc(f"WidgetUpdate={s}")

# ── Colour palette ────────────────────────────────────────────────────────────
BG   = "#0f1117"
BG2  = "#1a1f2e"
BG3  = "#0d1021"
FG   = "#e2e8f0"
DIM  = "#4b5563"
GRN  = "#10b981"
RED  = "#ef4444"
YEL  = "#f59e0b"
BLU  = "#3b82f6"
PRP  = "#8b5cf6"
CYN  = "#06b6d4"
ORG  = "#f97316"

PLATFORM = platform.system()   # "Darwin" or "Linux"

# ── Non-blocking event reader ─────────────────────────────────────────────────
def read_event_timeout(timeout: float):
    """Return a WidgetEvent dict, or None on timeout. Raises SystemExit on Ctrl+C."""
    deadline = time.monotonic() + timeout
    buf = ""
    in_seq = False
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
                in_seq = True
                buf = ""
        else:
            if ch in (BEL, "\n"):
                in_seq = False
                if "WidgetEvent=" in buf:
                    d = {}
                    for p in buf.split("WidgetEvent=", 1)[1].split(";"):
                        if ":" in p:
                            k, v = p.split(":", 1)
                            d[k] = v
                    return d
                buf = ""  # discard non-widget OSC (e.g. window title changes)
            else:
                buf += ch

# ── Helpers ───────────────────────────────────────────────────────────────────
def fmt_rate(bps: float) -> str:
    for unit in ("B/s", "KB/s", "MB/s", "GB/s"):
        if abs(bps) < 1024:
            return f"{bps:5.1f} {unit}"
        bps /= 1024
    return f"{bps:.1f} TB/s"

def pct_color(pct: float) -> str:
    if pct < 60: return GRN
    if pct < 85: return YEL
    return RED

# ── System info collector ─────────────────────────────────────────────────────
class SysInfo:
    def __init__(self):
        self._prev_cpu_stat = None   # Linux: (idle, total)

    # ── CPU % ──────────────────────────────────────────────────────────────────
    def cpu_percent(self) -> float:
        if PLATFORM == "Linux":
            return self._cpu_linux()
        return self._cpu_darwin()

    def _cpu_linux(self) -> float:
        try:
            with open("/proc/stat") as f:
                parts = f.readline().split()
            user, nice, sys_, idle, iowait = (int(parts[i]) for i in range(1, 6))
            irq, softirq = int(parts[6]), int(parts[7])
            idle_total  = idle + iowait
            total       = user + nice + sys_ + idle + iowait + irq + softirq
            if self._prev_cpu_stat is None:
                self._prev_cpu_stat = (idle_total, total)
                return 0.0
            prev_idle, prev_total = self._prev_cpu_stat
            self._prev_cpu_stat  = (idle_total, total)
            dt = total - prev_total
            di = idle_total - prev_idle
            return max(0.0, 100.0 * (1.0 - di / dt)) if dt else 0.0
        except Exception:
            return 0.0

    def _cpu_darwin(self) -> float:
        try:
            # top -l 2 -s 0.5 -n 0: two snapshots 0.5 s apart, no processes
            out = subprocess.check_output(
                ["top", "-l", "2", "-s", "0.5", "-n", "0"],
                text=True, stderr=subprocess.DEVNULL
            )
            lines = [l for l in out.splitlines() if "CPU usage" in l]
            if lines:
                m = re.search(r"([\d.]+)%\s+idle", lines[-1])
                if m:
                    return min(100.0, max(0.0, 100.0 - float(m.group(1))))
        except Exception:
            pass
        return 0.0

    # ── Memory ─────────────────────────────────────────────────────────────────
    def memory(self):
        """Returns (used_gb, total_gb, pct)."""
        if PLATFORM == "Linux":
            return self._mem_linux()
        return self._mem_darwin()

    def _mem_linux(self):
        try:
            info = {}
            with open("/proc/meminfo") as f:
                for line in f:
                    k, v = line.split(":", 1)
                    info[k.strip()] = int(v.split()[0])  # kB
            total = info["MemTotal"]
            avail = info.get("MemAvailable", info.get("MemFree", 0))
            used  = total - avail
            pct   = 100.0 * used / total if total else 0.0
            return (used / 1_048_576, total / 1_048_576, pct)
        except Exception:
            return (0.0, 0.0, 0.0)

    def _mem_darwin(self):
        try:
            total_b = int(subprocess.check_output(
                ["sysctl", "-n", "hw.memsize"], text=True).strip())
            vm = subprocess.check_output(["vm_stat"], text=True)
            def pages(key):
                m = re.search(rf"{re.escape(key)}:\s+(\d+)", vm)
                return int(m.group(1)) if m else 0
            page = 4096
            avail = (pages("Pages free") + pages("Pages inactive")) * page
            used  = total_b - avail
            pct   = 100.0 * used / total_b if total_b else 0.0
            return (used / 1e9, total_b / 1e9, pct)
        except Exception:
            return (0.0, 0.0, 0.0)

    # ── Disk ───────────────────────────────────────────────────────────────────
    def disk(self, path="/"):
        try:
            st = os.statvfs(path)
            # POSIX: f_blocks is in units of f_frsize, not f_bsize
            blk   = st.f_frsize if st.f_frsize else st.f_bsize
            total = st.f_blocks * blk
            free  = st.f_bavail * blk
            used  = total - free
            pct   = 100.0 * used / total if total else 0.0
            return (used / 1e9, total / 1e9, pct)
        except Exception:
            return (0.0, 0.0, 0.0)

    # ── Uptime ─────────────────────────────────────────────────────────────────
    def uptime(self) -> str:
        try:
            if PLATFORM == "Linux":
                with open("/proc/uptime") as f:
                    secs = float(f.read().split()[0])
            else:
                out = subprocess.check_output(
                    ["sysctl", "-n", "kern.boottime"], text=True)
                m = re.search(r"sec\s*=\s*(\d+)", out)
                secs = time.time() - int(m.group(1)) if m else 0.0
            d = int(secs // 86400)
            h = int((secs % 86400) // 3600)
            mn = int((secs % 3600) // 60)
            return f"{d}d {h:02d}h {mn:02d}m" if d else f"{h:02d}h {mn:02d}m"
        except Exception:
            return "?"

    # ── Load average ───────────────────────────────────────────────────────────
    def load_avg(self) -> str:
        try:
            la = os.getloadavg()
            return f"{la[0]:.2f}  {la[1]:.2f}  {la[2]:.2f}"
        except Exception:
            return "—"

    # ── Network byte counters ──────────────────────────────────────────────────
    def net_bytes(self):
        """Returns (rx_total, tx_total) — cumulative counters."""
        if PLATFORM == "Linux":
            try:
                rx = tx = 0
                with open("/proc/net/dev") as f:
                    for line in f:
                        line = line.strip()
                        if ":" not in line:
                            continue
                        iface, data = line.split(":", 1)
                        if iface.strip() == "lo":
                            continue
                        cols = data.split()
                        rx += int(cols[0])
                        tx += int(cols[8])
                return (rx, tx)
            except Exception:
                return (0, 0)
        else:
            try:
                out = subprocess.check_output(
                    ["netstat", "-ib"], text=True, stderr=subprocess.DEVNULL)
                rx = tx = 0
                for line in out.splitlines()[1:]:
                    p = line.split()
                    if len(p) >= 10 and not p[0].startswith("lo"):
                        try:
                            rx += int(p[6])
                            tx += int(p[9])
                        except (ValueError, IndexError):
                            pass
                return (rx, tx)
            except Exception:
                return (0, 0)

    # ── Top processes ──────────────────────────────────────────────────────────
    def top_procs(self):
        """Returns list of (name, cpu_pct, mem_mb)."""
        try:
            if PLATFORM == "Linux":
                out = subprocess.check_output(
                    ["ps", "-eo", "comm,%cpu,rss", "--sort=-%cpu", "--no-headers"],
                    text=True, stderr=subprocess.DEVNULL)
            else:
                out = subprocess.check_output(
                    ["ps", "-eo", "comm,%cpu,rss", "-r"],
                    text=True, stderr=subprocess.DEVNULL)
                out = "\n".join(out.splitlines()[1:])  # skip header on macOS
            procs = []
            for line in out.splitlines()[:12]:
                p = line.split(None, 2)
                if len(p) == 3:
                    name, cpu, rss = p
                    procs.append((name[:22], float(cpu), int(rss) // 1024))
            return procs
        except Exception:
            return []


# ══════════════════════════════════════════════════════════════════════════════
# UI
# ══════════════════════════════════════════════════════════════════════════════
os.system("clear")
fd    = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)

REFRESH      = 2.0          # seconds between data updates
CPU_HIST     = deque([0.0] * 60, maxlen=60)   # ring buffer for line graph
MEM_HIST     = deque([0.0] * 60, maxlen=60)

try:
    si = SysInfo()

    # ── Root ──────────────────────────────────────────────────────────────────
    P(f"id:root;layout:vertical;spacing:8;margin:10;bg:{BG}")

    # ── Title bar ─────────────────────────────────────────────────────────────
    P(f"id:titlebar;panel:root;layout:horizontal;spacing:10")
    W(f"type:label;id:lbl_title;panel:titlebar;"
      f"text:🖥  System Monitor;fg:{FG};font:Sans Bold;size:15;hexpand:true")
    W(f"type:label;id:lbl_host;panel:titlebar;"
      f"text:{platform.node()};fg:{PRP};font:Monospace;size:11")
    W(f"type:label;id:lbl_plat;panel:titlebar;"
      f"text:{PLATFORM};fg:{DIM};size:11")
    W(f"type:button;id:btn_quit;panel:titlebar;"
      f"label:✕;bg:{RED};fg:#fff;tooltip:Quit")

    # ── Stat cards ────────────────────────────────────────────────────────────
    P(f"id:cards;panel:root;layout:horizontal;spacing:8")

    # CPU card
    P(f"id:card_cpu;panel:cards;layout:vertical;spacing:4;margin:12;bg:{BG2};width:190")
    W(f"type:label;id:lbl_cpu_h;panel:card_cpu;"
      f"text:⚡ CPU;fg:{CYN};font:Sans Bold;size:11")
    W(f"type:label;id:lbl_cpu_pct;panel:card_cpu;"
      f"text:—;fg:{FG};font:Monospace Bold;size:30")
    W(f"type:progressbar;id:bar_cpu;panel:card_cpu;"
      f"value:0;min:0;max:100;width:166;height:10;bg:{BG3};fg:{CYN}")
    W(f"type:label;id:lbl_load;panel:card_cpu;"
      f"text:load —;fg:{DIM};font:Monospace;size:10")

    # RAM card
    P(f"id:card_ram;panel:cards;layout:vertical;spacing:4;margin:12;bg:{BG2};width:190")
    W(f"type:label;id:lbl_ram_h;panel:card_ram;"
      f"text:🧠 Memory;fg:{GRN};font:Sans Bold;size:11")
    W(f"type:label;id:lbl_ram_pct;panel:card_ram;"
      f"text:—;fg:{FG};font:Monospace Bold;size:30")
    W(f"type:progressbar;id:bar_ram;panel:card_ram;"
      f"value:0;min:0;max:100;width:166;height:10;bg:{BG3};fg:{GRN}")
    W(f"type:label;id:lbl_ram_det;panel:card_ram;"
      f"text:— / — GB;fg:{DIM};font:Monospace;size:10")

    # Disk card
    P(f"id:card_disk;panel:cards;layout:vertical;spacing:4;margin:12;bg:{BG2};width:190")
    W(f"type:label;id:lbl_disk_h;panel:card_disk;"
      f"text:💾 Disk  /;fg:{YEL};font:Sans Bold;size:11")
    W(f"type:label;id:lbl_disk_pct;panel:card_disk;"
      f"text:—;fg:{FG};font:Monospace Bold;size:30")
    W(f"type:progressbar;id:bar_disk;panel:card_disk;"
      f"value:0;min:0;max:100;width:166;height:10;bg:{BG3};fg:{YEL}")
    W(f"type:label;id:lbl_disk_det;panel:card_disk;"
      f"text:— / — GB;fg:{DIM};font:Monospace;size:10")

    # Uptime / Net / Load card
    P(f"id:card_up;panel:cards;layout:vertical;spacing:6;margin:12;bg:{BG2};hexpand:true")
    W(f"type:label;id:lbl_up_h;panel:card_up;"
      f"text:⏱ Uptime;fg:{PRP};font:Sans Bold;size:11")
    W(f"type:label;id:lbl_uptime;panel:card_up;"
      f"text:—;fg:{FG};font:Monospace Bold;size:20")
    W(f"type:label;id:lbl_net_h;panel:card_up;"
      f"text:🌐 Network;fg:{ORG};font:Sans Bold;size:11")
    W(f"type:label;id:lbl_net;panel:card_up;"
      f"text:↓ —    ↑ —;fg:{FG};font:Monospace;size:12")

    # ── Middle row ────────────────────────────────────────────────────────────
    P(f"id:middle;panel:root;layout:horizontal;spacing:8")

    # Graphs sub-panel (stacked CPU + MEM sparklines)
    P(f"id:pnl_graphs;panel:middle;layout:vertical;spacing:6;margin:10;bg:{BG2};hexpand:true")
    W(f"type:label;id:lbl_gh;panel:pnl_graphs;"
      f"text:CPU % — last 60 samples;fg:{CYN};font:Sans Bold;size:11")
    W(f"type:graph;id:cpu_graph;panel:pnl_graphs;"
      f"kind:line;title:;width:460;height:130;bg:{BG3};"
      f"colors:{CYN};data:0")
    W(f"type:label;id:lbl_mh;panel:pnl_graphs;"
      f"text:Memory % — last 60 samples;fg:{GRN};font:Sans Bold;size:11")
    W(f"type:graph;id:mem_graph;panel:pnl_graphs;"
      f"kind:line;title:;width:460;height:100;bg:{BG3};"
      f"colors:{GRN};data:0")

    # Process table
    P(f"id:pnl_procs;panel:middle;layout:vertical;spacing:4;margin:10;bg:{BG2};width:310")
    W(f"type:label;id:lbl_procs_h;panel:pnl_procs;"
      f"text:🔥 Top Processes;fg:{ORG};font:Sans Bold;size:11")
    W(f"type:table;id:tbl_procs;panel:pnl_procs;"
      f"cols:Process|CPU%|MB;"
      f"widths:160|65|65;"
      f"sortable:false;"
      f"width:290;height:260")

    # ── Info bar ──────────────────────────────────────────────────────────────
    P(f"id:infobar;panel:root;layout:horizontal;spacing:16;margin:6;bg:{BG2}")
    W(f"type:label;id:lbl_kern;panel:infobar;"
      f"text:kernel {platform.release()};fg:{DIM};font:Monospace;size:10")
    W(f"type:label;id:lbl_arch;panel:infobar;"
      f"text:{platform.machine()};fg:{DIM};font:Monospace;size:10")
    W(f"type:label;id:lbl_py;panel:infobar;"
      f"text:Python {platform.python_version()};fg:{DIM};font:Monospace;size:10")
    W(f"type:label;id:lbl_cpus;panel:infobar;"
      f"text:{os.cpu_count()} CPUs;fg:{DIM};font:Monospace;size:10")
    W(f"type:label;id:lbl_ts;panel:infobar;"
      f"text:starting…;fg:{DIM};font:Monospace;size:10;hexpand:true")

    # ── State for rate calculations ───────────────────────────────────────────
    prev_net      = [si.net_bytes()]
    prev_net_time = [time.monotonic()]
    last_refresh  = [0.0]  # force immediate first refresh

    # ── Refresh function ──────────────────────────────────────────────────────
    def refresh():
        cpu  = si.cpu_percent()
        mem  = si.memory()
        disk = si.disk("/")
        up   = si.uptime()
        load = si.load_avg()
        net  = si.net_bytes()
        now  = time.monotonic()
        dt   = max(0.1, now - prev_net_time[0])
        rx_s = max(0.0, (net[0] - prev_net[0][0]) / dt)
        tx_s = max(0.0, (net[1] - prev_net[0][1]) / dt)
        prev_net[0]      = net
        prev_net_time[0] = now

        CPU_HIST.append(cpu)
        MEM_HIST.append(mem[2])

        cpu_c  = pct_color(cpu)
        ram_c  = pct_color(mem[2])
        disk_c = pct_color(disk[2])

        # CPU card
        WU(f"id:lbl_cpu_pct;text:{cpu:.0f}%;fg:{cpu_c}")
        WU(f"id:bar_cpu;value:{cpu:.0f};fg:{cpu_c}")
        WU(f"id:lbl_load;text:load {load}")

        # RAM card
        WU(f"id:lbl_ram_pct;text:{mem[2]:.0f}%;fg:{ram_c}")
        WU(f"id:bar_ram;value:{mem[2]:.0f};fg:{ram_c}")
        WU(f"id:lbl_ram_det;text:{mem[0]:.1f} / {mem[1]:.1f} GB")

        # Disk card
        WU(f"id:lbl_disk_pct;text:{disk[2]:.0f}%;fg:{disk_c}")
        WU(f"id:bar_disk;value:{disk[2]:.0f};fg:{disk_c}")
        WU(f"id:lbl_disk_det;text:{disk[0]:.0f} / {disk[1]:.0f} GB")

        # Uptime / net
        WU(f"id:lbl_uptime;text:{up}")
        WU(f"id:lbl_net;text:↓ {fmt_rate(rx_s)}  ↑ {fmt_rate(tx_s)}")

        # Graphs — pipe-separated values (single series per graph)
        cpu_pts = "|".join(f"{v:.1f}" for v in CPU_HIST)
        mem_pts = "|".join(f"{v:.1f}" for v in MEM_HIST)
        WU(f"id:cpu_graph;data:{cpu_pts};colors:{cpu_c}")
        WU(f"id:mem_graph;data:{mem_pts};colors:{ram_c}")

        # Process table
        procs = si.top_procs()
        WU("id:tbl_procs;action:clear")
        for name, cpu_p, mem_mb in procs[:12]:
            c = pct_color(cpu_p)
            WU(f"id:tbl_procs;action:addrow;"
               f"cols:{name}|{cpu_p:.1f}|{mem_mb};fg:{c}")

        WU(f"id:lbl_ts;text:updated {time.strftime('%H:%M:%S')};fg:{DIM}")

        last_refresh[0] = time.monotonic()

    # ── Warm up CPU sampler (Linux needs two /proc/stat reads) ─────────────────
    si.cpu_percent()   # first read — seeds _prev_cpu_stat
    refresh()          # immediate first paint (CPU will show 0 the first time)

    # ── Main loop ─────────────────────────────────────────────────────────────
    while True:
        wait = max(0.05, REFRESH - (time.monotonic() - last_refresh[0]))
        ev   = read_event_timeout(wait)

        if ev is None:
            refresh()
            continue

        if ev.get("id") == "btn_quit":
            break

finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
    print()
