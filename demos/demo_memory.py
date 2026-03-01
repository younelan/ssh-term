#!/usr/bin/env python3
"""Memory card game — OSC 1337 widget demo."""

import sys, os, tty, termios, random, select, time

ESC, BEL = "\033", "\007"

def W(s):  sys.stdout.write("{}]1337;Widget={}{}".format(ESC,s,BEL));  sys.stdout.flush()
def P(s):  sys.stdout.write("{}]1337;Panel={}{}".format(ESC,s,BEL));   sys.stdout.flush()
def WU(s): sys.stdout.write("{}]1337;WidgetUpdate={}{}".format(ESC,s,BEL)); sys.stdout.flush()
def A(s):  sys.stdout.write("{}]1337;Alert={}{}".format(ESC,s,BEL));  sys.stdout.flush()

# ── Palette ───────────────────────────────────────────────────────
BG_ROOT = "#16213e"
BG_HDR  = "#0f3460"
FG_MAIN = "#e2e8f0"
FG_DIM  = "#64748b"

# ── 8 pairs — value + suit ────────────────────────────────────────
PAIRS = [
    ("A", "\u2665"),  # ♥ red
    ("K", "\u2666"),  # ♦ red
    ("Q", "\u2665"),  # ♥ red
    ("J", "\u2666"),  # ♦ red
    ("10","\u2660"),  # ♠ black
    ("9", "\u2663"),  # ♣ black
    ("8", "\u2660"),  # ♠ black
    ("7", "\u2663"),  # ♣ black
]

def is_red(pair_idx): return pair_idx < 4  # 0-3 = hearts/diamonds

def css_for(pair_idx):
    return "card-red" if is_red(pair_idx) else "card-black"

def face_label(pair_idx):
    v, s = PAIRS[pair_idx]
    return "{}\\n{}".format(v, s)   # \\n → Rust unescapes to real newline

# ── Game state ────────────────────────────────────────────────────
board      = []
state      = []   # 'hidden' | 'flipped' | 'matched'
first_flip = None
moves      = 0

def cid(i): return "c{:02d}".format(i)

def init_board():
    global board, state, first_flip, moves
    deck = list(range(8)) * 2
    random.shuffle(deck)
    board      = deck
    state      = ['hidden'] * 16
    first_flip = None
    moves      = 0

def show_hidden(i):
    WU("id:{};text:?\\n ;css_class:card-hidden".format(cid(i)))

def show_flipped(i):
    WU("id:{};text:{};css_class:{}".format(cid(i), face_label(board[i]), css_for(board[i])))

def show_matched(i):
    WU("id:{};text:{};css_class:card-matched".format(cid(i), face_label(board[i])))

def reset_unmatched():
    """Flip all currently-flipped (unmatched) cards face-down."""
    global first_flip
    for i in range(16):
        if state[i] == 'flipped':
            state[i] = 'hidden'
            show_hidden(i)
    first_flip = None

def matched_pairs():
    return sum(1 for s in state if s == 'matched') // 2

def update_status():
    m = matched_pairs()
    WU("id:status;text:{}/8 pairs  \u2022  {} moves".format(m, moves))

# ── Build UI ──────────────────────────────────────────────────────
os.system('clear')
fd = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)

try:
    # root: 4 cols × 62px cards + 3 × 4px gaps + 2 × 8px margin = 272px
    P("id:root;layout:vertical;spacing:0;margin:0;width:272;bg_color:{}".format(BG_ROOT))

    # header
    P("id:hdr;layout:vertical;spacing:4;margin:10;panel:root;bg_color:{}".format(BG_HDR))
    W("type:label;id:title;label:Memory Game;panel:hdr;fg_color:{};bg_color:{}".format(FG_MAIN, BG_HDR))
    W("type:label;id:status;label:0/8 pairs  \u2022  0 moves;panel:hdr;fg_color:{};bg_color:{}".format(FG_DIM, BG_HDR))

    # card grid  (4 rows × 4 cols)
    P("id:board;layout:vertical;spacing:4;margin:8;panel:root;bg_color:{}".format(BG_ROOT))
    for row in range(4):
        P("id:r{};layout:horizontal;spacing:4;panel:board;bg_color:{}".format(row, BG_ROOT))
        for col in range(4):
            i = row * 4 + col
            W("type:button;id:{};label:?\\n ;width:62;height:62;css_class:card-hidden;panel:r{}".format(cid(i), row))

    # footer
    P("id:footer;layout:horizontal;spacing:8;margin:8;panel:root;bg_color:{}".format(BG_ROOT))
    W("type:button;id:new_game;label:New Game;panel:footer;bg_color:{};fg_color:{}".format(BG_HDR, FG_MAIN))

    init_board()

    # ── Event loop ────────────────────────────────────────────────
    while True:
        ch = sys.stdin.read(1)
        if ch == '\x03': break
        if ch != ESC:    continue

        seq = ""
        while True:
            c = sys.stdin.read(1)
            if c == BEL: break
            seq += c

        if 'WidgetEvent' not in seq: continue

        evt   = seq.split('WidgetEvent=', 1)[1]
        parts = {}
        for p in evt.split(';'):
            if ':' in p:
                k, v = p.split(':', 1); parts[k] = v

        ev_id     = parts.get('id', '')
        ev_action = parts.get('action', '')

        # Win alert OK → restart
        if ev_id == 'win_alert' and ev_action == 'clicked':
            init_board()
            for i in range(16):
                WU("id:{};sensitive:true".format(cid(i)))
                show_hidden(i)
            update_status()
            continue

        # New Game
        if ev_id == 'new_game' and ev_action == 'clicked':
            init_board()
            for i in range(16):
                WU("id:{};sensitive:true".format(cid(i)))
                show_hidden(i)
            update_status()
            continue

        # Card click
        if ev_action == 'clicked' and ev_id.startswith('c') and len(ev_id) == 3:
            i = int(ev_id[1:])
            if state[i] in ('flipped', 'matched'):
                continue

            # Flip this card
            moves += 1
            state[i] = 'flipped'
            show_flipped(i)

            # Check for pair
            flipped = [j for j in range(16) if state[j] == 'flipped']
            if len(flipped) == 2:
                a, b = flipped[0], flipped[1]
                if board[a] == board[b]:
                    # Match!
                    state[a] = 'matched'
                    state[b] = 'matched'
                    show_matched(a)
                    show_matched(b)
                    if matched_pairs() == 8:
                        update_status()
                        for j in range(16):
                            WU("id:{};sensitive:false".format(cid(j)))
                        A("id:win_alert;title:You Win! \U0001f389;body:Completed in {} moves.;ok:Play Again".format(moves))
                        continue
                else:
                    # No match — pause 800 ms then auto-flip back
                    # Drain any input that arrived during the pause so stale
                    # clicks don't immediately trigger the next flip.
                    update_status()
                    time.sleep(0.8)
                    reset_unmatched()
                    # Discard buffered input during the sleep
                    while select.select([sys.stdin], [], [], 0)[0]:
                        sys.stdin.read(1)
                    continue
            update_status()

finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
