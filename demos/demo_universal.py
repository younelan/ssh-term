#!/usr/bin/env python3
"""
demo_universal.py  –  Universal widget props: hide/show, enable/disable,
                       tooltip, font/size, foreground/background color.
New widgets: spinner, colorpicker, flowbox, scrollarea, FileDialog.
"""
import sys, os, tty, termios, threading, time

ESC, BEL = "\033", "\007"

def osc(cmd):
    sys.stdout.write(f"{ESC}]1337;{cmd}{BEL}")
    sys.stdout.flush()

def P(s):  osc(f"Panel={s}")
def W(s):  osc(f"Widget={s}")
def WU(s): osc(f"WidgetUpdate={s}")

# ── Palette ───────────────────────────────────────────────────────────────────
BG  = "#1a1e2e"
BG2 = "#252a3a"
BG3 = "#2e3450"
FG  = "#e2e8f0"
DIM = "#94a3b8"
ACC = "#7c3aed"
GRN = "#10b981"
RED = "#ef4444"
YEL = "#f59e0b"
BLU = "#3b82f6"

# ── Raw reader ────────────────────────────────────────────────────────────────
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

os.system("clear")
fd = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)

try:
    # ── Root: title bar + 2-column body ───────────────────────────────────────
    P(f"id:root;layout:vertical;spacing:8;margin:10;width:700;bg:{BG}")

    # Title + quit on same row
    P(f"id:row_title;panel:root;layout:horizontal;spacing:10")
    W(f"type:label;id:title;panel:row_title;text:Universal Widget Features;"
      f"font:Sans Bold;size:16;fg:{FG};hexpand:true")
    W(f"type:button;id:btn_quit;panel:row_title;label:✕ Quit;"
      f"bg:{RED};fg:#fff;tooltip:Exit the demo")

    # 2-column body
    P(f"id:cols;panel:root;layout:horizontal;spacing:12")
    P(f"id:col_left;panel:cols;layout:vertical;spacing:8;hexpand:true")
    P(f"id:col_right;panel:cols;layout:vertical;spacing:8;hexpand:true")

    # ── LEFT: Spinner ─────────────────────────────────────────────────────────
    P(f"id:row_spin;panel:col_left;layout:horizontal;spacing:6")
    W(f"type:label;id:lbl_spin;panel:row_spin;text:Spinner:;fg:{DIM};size:12")
    W(f"type:spinner;id:spin1;panel:row_spin;spinning:true;size:24;"
      f"tooltip:Loading spinner")
    W(f"type:button;id:btn_stop;panel:row_spin;label:■;bg:{RED};fg:#fff;"
      f"tooltip:Stop spinner")
    W(f"type:button;id:btn_start;panel:row_spin;label:▶;bg:{GRN};fg:#fff;"
      f"tooltip:Start spinner")
    W(f"type:button;id:btn_hide;panel:row_spin;label:Hide;"
      f"tooltip:Hide the spinner")
    W(f"type:button;id:btn_show;panel:row_spin;label:Show;"
      f"tooltip:Show the spinner")
    W(f"type:button;id:btn_dis;panel:row_spin;label:Disable;"
      f"tooltip:Disable the Stop button")
    W(f"type:button;id:btn_ena;panel:row_spin;label:Enable;"
      f"tooltip:Re-enable the Stop button")

    # ── LEFT: Color picker ────────────────────────────────────────────────────
    P(f"id:row_cp;panel:col_left;layout:horizontal;spacing:8")
    W(f"type:label;id:lbl_cp;panel:row_cp;text:Color:;fg:{DIM};size:12")
    W(f"type:colorpicker;id:picker1;panel:row_cp;value:#7c3aed;alpha:false;"
      f"tooltip:Pick any color")
    W(f"type:label;id:color_out;panel:row_cp;text:#7c3aed;fg:{ACC};"
      f"font:Monospace;size:13;tooltip:Chosen hex value")

    # ── LEFT: Font / size ─────────────────────────────────────────────────────
    P(f"id:row_font;panel:col_left;layout:horizontal;spacing:6")
    W(f"type:label;id:lbl_font;panel:row_font;text:Font:;fg:{DIM};size:12")
    W(f"type:label;id:sample_text;panel:row_font;"
      f"text:foxΩ∞;font:Sans;size:14;fg:{FG};hexpand:true;"
      f"tooltip:Font preview")
    W(f"type:button;id:btn_mono;panel:row_font;label:Mono;"
      f"tooltip:Monospace")
    W(f"type:button;id:btn_serif;panel:row_font;label:Serif;"
      f"tooltip:Serif")
    W(f"type:button;id:btn_big;panel:row_font;label:A+;"
      f"tooltip:Increase size")
    W(f"type:button;id:btn_small;panel:row_font;label:A-;"
      f"tooltip:Decrease size")

    # ── LEFT: File browser trigger ────────────────────────────────────────────
    P(f"id:row_fd;panel:col_left;layout:horizontal;spacing:8")
    W(f"type:label;id:lbl_fd;panel:row_fd;text:File:;fg:{DIM};size:12")
    W(f"type:button;id:btn_browse;panel:row_fd;label:📁 Browse Remote;"
      f"bg:{BLU};fg:#fff;tooltip:Open remote file browser")
    W(f"type:label;id:fd_result;panel:row_fd;text:(none selected);fg:{DIM};"
      f"font:Monospace;size:11;hexpand:true;tooltip:Selected file path")

    # ── RIGHT: Flow box ───────────────────────────────────────────────────────
    P(f"id:row_fb_hdr;panel:col_right;layout:horizontal;spacing:8")
    W(f"type:label;id:lbl_fb;panel:row_fb_hdr;text:FlowBox:;fg:{DIM};size:12;hexpand:true")
    W(f"type:button;id:btn_add_item;panel:row_fb_hdr;label:+ Add;"
      f"tooltip:Add a chip")
    W(f"type:button;id:btn_clear_fb;panel:row_fb_hdr;label:Clear;"
      f"tooltip:Hide all chips")

    W(f"type:flowbox;id:fb1;panel:col_right;max_cols:5;col_spacing:6;row_spacing:6;"
      f"height:70;bg:{BG2};tooltip:Items wrap here")
    for i, (lbl, col) in enumerate([
        ("Python", ACC), ("Rust", RED), ("Go", BLU), ("Bash", YEL),
        ("Ruby", "#e11d48"), ("Julia", GRN),
    ]):
        W(f"type:badge;id:chip_{i};panel:fb1;text:{lbl};bg:{col};fg:#fff")

    # ── RIGHT: Scroll area ────────────────────────────────────────────────────
    W(f"type:label;id:lbl_sa;panel:col_right;text:ScrollArea:;fg:{DIM};size:12")
    W(f"type:scrollarea;id:scroll1;panel:col_right;height:120;"
      f"hscroll:never;vscroll:automatic;bg:{BG2};"
      f"tooltip:Vertical scroll area")
    for n in range(1, 16):
        W(f"type:label;id:sitem_{n};panel:scroll1;"
          f"text:{n}. Scrollable line {n};fg:{FG};size:12")

    # ═══════════════════════════════════════════════════════════════════════════
    # Remote File Browser  (hidden until Browse Remote is clicked)
    # Pattern: script owns the filesystem; browser is just widgets.
    # Same approach works over SSH with paramiko SFTP — replace VDIR with
    # sftp.listdir_attr(path) and sort folders first.
    # ═══════════════════════════════════════════════════════════════════════════
    P(f"id:fbrow;panel:root;layout:vertical;spacing:4;bg:{BG3};"
      f"visible:false")

    # Path bar
    P(f"id:fbrow_bar;panel:fbrow;layout:horizontal;spacing:8")
    W(f"type:button;id:fbrow_up;panel:fbrow_bar;label:↑ Up;"
      f"tooltip:Go to parent directory")
    W(f"type:label;id:fbrow_path;panel:fbrow_bar;text:/;fg:{FG};"
      f"font:Monospace;size:12;hexpand:true")
    W(f"type:button;id:fbrow_close;panel:fbrow_bar;label:✕;"
      f"tooltip:Close browser")

    # File list (scrolled)
    W(f"type:scrollarea;id:fbrow_scroll;panel:fbrow;height:160;"
      f"hscroll:never;vscroll:automatic;bg:{BG2}")

    # Status bar
    W(f"type:label;id:fbrow_status;panel:fbrow;text:Click a file to select it;"
      f"fg:{DIM};size:11;font:Monospace")

    # ── State ─────────────────────────────────────────────────────────────────
    font_size = [14]
    chip_counter = [len(["Python","Rust","Go","Bash","Ruby","Julia"])]
    CHIP_COLORS = [ACC, RED, BLU, YEL, GRN, "#e11d48", "#06b6d4", "#f97316"]

    # ── Simulated remote filesystem ───────────────────────────────────────────
    # In a real SSH script: replace with sftp.listdir_attr(path)
    VDIR = {
        "/": ["projects/", "docs/", "readme.md", "config.py", ".bashrc"],
        "/projects": ["terminal/", "webapp/", "Makefile", "notes.txt"],
        "/projects/terminal": ["src/", "Cargo.toml", "README.md"],
        "/projects/terminal/src": ["main.rs", "ssh.rs", "terminal_state.rs"],
        "/projects/webapp": ["index.html", "app.js", "style.css"],
        "/docs": ["design.pdf", "api.md", "changelog.txt"],
    }
    browser_path   = ["/"]
    browser_max    = [0]   # total entry slots ever created

    def browser_render(path):
        """Render a listing into fbrow_scroll, reusing existing entry slots."""
        key     = path.rstrip("/") or "/"
        entries = VDIR.get(key, ["(empty)"])

        WU(f"id:fbrow_path;text:{path}")

        for i, name in enumerate(entries):
            is_dir = name.endswith("/")
            icon   = "📁" if is_dir else "📄"
            color  = YEL if is_dir else FG
            eid    = f"fbrow_e_{i}"
            if i < browser_max[0]:
                # Slot exists — update label, colour, visibility
                WU(f"id:{eid};label:{icon} {name};fg:{color};"
                   f"tooltip:{'Open folder' if is_dir else 'Select file'}: {name};"
                   f"visible:true")
            else:
                # New slot needed
                W(f"type:button;id:{eid};panel:fbrow_scroll;"
                  f"label:{icon} {name};fg:{color};bg:{BG2};"
                  f"tooltip:{'Open folder' if is_dir else 'Select file'}: {name}")
                browser_max[0] += 1

        # Hide leftover slots from a previous longer listing
        for i in range(len(entries), browser_max[0]):
            WU(f"id:fbrow_e_{i};visible:false")

    # ── Event loop ────────────────────────────────────────────────────────────
    while True:
        ev = read_event()
        eid = ev.get("id", "")

        # Spinner controls
        if eid == "btn_stop":
            WU("id:spin1;spinning:false")
        elif eid == "btn_start":
            WU("id:spin1;spinning:true")
        elif eid == "btn_hide":
            WU("id:spin1;visible:false")
        elif eid == "btn_show":
            WU("id:spin1;visible:true")
        elif eid == "btn_dis":
            WU("id:btn_stop;enabled:false")
        elif eid == "btn_ena":
            WU("id:btn_stop;enabled:true")

        # Color picker
        elif eid == "picker1":
            hex_val = ev.get("value", "?")
            WU(f"id:color_out;text:{hex_val}")
            WU(f"id:color_out;fg:{hex_val}")

        # Font / size
        elif eid == "btn_mono":
            WU("id:sample_text;font:Monospace")
        elif eid == "btn_serif":
            WU("id:sample_text;font:Georgia")
        elif eid == "btn_big":
            font_size[0] = min(32, font_size[0] + 2)
            WU(f"id:sample_text;size:{font_size[0]}")
        elif eid == "btn_small":
            font_size[0] = max(8, font_size[0] - 2)
            WU(f"id:sample_text;size:{font_size[0]}")

        # Flow box
        elif eid == "btn_add_item":
            n = chip_counter[0]
            col = CHIP_COLORS[n % len(CHIP_COLORS)]
            W(f"type:badge;id:chip_dyn_{n};panel:fb1;text:Item {n};bg:{col};fg:#fff")
            chip_counter[0] += 1
        elif eid == "btn_clear_fb":
            # Hide all chips (clear them visually)
            for i in range(chip_counter[0]):
                WU(f"id:chip_{i};visible:false")
                WU(f"id:chip_dyn_{i};visible:false")

        # File browser – open
        elif eid == "btn_browse":
            WU("id:fbrow;visible:true")
            browser_path[0] = "/"
            browser_render("/")

        # File browser – up
        elif eid == "fbrow_up":
            cur = browser_path[0].rstrip("/") or "/"
            parent = "/".join(cur.split("/")[:-1]) or "/"
            browser_path[0] = parent
            browser_render(parent)

        # File browser – close
        elif eid == "fbrow_close":
            WU("id:fbrow;visible:false")

        # File browser – entry clicked
        elif eid.startswith("fbrow_e_"):
            idx  = int(eid.split("_")[-1])
            key  = browser_path[0].rstrip("/") or "/"
            name = VDIR.get(key, [])[idx] if idx < len(VDIR.get(key, [])) else ""
            if name.endswith("/"):
                # Navigate into folder — this is the event the real remote
                # script would receive to fetch sftp.listdir_attr(new_path)
                new_path = (browser_path[0].rstrip("/") + "/" + name.rstrip("/"))
                new_path = new_path if new_path.startswith("/") else "/" + new_path
                browser_path[0] = new_path
                browser_render(new_path)
                WU(f"id:fbrow_status;text:Opened: {new_path}/;fg:{YEL}")
            else:
                # File selected — surface full path back to caller
                full = browser_path[0].rstrip("/") + "/" + name
                short = full if len(full) <= 38 else "…" + full[-36:]
                WU(f"id:fd_result;text:{short};fg:{GRN}")
                WU(f"id:fbrow_status;text:Selected: {full};fg:{GRN}")
                WU("id:fbrow;visible:false")

        # File dialog events removed — was local GTK picker, not remote

        # Quit
        elif eid == "btn_quit":
            break

finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
    print()
