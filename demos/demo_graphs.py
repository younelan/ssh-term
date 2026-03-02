#!/usr/bin/env python3
"""
demo_graphs.py  –  Graph widget demo: bar, line, pie charts
                   with live updates and controls
"""
import sys, os, tty, termios, threading, time, math, random

ESC, BEL = "\033", "\007"

def osc(cmd):
    sys.stdout.write(f"{ESC}]1337;{cmd}{BEL}")
    sys.stdout.flush()

def P(s):  osc(f"Panel={s}")
def W(s):  osc(f"Widget={s}")
def WU(s): osc(f"WidgetUpdate={s}")

def read_event():
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

# ── Palette ───────────────────────────────────────────────────────────────────
BG  = "#0f172a"
BG2 = "#1e293b"
BG3 = "#253047"
FG  = "#e2e8f0"
DIM = "#64748b"

# ── Data sets ─────────────────────────────────────────────────────────────────
MONTHS = "Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec"

sales_data   = [42, 58, 35, 72, 88, 63, 95, 81, 67, 74, 90, 105]
traffic_data = [120, 145, 98, 167, 203, 178, 215, 198, 189, 220, 245, 280]
expenses     = [38, 52, 44, 61, 75, 58, 82, 70, 65, 68, 80, 92]

market_share = [38, 27, 18, 10, 7]
market_names = "Product A|Product B|Product C|Product D|Other"

os.system("clear")
fd = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)

def fmt_series(data):
    return "|".join(str(v) for v in data)

def fmt_multi(d1, d2):
    return f"{fmt_series(d1)},{fmt_series(d2)}"

try:
    # ── Root ─────────────────────────────────────────────────────────────────
    P(f"id:root;layout:vertical;spacing:10;margin:10;width:680;bg_color:{BG}")

    # ── Title bar ─────────────────────────────────────────────────────────────
    P(f"id:titlebar;panel:root;layout:horizontal;spacing:8;margin:0;bg_color:{BG}")
    W(f"type:label;id:title;label:Graph Widget Demo;panel:titlebar;fg_color:{FG}")
    W(f"type:badge;id:bdg;text:Live;color:#16a34a;fg:#fff;panel:titlebar")

    # ── Row 1: bar + pie ──────────────────────────────────────────────────────
    P(f"id:row1;panel:root;layout:horizontal;spacing:10;margin:0;bg_color:{BG}")

    # Bar chart – monthly sales
    W(f"type:graph;id:bar1;kind:bar;width:380;height:220;"
      f"title:Monthly Sales (units);"
      f"data:{fmt_series(sales_data)};"
      f"labels:{MONTHS};"
      f"colors:#5b8dee|#5b8dee|#5b8dee|#5b8dee|#5b8dee|#5b8dee|"
             f"#5b8dee|#5b8dee|#5b8dee|#5b8dee|#5b8dee|#fcc419;"
      f"bg:{BG2};panel:row1")

    # Pie chart – market share
    W(f"type:graph;id:pie1;kind:pie;width:280;height:220;"
      f"title:Market Share;"
      f"data:{fmt_series(market_share)};"
      f"labels:{market_names};"
      f"bg:{BG2};panel:row1")

    # ── Row 2: line chart (multi-series) ──────────────────────────────────────
    W(f"type:graph;id:line1;kind:line;width:660;height:200;"
      f"title:Sales vs Expenses (monthly);"
      f"data:{fmt_multi(sales_data, expenses)};"
      f"labels:{MONTHS};"
      f"series:Sales|Expenses;"
      f"colors:#5b8dee|#ff6b6b;"
      f"bg:{BG2};panel:root")

    # ── Controls ──────────────────────────────────────────────────────────────
    P(f"id:controls;panel:root;layout:horizontal;spacing:8;margin:0;bg_color:{BG}")
    W(f"type:button;id:btn_bar;label:Bar;panel:controls")
    W(f"type:button;id:btn_line;label:Line;panel:controls")
    W(f"type:button;id:btn_pie;label:Pie;panel:controls")
    W(f"type:button;id:btn_rand;label:Randomise;panel:controls")
    W(f"type:button;id:btn_anim;label:Animate;panel:controls")
    W(f"type:label;id:lbl_hint;label: ← chart1 switches kind;panel:controls;fg_color:{DIM}")

    osc("Toast=msg:Graphs ready – use controls to explore;duration:2500")

    # ── Animate thread ────────────────────────────────────────────────────────
    animating = [False]

    def do_animate():
        step = 0
        while animating[0]:
            phase = step * 0.3
            animated = [int(50 + 45 * math.sin(phase + i * 0.6)) for i in range(12)]
            animated2 = [int(30 + 25 * math.cos(phase + i * 0.5)) for i in range(12)]
            WU(f"id:bar1;data:{fmt_series(animated)}")
            WU(f"id:line1;data:{fmt_multi(animated, animated2)}")
            # Wiggle pie slices
            total = 100
            slices = []
            for i in range(4):
                v = int(15 + 20 * abs(math.sin(phase + i * 1.2)))
                slices.append(v)
                total -= v
            slices.append(max(5, total))
            WU(f"id:pie1;data:{fmt_series(slices)}")
            time.sleep(0.1)
            step += 1
        # Restore
        WU(f"id:bar1;data:{fmt_series(sales_data)}")
        WU(f"id:line1;data:{fmt_multi(sales_data, expenses)}")
        WU(f"id:pie1;data:{fmt_series(market_share)}")

    current_kind = ["bar"]

    # ── Event loop ────────────────────────────────────────────────────────────
    while True:
        ev  = read_event()
        wid = ev.get("id", "")

        if wid == "btn_bar":
            animating[0] = False
            current_kind[0] = "bar"
            WU("id:bar1;kind:bar")

        elif wid == "btn_line":
            animating[0] = False
            current_kind[0] = "line"
            WU("id:bar1;kind:line")

        elif wid == "btn_pie":
            animating[0] = False
            current_kind[0] = "pie"
            WU("id:bar1;kind:pie")

        elif wid == "btn_rand":
            animating[0] = False
            rand_sales = [random.randint(20, 120) for _ in range(12)]
            rand_exp   = [random.randint(10, 80)  for _ in range(12)]
            rand_pie   = sorted([random.randint(5, 40) for _ in range(5)], reverse=True)
            WU(f"id:bar1;data:{fmt_series(rand_sales)}")
            WU(f"id:line1;data:{fmt_multi(rand_sales, rand_exp)}")
            WU(f"id:pie1;data:{fmt_series(rand_pie)}")
            osc("Toast=msg:Randomised!;duration:1000")

        elif wid == "btn_anim":
            if animating[0]:
                animating[0] = False
                WU("id:bdg;text:Paused")
            else:
                animating[0] = True
                WU("id:bdg;text:Animating")
                threading.Thread(target=do_animate, daemon=True).start()

finally:
    animating[0] = False
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
