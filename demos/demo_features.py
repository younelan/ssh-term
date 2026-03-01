#!/usr/bin/env python3
"""
demo_features.py  –  Showcase: progressbar, badges, table sorting,
                      row colours, OpenURL=, Notify=
"""
import sys, os, tty, termios, threading, time

ESC, BEL = "\033", "\007"

def osc(cmd):
    sys.stdout.write(f"{ESC}]1337;{cmd}{BEL}")
    sys.stdout.flush()

def P(s):  osc(f"Panel={s}")
def W(s):  osc(f"Widget={s}")
def WU(s): osc(f"WidgetUpdate={s}")

# ── Raw event reader ──────────────────────────────────────────────────────────
def read_event():
    """Block until the next OSC 1337 WidgetEvent; return dict of key:value pairs."""
    while True:
        ch = sys.stdin.read(1)
        if ch != ESC:
            continue
        seq = ""
        while True:
            c = sys.stdin.read(1)
            if c == BEL or c == "\n":
                break
            seq += c
        if "WidgetEvent=" not in seq:
            continue
        payload = seq.split("WidgetEvent=", 1)[1]
        d = {}
        for p in payload.split(";"):
            if ":" in p:
                k, v = p.split(":", 1)
                d[k] = v
        return d

# ── Colour palette ────────────────────────────────────────────────────────────
BG  = "#1a1e2e"
BG2 = "#252a3a"
BG3 = "#2e3450"
FG  = "#e2e8f0"
DIM = "#94a3b8"
ACC = "#7c3aed"
GRN = "#16a34a"
RED = "#dc2626"
BLU = "#2563eb"
YEL = "#d97706"

os.system("clear")
fd = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)

try:
    # ── Root ─────────────────────────────────────────────────────────────────
    P(f"id:root;layout:vertical;spacing:12;margin:12;width:520;bg_color:{BG}")

    # ── Section: Title + Badges ───────────────────────────────────────────────
    P(f"id:s_head;panel:root;layout:horizontal;spacing:8;margin:0;bg_color:{BG}")
    W(f"type:label;id:hdr;label:Feature Demo;panel:s_head;fg_color:{FG}")
    W(f"type:badge;id:bdg_new;text:NEW;color:{GRN};fg:#fff;panel:s_head")
    W(f"type:badge;id:bdg_cnt;text:0 alerts;color:{RED};fg:#fff;panel:s_head")
    W(f"type:badge;id:bdg_ver;text:v2.0;color:{BLU};fg:#fff;panel:s_head")

    # ── Section: Progress bars ────────────────────────────────────────────────
    P(f"id:s_pb;panel:root;layout:vertical;spacing:6;margin:0;title:Progress Bars;bg_color:{BG2}")
    W(f"type:progressbar;id:pb1;value:30;text:Download 30%;width:460;panel:s_pb")
    W(f"type:progressbar;id:pb2;value:65;text:Upload 65%;width:460;panel:s_pb")
    P(f"id:pb_btns;panel:s_pb;layout:horizontal;spacing:6;margin:0;bg_color:{BG2}")
    W(f"type:button;id:btn_pb_inc;label:+10%;panel:pb_btns")
    W(f"type:button;id:btn_pb_reset;label:Reset;panel:pb_btns")
    W(f"type:button;id:btn_pb_anim;label:Animate;panel:pb_btns")

    # ── Section: Sortable table with row colours ──────────────────────────────
    P(f"id:s_tbl;panel:root;layout:vertical;spacing:4;margin:0;title:Sortable Table – click column headers;bg_color:{BG2}")
    W(f"type:table;id:tbl;cols:Name|Status|Score;widths:200|110|80;height:160;sortable:true;panel:s_tbl")

    ROWS = [
        ("Alice",   "Active",   "92", "#ffffff", GRN),
        ("Bob",     "Inactive", "45", "#ffffff", "#5c1f1f"),
        ("Charlie", "Active",   "78", FG,        ""),
        ("Diana",   "Pending",  "61", DIM,       BG3),
        ("Eve",     "Active",   "99", "#ffffff", "#1a3a5c"),
    ]

    def populate(rows=None):
        rows = rows or ROWS
        WU("id:tbl;action:clear")
        for name, status, score, fg, bg in rows:
            extra = ""
            if fg: extra += f";fg:{fg}"
            if bg: extra += f";bg:{bg}"
            WU(f"id:tbl;action:addrow;cols:{name}|{status}|{score}{extra}")

    populate()

    P(f"id:tbl_btns;panel:s_tbl;layout:horizontal;spacing:6;margin:0;bg_color:{BG2}")
    W(f"type:button;id:btn_recolor;label:Highlight row 1;panel:tbl_btns")
    W(f"type:button;id:btn_clear_row;label:Clear highlight;panel:tbl_btns")

    # ── Section: OS integration ───────────────────────────────────────────────
    P(f"id:s_os;panel:root;layout:horizontal;spacing:8;margin:0;title:OS Integration;bg_color:{BG2}")
    W(f"type:button;id:btn_notify;label:Send Notification;panel:s_os")
    W(f"type:button;id:btn_url;label:Open GTK Docs;panel:s_os")

    # ── Section: Badge counter ────────────────────────────────────────────────
    P(f"id:s_badge;panel:root;layout:horizontal;spacing:8;margin:0;title:Badge Demo;bg_color:{BG2}")
    W(f"type:label;id:lbl_cnt;label:Alert count:;panel:s_badge;fg_color:{DIM}")
    W(f"type:badge;id:bdg_live;text:0;color:{ACC};fg:#fff;panel:s_badge")
    W(f"type:button;id:btn_badge_inc;label:+1;panel:s_badge")
    W(f"type:button;id:btn_badge_reset;label:Reset;panel:s_badge")

    # ── State ─────────────────────────────────────────────────────────────────
    pb  = [30, 65]
    cnt = [0]

    def animate():
        for v in range(0, 101, 4):
            time.sleep(0.06)
            WU(f"id:pb1;value:{v};text:Download {v}%")
        osc("Toast=msg:Download complete!;duration:2500")

    osc("Toast=msg:Demo ready – click any control;duration:2000")

    # ── Event loop ────────────────────────────────────────────────────────────
    while True:
        ev     = read_event()
        wid    = ev.get("id", "")
        action = ev.get("action", "")

        if wid == "btn_pb_inc":
            pb[0] = min(100, pb[0] + 10)
            pb[1] = min(100, pb[1] + 10)
            WU(f"id:pb1;value:{pb[0]};text:Download {pb[0]}%")
            WU(f"id:pb2;value:{pb[1]};text:Upload {pb[1]}%")

        elif wid == "btn_pb_reset":
            pb[0], pb[1] = 0, 0
            WU("id:pb1;value:0;text:Download 0%")
            WU("id:pb2;value:0;text:Upload 0%")

        elif wid == "btn_pb_anim":
            threading.Thread(target=animate, daemon=True).start()

        elif wid == "tbl" and action == "sort":
            col = ev.get("col", "")
            asc = ev.get("dir", "asc") == "asc"
            idx = {"Name": 0, "Status": 1, "Score": 2}.get(col, 0)
            key = (lambda r: int(r[idx])) if col == "Score" else (lambda r: r[idx])
            populate(sorted(ROWS, key=key, reverse=not asc))
            osc(f"Toast=msg:Sorted by {col} {'up' if asc else 'down'};duration:1500")

        elif wid == "tbl" and action == "selected":
            name = ev.get("value", "").split("|")[0]
            osc(f"Toast=msg:Selected: {name};duration:1200")

        elif wid == "btn_recolor":
            WU(f"id:tbl;action:rowcolor;row:1;bg:{YEL};fg:#000")

        elif wid == "btn_clear_row":
            WU("id:tbl;action:rowcolor;row:1;bg:;fg:")

        elif wid == "btn_notify":
            osc("Notify=title:Feature Demo;body:Hello from the terminal GUI!;id:demo1")
            osc("Toast=msg:Notification sent;duration:1500")

        elif wid == "btn_url":
            osc("OpenURL=https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/")
            osc("Toast=msg:Opening browser...;duration:1200")

        elif wid == "btn_badge_inc":
            cnt[0] += 1
            WU(f"id:bdg_live;text:{cnt[0]}")
            WU(f"id:bdg_cnt;text:{cnt[0]} alerts")

        elif wid == "btn_badge_reset":
            cnt[0] = 0
            WU("id:bdg_live;text:0")
            WU("id:bdg_cnt;text:0 alerts")

finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
