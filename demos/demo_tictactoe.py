#!/usr/bin/env python3
"""Tic Tac Toe — OSC 1337 widget demo.  Minimax AI opponent."""

import sys
import os
import tty
import termios

ESC = "\033"
BEL = "\007"

def W(s):
    sys.stdout.write("{}]1337;Widget={}{}".format(ESC, s, BEL)); sys.stdout.flush()
def P(s):
    sys.stdout.write("{}]1337;Panel={}{}".format(ESC, s, BEL));  sys.stdout.flush()
def WU(s):
    sys.stdout.write("{}]1337;WidgetUpdate={}{}".format(ESC, s, BEL)); sys.stdout.flush()

# ── Palette ───────────────────────────────────────────────────────
BG_ROOT   = "#282a36"
BG_PANEL  = "#1e2030"
BG_CELL   = "#44475a"
BG_HEADER = "#21222c"
FG_MAIN   = "#f8f8f2"
FG_DIM    = "#6272a4"
FG_X      = "#8be9fd"
FG_O      = "#ff79c6"
FG_WIN    = "#50fa7b"

# ── Game state ────────────────────────────────────────────────────
WINS = [
    (0,1,2),(3,4,5),(6,7,8),
    (0,3,6),(1,4,7),(2,5,8),
    (0,4,8),(2,4,6),
]
board     = [''] * 9
current   = 'X'
game_over = False

def cell_id(r, c): return "b{}{}".format(r, c)
def idx(r, c):     return r * 3 + c

def check_winner_board(b):
    for a, bb, cc in WINS:
        if b[a] and b[a] == b[bb] == b[cc]:
            return b[a]
    return 'draw' if all(b) else None

def check_winner():
    return check_winner_board(board)

# ── Minimax ───────────────────────────────────────────────────────
def minimax(b, is_max, depth):
    r = check_winner_board(b)
    if r == 'X':    return -10 + depth
    if r == 'O':    return  10 - depth
    if r == 'draw': return 0
    if is_max:
        best = -100
        for i in range(9):
            if not b[i]:
                b[i] = 'O'; best = max(best, minimax(b, False, depth+1)); b[i] = ''
        return best
    else:
        best = 100
        for i in range(9):
            if not b[i]:
                b[i] = 'X'; best = min(best, minimax(b, True,  depth+1)); b[i] = ''
        return best

def ai_move():
    best_score, best_i = -100, -1
    for i in range(9):
        if not board[i]:
            board[i] = 'O'
            score = minimax(board[:], False, 0)
            board[i] = ''
            if score > best_score:
                best_score, best_i = score, i
    if best_i >= 0:
        board[best_i] = 'O'

# ── UI helpers ────────────────────────────────────────────────────
def winning_cells():
    for a, b, c in WINS:
        if board[a] and board[a] == board[b] == board[c]:
            return {a, b, c}
    return set()

def refresh_board(win_set=None):
    win_set = win_set or set()
    for r in range(3):
        for c in range(3):
            i = idx(r, c)
            v = board[i]
            if i in win_set:
                WU("id:{};text:{};css_class:ttt-win".format(cell_id(r,c), v))
            elif v == 'X':
                WU("id:{};text:X;css_class:ttt-x".format(cell_id(r,c)))
            elif v == 'O':
                WU("id:{};text:O;css_class:ttt-o".format(cell_id(r,c)))
            else:
                WU("id:{};text: ;css_class:ttt-empty".format(cell_id(r,c)))

def set_status(text, fg=FG_MAIN):
    WU("id:status;text:{};fg_color:{}".format(text, fg))

def set_board_sensitive(val):
    s = "true" if val else "false"
    for r in range(3):
        for c in range(3):
            WU("id:{};sensitive:{}".format(cell_id(r,c), s))

def new_game():
    global board, current, game_over
    board = [''] * 9; current = 'X'; game_over = False
    refresh_board()
    set_board_sensitive(True)
    set_status("Your turn  (you are X)", FG_X)

# ── Build UI ──────────────────────────────────────────────────────
os.system('clear')
fd = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)

try:
    P("id:root;layout:vertical;spacing:0;margin:0;width:280;bg_color:{}".format(BG_ROOT))

    # Header
    P("id:hdr;layout:vertical;spacing:4;margin:12;panel:root;bg_color:{}".format(BG_HEADER))
    W("type:label;id:title;label:Tic  Tac  Toe;panel:hdr;fg_color:{};bg_color:{}".format(FG_MAIN, BG_HEADER))
    W("type:label;id:status;label:Your turn  (you are X);panel:hdr;fg_color:{};bg_color:{}".format(FG_X, BG_HEADER))

    # Board
    P("id:board;layout:vertical;spacing:4;margin:10;panel:root;bg_color:{}".format(BG_ROOT))
    for r in range(3):
        P("id:row{};layout:horizontal;spacing:4;panel:board;bg_color:{}".format(r, BG_ROOT))
        for c in range(3):
            W("type:button;id:{};label: ;width:70;height:70;css_class:ttt-empty;panel:row{}".format(
                cell_id(r,c), r))

    # Footer
    P("id:footer;layout:horizontal;spacing:8;margin:10;panel:root;bg_color:{}".format(BG_ROOT))
    W("type:button;id:new_game;label:New Game;panel:footer;bg_color:{};fg_color:{}".format(BG_PANEL, FG_MAIN))

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

        if 'WidgetEvent' not in seq:
            continue

        evt   = seq.split('WidgetEvent=', 1)[1]
        parts = {}
        for p in evt.split(';'):
            if ':' in p:
                k, v = p.split(':', 1); parts[k] = v

        ev_id     = parts.get('id', '')
        ev_action = parts.get('action', '')

        if ev_id == 'new_game' and ev_action == 'clicked':
            new_game(); continue

        if ev_action == 'clicked' and not game_over:
            if len(ev_id) == 3 and ev_id[0] == 'b' and ev_id[1].isdigit() and ev_id[2].isdigit():
                r, c = int(ev_id[1]), int(ev_id[2])
                i = idx(r, c)
                if board[i] == '':
                    board[i] = 'X'
                    refresh_board()
                    result = check_winner()
                    if result == 'X':
                        refresh_board(winning_cells())
                        set_status("You win! \U0001f389", FG_WIN)
                        game_over = True; set_board_sensitive(False)
                    elif result == 'draw':
                        set_status("It's a draw!", FG_DIM)
                        game_over = True; set_board_sensitive(False)
                    else:
                        set_status("AI thinking...", FG_DIM)
                        set_board_sensitive(False)
                        ai_move()
                        refresh_board()
                        result2 = check_winner()
                        if result2 == 'O':
                            refresh_board(winning_cells())
                            set_status("AI wins! \U0001f916", FG_O)
                            game_over = True
                        elif result2 == 'draw':
                            set_status("It's a draw!", FG_DIM)
                            game_over = True
                        else:
                            set_board_sensitive(True)
                            set_status("Your turn  (you are X)", FG_X)

finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
