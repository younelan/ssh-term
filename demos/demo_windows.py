#!/usr/bin/env python3
"""
Multi-window demo  —  shows Window=, WindowUpdate=, modal, floating, hide/show.

Main window (inline):   Task list with Add / Edit / Delete
Settings window:        Floating, non-modal, toggle-able
Detail window:          Modal — opens when you double-click (select) a task
Log window:             Floating, always-on-side — shows recent events
"""

import sys, os, tty, termios, select, time

ESC, BEL = "\033", "\007"

def W(s):   sys.stdout.write("{}]1337;Widget={}{}".format(ESC,s,BEL));  sys.stdout.flush()
def P(s):   sys.stdout.write("{}]1337;Panel={}{}".format(ESC,s,BEL));   sys.stdout.flush()
def WU(s):  sys.stdout.write("{}]1337;WidgetUpdate={}{}".format(ESC,s,BEL)); sys.stdout.flush()
def WIN(s): sys.stdout.write("{}]1337;Window={}{}".format(ESC,s,BEL));  sys.stdout.flush()
def WW(s):  sys.stdout.write("{}]1337;WindowUpdate={}{}".format(ESC,s,BEL)); sys.stdout.flush()
def A(s):   sys.stdout.write("{}]1337;Alert={}{}".format(ESC,s,BEL));   sys.stdout.flush()
def T(s):   sys.stdout.write("{}]1337;Toast={}{}".format(ESC,s,BEL));   sys.stdout.flush()
def C(s):   sys.stdout.write("{}]1337;Confirm={}{}".format(ESC,s,BEL)); sys.stdout.flush()

BG    = "#0f172a"
CARD  = "#1e293b"
HDR   = "#1e3a5f"
FG    = "#e2e8f0"
DIM   = "#64748b"
GREEN = "#22c55e"
RED   = "#ef4444"
AMBER = "#f59e0b"

tasks = [
    {"title": "Design login screen",   "status": "done",    "prio": "high"},
    {"title": "Fix auth bug",          "status": "in-prog", "prio": "high"},
    {"title": "Write unit tests",      "status": "todo",    "prio": "medium"},
    {"title": "Update README",         "status": "todo",    "prio": "low"},
    {"title": "Deploy to staging",     "status": "in-prog", "prio": "high"},
    {"title": "Code review PR #42",    "status": "todo",    "prio": "medium"},
    {"title": "Performance profiling", "status": "todo",    "prio": "low"},
]

# Settings state
settings = {"show_done": True, "theme": "Dark", "font_size": 14}
# Log lines
log_lines = []
pending_delete = None
selected_task  = None    # index into tasks[]
settings_open  = False
log_open       = False

STATUS_ICON = {"done": "✓", "in-prog": "⟳", "todo": "○"}
PRIO_ICON   = {"high": "🔴", "medium": "🟡", "low": "🟢"}

def visible_tasks():
    if settings["show_done"]:
        return list(range(len(tasks)))
    return [i for i, t in enumerate(tasks) if t["status"] != "done"]

def rebuild_table():
    WU("id:task_tbl;action:clear")
    for i in visible_tasks():
        t = tasks[i]
        WU("id:task_tbl;action:addrow;cols:{}|{}|{}|{}".format(
            STATUS_ICON[t["status"]], t["title"], PRIO_ICON[t["prio"]], t["status"].replace("-"," ")))
    done_n = sum(1 for t in tasks if t["status"] == "done")
    WU("id:main_status;text:{} tasks  •  {} done  •  {} remaining".format(
        len(tasks), done_n, len(tasks) - done_n))

def add_log(msg):
    ts = time.strftime("%H:%M:%S")
    log_lines.append("[{}] {}".format(ts, msg))
    if len(log_lines) > 50:
        log_lines.pop(0)
    # Refresh log textview if open
    WU("id:log_tv;text:{}".format("\n".join(log_lines[-20:])))

# ── Build MAIN inline UI ───────────────────────────────────────────────────────
os.system('clear')
fd = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)

try:
    P("id:root;layout:vertical;spacing:0;margin:0;width:520;bg_color:{}".format(BG))
    W("type:titlebar;id:main_title;title:📋 Task Manager;panel:root")

    # Toolbar
    P("id:toolbar;layout:horizontal;spacing:6;margin:6;panel:root;bg_color:{}".format(HDR))
    W("type:button;id:btn_add_task;label:＋ Add Task;panel:toolbar")
    W("type:button;id:btn_settings;label:⚙ Settings;panel:toolbar")
    W("type:button;id:btn_log;label:📜 Log;panel:toolbar")
    W("type:label;id:spacer;label:;panel:toolbar;hexpand:true;fg_color:{}".format(HDR))
    W("type:button;id:btn_mark_done;label:✓ Done;panel:toolbar")
    W("type:button;id:btn_delete;label:🗑 Delete;panel:toolbar")

    # Task table
    W("type:frame;id:tbl_frame;label:Tasks;panel:root;hexpand:true;bg_color:{}".format(CARD))
    W("type:table;id:task_tbl;cols:St|Title|Pri|Status;widths:28|220|28|80;"
      "height:240;hexpand:true;panel:tbl_frame")

    W("type:label;id:main_status;label:Loading...;panel:root;fg_color:{}".format(DIM))

    rebuild_table()
    add_log("Task Manager started")

    # ── BUILD SETTINGS WINDOW (hidden until btn_settings clicked) ─────────────
    WIN("id:settings_win;title:⚙ Settings;width:320;height:260;modal:false")

    P("id:sw_body;layout:vertical;spacing:14;margin:16;panel:settings_win;bg_color:{}".format(CARD))

    # Show done toggle
    P("id:sw_row1;layout:horizontal;spacing:8;panel:sw_body;bg_color:{}".format(CARD))
    W("type:label;id:sw_lbl1;label:Show completed tasks:;panel:sw_row1;fg_color:{}".format(FG))
    W("type:checkbox;id:sw_show_done;label:;checked:true;panel:sw_row1")

    # Font size
    P("id:sw_row2;layout:horizontal;spacing:8;panel:sw_body;bg_color:{}".format(CARD))
    W("type:label;id:sw_lbl2;label:Font size:;panel:sw_row2;fg_color:{}".format(FG))
    W("type:spinbox;id:sw_font;min:8;max:32;value:14;step:1;panel:sw_row2")

    # Theme selector
    P("id:sw_row3;layout:horizontal;spacing:8;panel:sw_body;bg_color:{}".format(CARD))
    W("type:label;id:sw_lbl3;label:Theme:;panel:sw_row3;fg_color:{}".format(FG))
    W("type:dropdown;id:sw_theme;items:Dark|Light|Solarized;panel:sw_row3")

    W("type:separator;panel:sw_body")
    P("id:sw_btnrow;layout:horizontal;spacing:8;panel:sw_body;bg_color:{}".format(CARD))
    W("type:button;id:sw_apply;label:Apply;panel:sw_btnrow")
    W("type:button;id:sw_close;label:Close;panel:sw_btnrow")

    # Hide immediately — will show on demand
    WW("id:settings_win;visible:false")

    # ── BUILD LOG WINDOW (floating, non-modal) ────────────────────────────────
    WIN("id:log_win;title:📜 Event Log;width:380;height:280;modal:false")

    P("id:lw_body;layout:vertical;spacing:6;margin:8;panel:log_win;bg_color:{}".format(BG))
    W("type:textarea;id:log_tv;text:(no events yet);editable:false;width:360;height:220;"
      "panel:lw_body;hexpand:true;vexpand:true;bg_color:{}".format(CARD))
    P("id:lw_btnrow;layout:horizontal;spacing:8;panel:lw_body;bg_color:{}".format(BG))
    W("type:button;id:lw_clear;label:Clear;panel:lw_btnrow")
    W("type:button;id:lw_close;label:Close;panel:lw_btnrow")

    WW("id:log_win;visible:false")

    # ── Event loop ─────────────────────────────────────────────────────────────
    while True:
        ch = sys.stdin.read(1)
        if ch == '\x03': break
        if ch != ESC: continue

        seq = ""
        while True:
            c = sys.stdin.read(1)
            if c == BEL: break
            seq += c

        if 'WidgetEvent' not in seq:
            continue

        evt = seq.split('WidgetEvent=', 1)[1]
        parts = {}
        evt_tmp = evt.replace('\\\\', '\x01').replace('\\;', '\x02')
        for p in evt_tmp.split(';'):
            p = p.replace('\x01', '\\\\').replace('\x02', '\\;')
            if ':' in p:
                k, v = p.split(':', 1)
                parts[k] = v

        eid    = parts.get('id', '')
        action = parts.get('action', '')
        value  = parts.get('value', '')
        row_s  = parts.get('row', '0')
        row    = int(row_s) if row_s.isdigit() else 0

        # ── Main window close ──────────────────────────────────────────────────
        if eid == 'main_title' and action == 'close':
            break

        # ── Windows: close events ──────────────────────────────────────────────
        if action == 'close' and eid == 'settings_win':
            settings_open = False
            T("msg:Settings closed;duration:1500")
            continue
        if action == 'close' and eid == 'log_win':
            log_open = False
            continue

        # ── Toolbar: Settings toggle ───────────────────────────────────────────
        if eid == 'btn_settings' and action == 'clicked':
            if not settings_open:
                WW("id:settings_win;visible:true")
                settings_open = True
                add_log("Settings window opened")
            else:
                WW("id:settings_win;visible:false")
                settings_open = False
                add_log("Settings window hidden")
            continue

        # ── Toolbar: Log toggle ────────────────────────────────────────────────
        if eid == 'btn_log' and action == 'clicked':
            if not log_open:
                WW("id:log_win;visible:true")
                log_open = True
                add_log("Log window opened")
            else:
                WW("id:log_win;visible:false")
                log_open = False
            continue

        # ── Settings: Apply ────────────────────────────────────────────────────
        if eid == 'sw_apply' and action == 'clicked':
            rebuild_table()
            T("msg:Settings applied;duration:2000")
            add_log("Settings applied: show_done={}  font={}  theme={}".format(
                settings["show_done"], settings["font_size"], settings["theme"]))
            continue

        if eid == 'sw_close' and action == 'clicked':
            WW("id:settings_win;visible:false")
            settings_open = False
            continue

        if eid == 'sw_show_done' and action == 'toggled':
            settings["show_done"] = (value == "true")
            continue

        if eid == 'sw_font' and action == 'changed':
            try: settings["font_size"] = int(float(value))
            except: pass
            continue

        if eid == 'sw_theme' and action == 'selected':
            settings["theme"] = value
            continue

        # ── Log: clear ────────────────────────────────────────────────────────
        if eid == 'lw_clear' and action == 'clicked':
            log_lines.clear()
            WU("id:log_tv;text:(cleared)")
            continue

        if eid == 'lw_close' and action == 'clicked':
            WW("id:log_win;visible:false")
            log_open = False
            continue

        # ── Add Task ──────────────────────────────────────────────────────────
        if eid == 'btn_add_task' and action == 'clicked':
            A("id:add_task_dlg;title:New Task;body:Enter task title:;input:Task description;ok:Add Task")
            continue

        if eid == 'add_task_dlg' and action == 'clicked' and value.strip():
            tasks.append({"title": value.strip(), "status": "todo", "prio": "medium"})
            rebuild_table()
            T("msg:Task added;duration:2000")
            add_log("Added task: {}".format(value.strip()))
            continue

        # ── Mark Done ─────────────────────────────────────────────────────────
        if eid == 'btn_mark_done' and action == 'clicked':
            vis = visible_tasks()
            if selected_task is not None and selected_task < len(vis):
                ci = vis[selected_task]
                tasks[ci]["status"] = "done"
                rebuild_table()
                T("msg:Task marked done;duration:2000")
                add_log("Done: {}".format(tasks[ci]["title"]))
            else:
                T("msg:Select a task first;duration:2000")
            continue

        # ── Delete ────────────────────────────────────────────────────────────
        if eid == 'btn_delete' and action == 'clicked':
            vis = visible_tasks()
            if selected_task is not None and selected_task < len(vis):
                ci = vis[selected_task]
                pending_delete = ci
                C("id:del_confirm;title:Delete Task;body:Remove: {}?;confirm:Delete;cancel:Keep".format(
                    tasks[ci]["title"]))
            else:
                T("msg:Select a task first;duration:2000")
            continue

        if eid == 'del_confirm' and action == 'confirm':
            if pending_delete is not None:
                name = tasks[pending_delete]["title"]
                tasks.pop(pending_delete)
                selected_task = None
                rebuild_table()
                T("msg:Deleted: {};duration:2500".format(name))
                add_log("Deleted: {}".format(name))
            pending_delete = None
            continue

        if eid == 'del_confirm' and action == 'cancel':
            pending_delete = None
            continue

        # ── Task selection → open detail in a MODAL floating window ───────────
        if eid == 'task_tbl' and action == 'selected':
            vis = visible_tasks()
            if vis:
                # Find which row was selected by matching first col value
                for row_i, ci in enumerate(vis):
                    if STATUS_ICON[tasks[ci]["status"]] == value or tasks[ci]["title"] in value:
                        selected_task = row_i
                        t = tasks[ci]
                        # Open modal detail window
                        WIN("id:detail_win;title:Task Detail;width:360;height:220;modal:true")
                        P("id:dw_body;layout:vertical;spacing:12;margin:16;panel:detail_win;bg_color:{}".format(CARD))
                        W("type:label;id:dw_title;label:{};panel:dw_body;fg_color:{}".format(t["title"], FG))
                        W("type:separator;panel:dw_body")
                        W("type:label;id:dw_status;label:Status: {}  {};"
                          "panel:dw_body;fg_color:{}".format(t["status"], STATUS_ICON[t["status"]], FG))
                        W("type:label;id:dw_prio;label:Priority: {}  {};"
                          "panel:dw_body;fg_color:{}".format(t["prio"], PRIO_ICON[t["prio"]], FG))
                        P("id:dw_row;layout:horizontal;spacing:8;panel:dw_body;bg_color:{}".format(CARD))
                        W("type:button;id:dw_done;label:Mark Done;panel:dw_row")
                        W("type:button;id:dw_close;label:Close;panel:dw_row")
                        add_log("Opened detail: {}".format(t["title"]))
                        break
            continue

        if eid == 'dw_close' and action == 'clicked':
            WW("id:detail_win;close:")
            continue

        if eid == 'detail_win' and action == 'close':
            add_log("Detail window closed")
            continue

        if eid == 'dw_done' and action == 'clicked':
            vis = visible_tasks()
            if selected_task is not None and selected_task < len(vis):
                ci = vis[selected_task]
                tasks[ci]["status"] = "done"
                rebuild_table()
                T("msg:Done: {};duration:2000".format(tasks[ci]["title"]))
                add_log("Done (from detail): {}".format(tasks[ci]["title"]))
            WW("id:detail_win;close:")
            continue

finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
