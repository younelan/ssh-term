#!/usr/bin/env python3
"""
Contact Manager  —  demo for:
  • type:frame          labeled border around a group
  • type:table          multi-column list with row actions (Edit / Delete)
  • type:treeview       category sidebar with expand/collapse events
  • type:expander       collapsible "Add Contact" form
  • type:entry          name / phone input (press Enter to confirm)
  • type:spinbox        priority 1-5
  • type:datepicker     birthday picker
  • hexpand/vexpand     prop on widgets and panels
  • Alert= + input:     rename dialog that returns typed text
  • Confirm=            delete confirmation, fires action:confirm / action:cancel
  • Toast=              non-blocking success/info notifications
"""

import sys, os, tty, termios, select

ESC, BEL = "\033", "\007"

def W(s):  sys.stdout.write("{}]1337;Widget={}{}".format(ESC,s,BEL));  sys.stdout.flush()
def P(s):  sys.stdout.write("{}]1337;Panel={}{}".format(ESC,s,BEL));   sys.stdout.flush()
def WU(s): sys.stdout.write("{}]1337;WidgetUpdate={}{}".format(ESC,s,BEL)); sys.stdout.flush()
def A(s):  sys.stdout.write("{}]1337;Alert={}{}".format(ESC,s,BEL));   sys.stdout.flush()
def T(s):  sys.stdout.write("{}]1337;Toast={}{}".format(ESC,s,BEL));   sys.stdout.flush()
def C(s):  sys.stdout.write("{}]1337;Confirm={}{}".format(ESC,s,BEL)); sys.stdout.flush()

# ── Palette (dark navy) ────────────────────────────────────────────────────────
BG   = "#0f172a"
CARD = "#1e293b"
HDR  = "#0f3460"
FG   = "#e2e8f0"
DIM  = "#64748b"

# ── Data ──────────────────────────────────────────────────────────────────────
contacts = [
    {"name": "Alice Martin",   "phone": "+1 555-0101", "cat": "Friends",  "priority": 5, "birthday": "1990-03-15"},
    {"name": "Bob Chen",       "phone": "+1 555-0202", "cat": "Work",     "priority": 3, "birthday": "1985-07-22"},
    {"name": "Carol White",    "phone": "+1 555-0303", "cat": "Family",   "priority": 5, "birthday": "1978-11-08"},
    {"name": "Dave Kumar",     "phone": "+1 555-0404", "cat": "Work",     "priority": 2, "birthday": "1992-01-30"},
    {"name": "Eve Nakamura",   "phone": "+1 555-0505", "cat": "Friends",  "priority": 4, "birthday": "1995-06-14"},
    {"name": "Frank Müller",   "phone": "+49 30 555-0606", "cat": "Work", "priority": 1, "birthday": "1983-09-01"},
    {"name": "Grace Lee",      "phone": "+1 555-0707", "cat": "Family",   "priority": 5, "birthday": "2001-12-25"},
]

selected_cat   = "all"
pending_delete = None   # index into contacts[]
pending_edit   = None   # index into contacts[]
form = {"name": "", "phone": "", "priority": 3, "birthday": ""}

def stars(n): return "★" * n + "☆" * (5 - n)

def visible():
    if selected_cat == "all":
        return list(range(len(contacts)))
    return [i for i, c in enumerate(contacts) if c["cat"] == selected_cat]

def rebuild_table():
    WU("id:tbl;action:clear")
    for i in visible():
        c = contacts[i]
        WU("id:tbl;action:addrow;cols:{}|{}|{}|{}".format(
            c["name"], c["phone"], stars(c["priority"]), c["birthday"]))
    n = len(visible())
    total = len(contacts)
    WU("id:statusbar;text:{} / {} contact(s)  •  category: {}".format(
        n, total, selected_cat))

# ── Build UI ──────────────────────────────────────────────────────────────────
os.system('clear')
fd = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)

try:
    # Root  ──────────────────────────────────────────────────────────────────
    P("id:root;layout:vertical;spacing:0;margin:0;width:620;bg_color:{}".format(BG))

    # Titlebar
    W("type:titlebar;id:title;title:📇 Contact Manager;panel:root")

    # Main body (horizontal: sidebar + content)
    P("id:body;layout:horizontal;spacing:8;margin:8;panel:root;bg_color:{}".format(BG))

    # ── LEFT: category treeview in a frame ──────────────────────────────────
    W("type:frame;id:cat_frame;label:Categories;width:145;panel:body;bg_color:{}".format(CARD))
    W("type:treeview;id:cats;width:133;height:260;panel:cat_frame")

    WU("id:cats;action:addrow;label:All Contacts;rowid:all")
    WU("id:cats;action:addrow;label:  Friends;rowid:Friends;parent:all")
    WU("id:cats;action:addrow;label:  Work;rowid:Work;parent:all")
    WU("id:cats;action:addrow;label:  Family;rowid:Family;parent:all")
    WU("id:cats;action:expand_all")

    # ── RIGHT: table + form ──────────────────────────────────────────────────
    P("id:right;layout:vertical;spacing:8;margin:0;panel:body;hexpand:true;bg_color:{}".format(BG))

    # Contacts table in a frame
    W("type:frame;id:tbl_frame;label:Contacts;panel:right;hexpand:true;bg_color:{}".format(CARD))
    W("type:table;id:tbl;cols:Name|Phone|Priority|Birthday;widths:150|110|75|90;"
      "height:210;actions:✏ Edit|🗑 Delete;panel:tbl_frame;hexpand:true")

    # Status bar
    W("type:label;id:statusbar;label:Loading...;panel:right;fg_color:{}".format(DIM))

    # ── Add Contact expander ─────────────────────────────────────────────────
    W("type:expander;id:add_exp;label:＋ Add New Contact;panel:right")

    # Frame inside the expander
    W("type:frame;id:add_frame;label:New Contact Details;panel:add_exp;hexpand:true;bg_color:{}".format(CARD))

    P("id:form_box;layout:vertical;spacing:6;margin:0;panel:add_frame;bg_color:{}".format(CARD))

    # Name row
    P("id:row_name;layout:horizontal;spacing:6;panel:form_box;bg_color:{}".format(CARD))
    W("type:label;id:lbl_name;label:Name:;width:65;panel:row_name;fg_color:{}".format(FG))
    W("type:entry;id:inp_name;placeholder:Full name  (press Enter);panel:row_name;hexpand:true")

    # Phone row
    P("id:row_phone;layout:horizontal;spacing:6;panel:form_box;bg_color:{}".format(CARD))
    W("type:label;id:lbl_phone;label:Phone:;width:65;panel:row_phone;fg_color:{}".format(FG))
    W("type:entry;id:inp_phone;placeholder:+1 555-0000  (press Enter);panel:row_phone;hexpand:true")

    # Priority row
    P("id:row_prio;layout:horizontal;spacing:6;panel:form_box;bg_color:{}".format(CARD))
    W("type:label;id:lbl_prio;label:Priority:;width:65;panel:row_prio;fg_color:{}".format(FG))
    W("type:spinbox;id:inp_prio;min:1;max:5;value:3;step:1;panel:row_prio")
    W("type:label;id:lbl_prio2;label:(1 = low, 5 = high);panel:row_prio;fg_color:{}".format(DIM))

    # Birthday row
    P("id:row_bday;layout:horizontal;spacing:6;panel:form_box;bg_color:{}".format(CARD))
    W("type:label;id:lbl_bday;label:Birthday:;width:65;panel:row_bday;fg_color:{}".format(FG))
    W("type:datepicker;id:inp_bday;panel:row_bday")

    # Buttons row
    P("id:btn_row;layout:horizontal;spacing:8;panel:form_box;bg_color:{}".format(CARD))
    W("type:button;id:btn_add;label:Add Contact;panel:btn_row")
    W("type:button;id:btn_clear;label:Clear Form;panel:btn_row")
    W("type:button;id:btn_toast_demo;label:Test Toast;panel:btn_row")

    # Populate table
    rebuild_table()

    # ── Event loop ────────────────────────────────────────────────────────────
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

        evt   = seq.split('WidgetEvent=', 1)[1]
        parts = {}
        for p in evt.split(';'):
            if ':' in p:
                k, v = p.split(':', 1)
                parts[k] = v

        eid    = parts.get('id', '')
        action = parts.get('action', '')
        value  = parts.get('value', '')
        row    = int(parts.get('row', '0') or '0')

        # ── Category selection ───────────────────────────────────────────────
        if eid == 'cats' and action == 'selected':
            lv = value.strip().lstrip()
            if 'All' in lv:      selected_cat = 'all'
            elif 'Friend' in lv: selected_cat = 'Friends'
            elif 'Work'   in lv: selected_cat = 'Work'
            elif 'Family' in lv: selected_cat = 'Family'
            rebuild_table()
            continue

        # ── Treeview expand / collapse toasts ────────────────────────────────
        if eid == 'cats' and action == 'expanded':
            T("msg:📂 {} expanded;duration:1200".format(value.strip()))
            continue
        if eid == 'cats' and action == 'collapsed':
            T("msg:📁 {} collapsed;duration:1200".format(value.strip()))
            continue

        # ── Form field tracking ──────────────────────────────────────────────
        if eid == 'inp_name'  and action == 'submit':
            form['name']  = value; continue
        if eid == 'inp_phone' and action == 'submit':
            form['phone'] = value; continue
        if eid == 'inp_prio'  and action == 'changed':
            try:   form['priority'] = int(float(value))
            except: pass
            continue
        if eid == 'inp_bday' and action == 'selected':
            form['birthday'] = value; continue

        # ── Add Contact button ───────────────────────────────────────────────
        if eid == 'btn_add' and action == 'clicked':
            name = form['name'].strip()
            if not name:
                A("title:Missing Name;body:Enter a name in the form and press Enter to confirm it, then click Add.;ok:Got it")
                continue
            cat = selected_cat if selected_cat != 'all' else 'Friends'
            contacts.append({
                "name":     name,
                "phone":    form['phone'].strip() or '—',
                "cat":      cat,
                "priority": form['priority'],
                "birthday": form['birthday'] or '—',
            })
            rebuild_table()
            T("msg:✓ {} added to {};duration:2500".format(name, cat))
            form = {"name": "", "phone": "", "priority": 3, "birthday": ""}
            WU("id:inp_name;text:")
            WU("id:inp_phone;text:")
            continue

        # ── Clear Form ───────────────────────────────────────────────────────
        if eid == 'btn_clear' and action == 'clicked':
            form = {"name": "", "phone": "", "priority": 3, "birthday": ""}
            WU("id:inp_name;text:")
            WU("id:inp_phone;text:")
            T("msg:Form cleared;duration:1500")
            continue

        # ── Toast demo button ────────────────────────────────────────────────
        if eid == 'btn_toast_demo' and action == 'clicked':
            T("msg:🔔 This is a Toast notification! It auto-dismisses.;duration:3500")
            continue

        # ── Row action: Edit ─────────────────────────────────────────────────
        if eid == 'tbl' and action == 'row_action' and '✏' in value:
            vis = visible()
            if row < len(vis):
                ci = vis[row]
                pending_edit = ci
                A("id:edit_dlg;title:Rename Contact;"
                  "body:Current name: {};input:New name".format(contacts[ci]['name']))
            continue

        # ── Row action: Delete ───────────────────────────────────────────────
        if eid == 'tbl' and action == 'row_action' and '🗑' in value:
            vis = visible()
            if row < len(vis):
                ci = vis[row]
                pending_delete = ci
                C("id:del_confirm;title:Delete Contact;"
                  "body:Remove {} from your contacts?;"
                  "confirm:Delete;cancel:Keep".format(contacts[ci]['name']))
            continue

        # ── Confirm: yes → delete ────────────────────────────────────────────
        if eid == 'del_confirm' and action == 'confirm':
            if pending_delete is not None and pending_delete < len(contacts):
                name = contacts[pending_delete]['name']
                contacts.pop(pending_delete)
                rebuild_table()
                T("msg:🗑 {} removed;duration:2500".format(name))
            pending_delete = None
            continue

        if eid == 'del_confirm' and action == 'cancel':
            pending_delete = None
            T("msg:Deletion cancelled;duration:1500")
            continue

        # ── Alert edit result: value = typed text ────────────────────────────
        if eid == 'edit_dlg' and action == 'clicked':
            if pending_edit is not None and value.strip():
                old = contacts[pending_edit]['name']
                contacts[pending_edit]['name'] = value.strip()
                rebuild_table()
                T("msg:Renamed {} to {};duration:3000".format(old, value.strip()))
            pending_edit = None
            continue

finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
