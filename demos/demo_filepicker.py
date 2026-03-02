#!/usr/bin/env python3
"""
demo_filepicker.py  –  Notepad with a proper floating file-picker window.

The file picker opens as a real separate GTK window (modal, decorated,
resizable) — exactly like a native OS file dialog — but the file listing
comes from the script, so it works identically over SSH by swapping
LocalFS for SFTPBackend.
"""
import sys, os, tty, termios, time, pathlib

ESC, BEL = "\033", "\007"

def osc(cmd):
    sys.stdout.write(f"{ESC}]1337;{cmd}{BEL}")
    sys.stdout.flush()

def enc_val(s: str) -> str:
    """Escape a string for embedding in an OSC Widget property value.
    Rust's parse_widget_props will unescape: \\\\->\\ \\n->newline \\r->CR \\;->;"""
    out = []
    for ch in s:
        if ch == '\\':
            out.append('\\\\')
        elif ch == '\n':
            out.append('\\n')
        elif ch == '\r':
            out.append('\\r')
        elif ch == ';':
            out.append('\\;')
        else:
            out.append(ch)
    return ''.join(out)

def P(s):    osc(f"Panel={s}")
def W(s):    osc(f"Widget={s}")
def WU(s):   osc(f"WidgetUpdate={s}")
def WIN(s):  osc(f"Window={s}")
def WINU(s): osc(f"WindowUpdate={s}")
def GV(wid): osc(f"GetWidgetValue=id:{wid}")

# ── Colour palette ────────────────────────────────────────────────────────────
BG   = "#1a1e2e"
BG2  = "#252a3a"
BG3  = "#1e2030"
BG4  = "#2e3450"
FG   = "#e2e8f0"
DIM  = "#64748b"
ACC  = "#7c3aed"
GRN  = "#10b981"
RED  = "#ef4444"
YEL  = "#f59e0b"
BLU  = "#3b82f6"
DIR_FG = "#93c5fd"   # blue tint for folders

# ── Raw event reader ──────────────────────────────────────────────────────────
def _split_props(s: str) -> list:
    """Split on ';' but treat '\\;' as a literal semicolon (don't split there)."""
    parts, cur, i = [], [], 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s) and s[i + 1] == ";":
            cur.append(";")
            i += 2
        elif s[i] == ";":
            parts.append("".join(cur))
            cur = []
            i += 1
        else:
            cur.append(s[i])
            i += 1
    parts.append("".join(cur))
    return parts

def read_event():
    """Read the next WidgetEvent from the PTY. Raises SystemExit on Ctrl+C."""
    while True:
        ch = sys.stdin.read(1)
        if ch == "\x03":          # Ctrl+C in raw mode
            raise SystemExit(0)
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
        d = {}
        for p in _split_props(seq.split("WidgetEvent=", 1)[1]):
            if ":" in p:
                k, v = p.split(":", 1)
                d[k] = v
        return d

# ═══════════════════════════════════════════════════════════════════════════════
# Filesystem backend  –  swap LocalFS() for SFTPBackend(sftp) for remote use
# ═══════════════════════════════════════════════════════════════════════════════

class LocalFS:
    """Browse the local filesystem."""
    def listdir(self, path):
        """Return list of (name, is_dir, size_str, mtime_str) sorted dirs-first.
        Raises PermissionError so callers can display a proper message.
        Broken symlinks and un-statable entries are silently skipped."""
        entries = []
        # Let PermissionError / OSError on the directory itself propagate
        for entry in sorted(pathlib.Path(path).iterdir(),
                            key=lambda e: (not e.is_dir(), e.name.lower())):
            try:
                # lstat() gets the symlink's own stat; follow with stat() to
                # detect broken symlinks — if stat() raises, skip the entry.
                st = entry.stat()   # follows symlink; raises OSError if broken
                size  = self._fmt_size(st.st_size) if entry.is_file() else "<dir>"
                mtime = time.strftime("%b %d %H:%M", time.localtime(st.st_mtime))
                entries.append((entry.name, entry.is_dir(), size, mtime))
            except OSError:
                pass   # broken symlink or no permission on entry — skip silently
        return entries

    def read_file(self, path):
        return pathlib.Path(path).read_text(errors="replace")

    def write_file(self, path, content):
        pathlib.Path(path).write_text(content)

    @staticmethod
    def _fmt_size(n):
        for unit in ("B", "KB", "MB", "GB"):
            if n < 1024:
                return f"{n:.0f} {unit}"
            n /= 1024
        return f"{n:.1f} TB"


# ── To browse a remote SSH host, use this instead of LocalFS(): ───────────────
#
# import paramiko
#
# class SFTPBackend:
#     def __init__(self, sftp):   # sftp = paramiko.SFTPClient
#         self.sftp = sftp
#
#     def listdir(self, path):
#         entries = []
#         for attr in sorted(self.sftp.listdir_attr(path),
#                            key=lambda a: (not stat.S_ISDIR(a.st_mode), a.filename.lower())):
#             is_dir = stat.S_ISDIR(attr.st_mode)
#             size   = self._fmt_size(attr.st_size) if not is_dir else "<dir>"
#             mtime  = time.strftime("%b %d %H:%M", time.localtime(attr.st_mtime))
#             entries.append((attr.filename, is_dir, size, mtime))
#         return entries
#
#     def read_file(self, path):
#         with self.sftp.open(path) as f:
#             return f.read().decode(errors="replace")
#
#     def write_file(self, path, content):
#         with self.sftp.open(path, "w") as f:
#             f.write(content)
#
#     @staticmethod
#     def _fmt_size(n):   ...  # same as LocalFS

fs = LocalFS()

# dir_entries maps column-0 display string → (full_path, is_dir), rebuilt on each populate()
dir_entries: dict = {}

# ═══════════════════════════════════════════════════════════════════════════════
# UI build
# ═══════════════════════════════════════════════════════════════════════════════

os.system("clear")
fd    = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)

try:
    # ── Root panel (notepad app) ──────────────────────────────────────────────
    P(f"id:root;layout:vertical;spacing:0;margin:0;bg:{BG}")

    P(f"id:toolbar;panel:root;layout:horizontal;spacing:6;margin:6")
    W(f"type:button;id:btn_new;panel:toolbar;label:📄 New;tooltip:New file")
    W(f"type:button;id:btn_open;panel:toolbar;"
      f"label:📂 Open…;bg:{BLU};fg:#fff;tooltip:Open file")
    W(f"type:button;id:btn_save;panel:toolbar;"
      f"label:💾 Save;tooltip:Save current file")
    W(f"type:button;id:btn_saveas;panel:toolbar;"
      f"label:Save As…;tooltip:Save to a new path")
    W(f"type:label;id:lbl_title;panel:toolbar;"
      f"text:Untitled;fg:{DIM};font:Sans Italic;size:12;hexpand:true")
    W(f"type:button;id:btn_quit;panel:toolbar;"
      f"label:✕;bg:{RED};fg:#fff;tooltip:Quit")

    W(f"type:textarea;id:editor;panel:root;"
      f"editable:true;wrap:none;width:720;height:460;"
      f"font:Monospace;size:13;bg:{BG3};fg:{FG};"
      f"text:Click 📂 Open… to load a file.")

    # ═══════════════════════════════════════════════════════════════════════════
    # File-picker dialog — a SEPARATE floating GTK window (Window= OSC).
    # It appears on top like a native OS file dialog, modal over the app.
    # Its root box is registered as panel "filedlg" so Panel=/Widget= with
    # panel:filedlg route content into the window, not the main layout.
    # ═══════════════════════════════════════════════════════════════════════════
    WIN(f"id:filedlg;title:Open File;"
        f"width:640;height:460;modal:true;resizable:true;"
        f"visible:false")

    # All content below uses panel:filedlg → lives inside the separate window
    P(f"id:dlg_body;panel:filedlg;layout:vertical;spacing:8;margin:10;bg:{BG2}")

    # — Navigation bar —
    P(f"id:dlg_nav;panel:dlg_body;layout:horizontal;spacing:6")
    W(f"type:button;id:dlg_up;panel:dlg_nav;"
      f"label:↑ Up;tooltip:Go to parent folder")
    W(f"type:button;id:dlg_home;panel:dlg_nav;"
      f"label:🏠 Home;tooltip:Home directory")
    W(f"type:button;id:dlg_desktop;panel:dlg_nav;"
      f"label:🖥 Desktop;tooltip:Desktop folder")
    W(f"type:label;id:dlg_path;panel:dlg_nav;"
      f"text:~;fg:{FG};font:Monospace;size:11;hexpand:true")

    # — File list (fills the window like a native file dialog) —
    W(f"type:table;id:filelist;panel:dlg_body;"
      f"cols:  Name|Size|Date Modified;"
      f"widths:360|80|130;"
      f"sortable:false;"
      f"width:600;height:310")

    # — Filename bar (always visible — shows selected name, also accepts typed names) —
    P(f"id:dlg_fname_row;panel:dlg_body;layout:horizontal;spacing:8")
    W(f"type:label;id:dlg_fname_lbl;panel:dlg_fname_row;"
      f"text:Filename:;fg:{DIM};size:12")
    W(f"type:entry;id:dlg_fname;panel:dlg_fname_row;"
      f"placeholder:Enter filename…;hexpand:true")

    # — Bottom bar —
    P(f"id:dlg_bottom;panel:dlg_body;layout:horizontal;spacing:8")
    W(f"type:label;id:dlg_status;panel:dlg_bottom;"
      f"text: ;fg:{DIM};size:11;font:Monospace;hexpand:true")
    W(f"type:button;id:dlg_cancel;panel:dlg_bottom;label:Cancel")
    W(f"type:button;id:dlg_confirm;panel:dlg_bottom;"
      f"label:Open;bg:{BLU};fg:#fff")

    # ── State ─────────────────────────────────────────────────────────────────
    picker_mode  = ["open"]
    picker_path  = [str(pathlib.Path.home())]
    picker_sel   = [None]   # full path of highlighted file (None = nothing selected)
    current_file = [None]
    pending_save = [None]
    picker_open    = [False]  # True only while the file dialog window is visible
    act_skip       = [0]      # discard this many upcoming activated events (anti-spurious double-nav)
    pending_confirm = [None]  # "open" or "save" while waiting for GV(dlg_fname) reply
    # Maps the first-column display string (stripped) → (full_path, is_dir)
    # Rebuilt on every populate() call so folder navigation is exact.
    # (declared at module level above the try block)

    def fmt_path(p):
        h = str(pathlib.Path.home())
        return "~" + p[len(h):] if p.startswith(h) else p

    def populate(path):
        global dir_entries
        dir_entries = {}
        # Absorb up to 3 spurious activated events GTK fires after clear+addrow
        act_skip[0] = 3
        WU("id:filelist;action:clear")
        WU(f"id:dlg_path;text:{fmt_path(path)}")
        WU("id:dlg_status;text: ")
        picker_sel[0] = None
        try:
            rows = fs.listdir(path)
        except PermissionError:
            WU(f"id:dlg_status;text:Permission denied: {path};fg:{RED}")
            return
        except OSError as e:
            WU(f"id:dlg_status;text:Cannot read: {e};fg:{RED}")
            return
        except Exception as e:
            WU(f"id:dlg_status;text:Error: {e};fg:{RED}")
            return
        if not rows:
            WU("id:filelist;action:addrow;cols: (empty)| | ")
            return
        for name, is_dir, size, mod in rows:
            icon = "📁" if is_dir else "📄"
            fg   = DIR_FG if is_dir else FG
            disp = f"{icon} {name}"   # what will appear in column 0
            full = str(pathlib.Path(path) / name)
            dir_entries[disp] = (full, is_dir)
            WU(f"id:filelist;action:addrow;"
               f"cols:{disp}|{size}|{mod};fg:{fg}")

    def open_picker(mode):
        picker_mode[0] = mode
        picker_sel[0]  = None
        picker_open[0] = True
        act_skip[0]    = 0
        pending_confirm[0] = None
        WINU(f"id:filedlg;title:{'Open File' if mode == 'open' else 'Save File'}")
        WU(f"id:dlg_confirm;label:{'Open' if mode == 'open' else 'Save'}")
        # Filename bar is always visible — user can type or click to populate it
        WU("id:dlg_fname_row;visible:true")
        if mode == "save" and current_file[0]:
            WU(f"id:dlg_fname;text:{pathlib.Path(current_file[0]).name}")
        else:
            WU("id:dlg_fname;text:")
        populate(picker_path[0])
        WINU("id:filedlg;visible:true")

    def close_picker():
        picker_open[0] = False
        act_skip[0]    = 0
        pending_confirm[0] = None
        global dir_entries
        dir_entries    = {}
        WINU("id:filedlg;visible:false")

    # ── Event loop ────────────────────────────────────────────────────────────
    while True:
        ev  = read_event()
        eid = ev.get("id", "")
        act = ev.get("action", "")

        if eid == "btn_quit":
            break

        elif eid == "btn_new":
            WU("id:editor;text:")
            WU(f"id:lbl_title;text:Untitled;fg:{DIM}")
            current_file[0] = None

        elif eid == "btn_open":
            open_picker("open")

        elif eid == "btn_save":
            if current_file[0] is None:
                open_picker("save")
            else:
                pending_save[0] = current_file[0]
                GV("editor")

        elif eid == "btn_saveas":
            open_picker("save")

        # Window X button closed the picker window
        elif eid == "filedlg" and act == "close":
            close_picker()

        elif eid == "dlg_up":
            p = pathlib.Path(picker_path[0])
            if p.parent != p:
                picker_path[0] = str(p.parent)
                populate(picker_path[0])

        elif eid == "dlg_home":
            picker_path[0] = str(pathlib.Path.home())
            populate(picker_path[0])

        elif eid == "dlg_desktop":
            desk = str(pathlib.Path.home() / "Desktop")
            picker_path[0] = desk if pathlib.Path(desk).exists() else str(pathlib.Path.home())
            populate(picker_path[0])

        # Single click → highlight the file, set picker_sel, update status/filename bar
        elif eid == "filelist" and act == "selected":
            if not picker_open[0]:
                continue
            # Never skip selected events — these are always genuine user clicks
            disp = ev.get("value", "").split("|")[0].strip()
            entry = dir_entries.get(disp)
            if entry:
                full, is_dir = entry
                if not is_dir:
                    picker_sel[0] = full
                    name = pathlib.Path(full).name
                    short = name if len(name) <= 46 else "…" + name[-44:]
                    WU(f"id:dlg_status;text:{short};fg:{FG}")
                    WU(f"id:dlg_fname;text:{name}")  # always populate the entry
                else:
                    WU(f"id:dlg_status;text:{pathlib.Path(full).name}/  (double-click to open);fg:{DIM}")

        # Double click → navigate into folder or open file
        elif eid == "filelist" and act == "activated":
            if not picker_open[0]:
                continue
            if act_skip[0] > 0:
                act_skip[0] -= 1
                continue
            disp = ev.get("value", "").split("|")[0].strip()
            entry = dir_entries.get(disp)
            if entry:
                full, is_dir = entry
                if is_dir:
                    picker_path[0] = full
                    populate(full)
                else:
                    picker_sel[0] = full
                    name = pathlib.Path(full).name
                    WU(f"id:dlg_status;text:{name};fg:{GRN}")
                    # Auto-confirm on double-click for files in open mode
                    if picker_mode[0] == "open":
                        try:
                            content = fs.read_file(full)
                            WU(f"id:editor;text:{enc_val(content)}")
                            current_file[0] = full
                            WU(f"id:lbl_title;text:{fmt_path(full)};fg:{FG}")
                            close_picker()
                        except Exception as e:
                            WU(f"id:dlg_status;text:Error: {e};fg:{RED}")

        elif eid == "dlg_confirm":
            # Read whatever is in the filename entry, then act in the dlg_fname value handler
            if picker_open[0]:
                pending_confirm[0] = picker_mode[0]
                GV("dlg_fname")

        elif eid == "dlg_fname" and act == "value":
            mode = pending_confirm[0]
            pending_confirm[0] = None
            if not mode:
                continue
            raw_name = ev.get("value", "").strip()
            if not raw_name:
                WU(f"id:dlg_status;text:Enter a filename first;fg:{YEL}")
                pending_confirm[0] = None
                continue
            # Build the full path: if raw_name is absolute use it, else join with current dir
            p = pathlib.Path(raw_name)
            path = str(p) if p.is_absolute() else str(pathlib.Path(picker_path[0]) / raw_name)
            if mode == "open":
                try:
                    content = fs.read_file(path)
                    WU(f"id:editor;text:{enc_val(content)}")
                    current_file[0] = path
                    WU(f"id:lbl_title;text:{fmt_path(path)};fg:{FG}")
                    close_picker()
                except Exception as e:
                    WU(f"id:dlg_status;text:Error: {e};fg:{RED}")
            else:  # save
                pending_save[0] = path
                close_picker()
                GV("editor")

        elif eid == "dlg_cancel":
            close_picker()

        # GetWidgetValue response → complete the pending save
        elif eid == "editor" and act == "value":
            if pending_save[0]:
                raw = ev.get("value", "")
                # Unescape: \\ → \, \n → newline, \; → ;
                content = raw.replace("\\n", "\n").replace("\\;", ";").replace("\\\\", "\\")
                path    = pending_save[0]
                pending_save[0] = None
                try:
                    fs.write_file(path, content)
                    current_file[0] = path
                    WU(f"id:lbl_title;text:{fmt_path(path)};fg:{FG}")
                except Exception as e:
                    WU(f"id:lbl_title;text:Save error: {e};fg:{RED}")

finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
    print()
