#!/usr/bin/env python3
"""
HTML Editor demo — rich text editing with inline SVG, tables, styling.

Features demonstrated:
  • type:htmledit         Rich HTML editor widget (osz-htmledit)
  • html64 property       Base64-encoded HTML (avoids escaping issues)
  • Inline SVG rendering  Logos, icons, decorative elements
  • Formatting toolbar    Bold, italic, underline, lists, alignment
  • Open / Save HTML      File dialog integration
  • Live preview window   Readonly htmledit in a floating window
  • Template gallery      Pre-built HTML templates to load
"""

import sys, os, tty, termios, base64

ESC, BEL = "\033", "\007"

def W(s):   sys.stdout.write("{}]1337;Widget={}{}".format(ESC,s,BEL));  sys.stdout.flush()
def P(s):   sys.stdout.write("{}]1337;Panel={}{}".format(ESC,s,BEL));   sys.stdout.flush()
def WU(s):  sys.stdout.write("{}]1337;WidgetUpdate={}{}".format(ESC,s,BEL)); sys.stdout.flush()
def WIN(s): sys.stdout.write("{}]1337;Window={}{}".format(ESC,s,BEL));  sys.stdout.flush()
def WW(s):  sys.stdout.write("{}]1337;WindowUpdate={}{}".format(ESC,s,BEL)); sys.stdout.flush()
def GV(s):  sys.stdout.write("{}]1337;GetWidgetValue={}{}".format(ESC,s,BEL)); sys.stdout.flush()
def FD(s):  sys.stdout.write("{}]1337;FileDialog={}{}".format(ESC,s,BEL)); sys.stdout.flush()
def A(s):   sys.stdout.write("{}]1337;Alert={}{}".format(ESC,s,BEL));   sys.stdout.flush()
def T(s):   sys.stdout.write("{}]1337;Toast={}{}".format(ESC,s,BEL));   sys.stdout.flush()

def b64(html):
    """Base64-encode HTML for safe OSC transport."""
    return base64.b64encode(html.encode('utf-8')).decode('ascii')

# ── Palette ───────────────────────────────────────────────────────────────────
BG    = "#0f0f1a"
CARD  = "#1a1a2e"
HDR   = "#16213e"
TB    = "#0a1628"
FG    = "#e0e0ff"
DIM   = "#6272a4"
CYAN  = "#8be9fd"
GREEN = "#50fa7b"
PINK  = "#ff79c6"
PURPLE= "#bd93f9"
ORANGE= "#ffb86c"
RED   = "#ff5555"
YELLOW= "#f1fa8c"

# ── SVG Assets ────────────────────────────────────────────────────────────────

SVG_LOGO = '''<svg xmlns="http://www.w3.org/2000/svg" width="320" height="64" viewBox="0 0 320 64">
  <rect width="320" height="64" rx="12" fill="#16213e" stroke="#333355" stroke-width="0.8"/>
  <rect x="8" y="8" width="48" height="48" rx="10" fill="#bd93f9" opacity="0.85"/>
  <rect x="8" y="8" width="48" height="48" rx="10" fill="#ff79c6" opacity="0.35"/>
  <rect x="14" y="16" width="10" height="3" rx="1" fill="white" opacity="0.9"/>
  <rect x="14" y="22" width="16" height="3" rx="1" fill="white" opacity="0.7"/>
  <rect x="14" y="28" width="12" height="3" rx="1" fill="white" opacity="0.5"/>
  <rect x="14" y="34" width="18" height="3" rx="1" fill="white" opacity="0.7"/>
  <rect x="14" y="40" width="8" height="3" rx="1" fill="white" opacity="0.9"/>
  <circle cx="42" cy="26" r="6" fill="none" stroke="white" stroke-width="1.5" opacity="0.6"/>
  <line x1="46" y1="30" x2="50" y2="34" stroke="white" stroke-width="1.5" opacity="0.6"/>
  <text x="72" y="30" font-family="sans-serif" font-size="18" font-weight="bold" fill="#bd93f9">HTML Editor</text>
  <text x="72" y="48" font-family="sans-serif" font-size="11" fill="#6272a4">Rich text editing with SVG support</text>
  <rect x="260" y="14" width="6" height="20" rx="2" fill="#ff79c6" opacity="0.6"/>
  <rect x="270" y="10" width="6" height="24" rx="2" fill="#8be9fd" opacity="0.6"/>
  <rect x="280" y="18" width="6" height="16" rx="2" fill="#50fa7b" opacity="0.6"/>
  <rect x="290" y="8" width="6" height="26" rx="2" fill="#ffb86c" opacity="0.6"/>
  <rect x="300" y="12" width="6" height="22" rx="2" fill="#bd93f9" opacity="0.6"/>
</svg>'''

SVG_WAVE = '''<svg xmlns="http://www.w3.org/2000/svg" width="560" height="36" viewBox="0 0 560 36">
  <path d="M0,18 Q70,2 140,18 T280,18 T420,18 T560,18 L560,36 L0,36 Z" fill="#bd93f9" opacity="0.15"/>
  <path d="M0,22 Q70,8 140,22 T280,22 T420,22 T560,22" fill="none" stroke="#ff79c6" stroke-width="1.2" opacity="0.5"/>
  <path d="M0,26 Q70,14 140,26 T280,26 T420,26 T560,26" fill="none" stroke="#8be9fd" stroke-width="0.8" opacity="0.4"/>
</svg>'''

SVG_BADGE = '''<svg xmlns="http://www.w3.org/2000/svg" width="130" height="28" viewBox="0 0 130 28">
  <rect width="130" height="28" rx="14" fill="#bd93f9" opacity="0.15"/>
  <rect width="130" height="28" rx="14" fill="none" stroke="#bd93f9" stroke-width="1.2"/>
  <text x="65" y="19" font-family="sans-serif" font-size="12" fill="#bd93f9" text-anchor="middle">Rich Editor</text>
</svg>'''

SVG_DIVIDER = '''<svg xmlns="http://www.w3.org/2000/svg" width="480" height="16" viewBox="0 0 480 16">
  <line x1="0" y1="8" x2="190" y2="8" stroke="#444466" stroke-width="0.8" stroke-dasharray="4,4"/>
  <circle cx="210" cy="8" r="2.5" fill="#ff79c6"/>
  <circle cx="228" cy="8" r="2.5" fill="#bd93f9"/>
  <circle cx="246" cy="8" r="2.5" fill="#8be9fd"/>
  <line x1="266" y1="8" x2="480" y2="8" stroke="#444466" stroke-width="0.8" stroke-dasharray="4,4"/>
</svg>'''

SVG_ICON_CODE = '''<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
  <circle cx="16" cy="16" r="14" fill="#16213e" stroke="#8be9fd" stroke-width="1.2"/>
  <text x="16" y="21" font-family="monospace" font-size="14" fill="#8be9fd" text-anchor="middle">{}</text>
</svg>'''

SVG_ICON_PEN = '''<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
  <circle cx="16" cy="16" r="14" fill="#16213e" stroke="#ff79c6" stroke-width="1.2"/>
  <text x="16" y="22" font-family="sans-serif" font-size="16" fill="#ff79c6" text-anchor="middle">&#9998;</text>
</svg>'''

SVG_ICON_IMG = '''<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
  <circle cx="16" cy="16" r="14" fill="#16213e" stroke="#50fa7b" stroke-width="1.2"/>
  <text x="16" y="22" font-family="sans-serif" font-size="15" fill="#50fa7b" text-anchor="middle">&#9733;</text>
</svg>'''

SVG_CHART = '''<svg xmlns="http://www.w3.org/2000/svg" width="320" height="140" viewBox="0 0 320 140">
  <rect width="320" height="140" rx="8" fill="#16213e" stroke="#333355" stroke-width="0.8"/>
  <text x="160" y="20" font-family="sans-serif" font-size="11" fill="#6272a4" text-anchor="middle">Monthly Views</text>
  <rect x="25"  y="95" width="28" height="28" rx="4" fill="#bd93f9" opacity="0.75"/>
  <rect x="65"  y="72" width="28" height="51" rx="4" fill="#ff79c6" opacity="0.75"/>
  <rect x="105" y="50" width="28" height="73" rx="4" fill="#8be9fd" opacity="0.75"/>
  <rect x="145" y="38" width="28" height="85" rx="4" fill="#50fa7b" opacity="0.75"/>
  <rect x="185" y="30" width="28" height="93" rx="4" fill="#ffb86c" opacity="0.75"/>
  <rect x="225" y="42" width="28" height="81" rx="4" fill="#f1fa8c" opacity="0.75"/>
  <rect x="265" y="55" width="28" height="68" rx="4" fill="#ff5555" opacity="0.75"/>
  <text x="39"  y="135" font-size="9" fill="#6272a4" text-anchor="middle">Jan</text>
  <text x="79"  y="135" font-size="9" fill="#6272a4" text-anchor="middle">Feb</text>
  <text x="119" y="135" font-size="9" fill="#6272a4" text-anchor="middle">Mar</text>
  <text x="159" y="135" font-size="9" fill="#6272a4" text-anchor="middle">Apr</text>
  <text x="199" y="135" font-size="9" fill="#6272a4" text-anchor="middle">May</text>
  <text x="239" y="135" font-size="9" fill="#6272a4" text-anchor="middle">Jun</text>
  <text x="279" y="135" font-size="9" fill="#6272a4" text-anchor="middle">Jul</text>
</svg>'''

SVG_STARS = '''<svg xmlns="http://www.w3.org/2000/svg" width="100" height="20" viewBox="0 0 100 20">
  <text x="0" y="16" font-size="16" fill="#f1fa8c">&#9733;&#9733;&#9733;&#9733;&#9733;</text>
</svg>'''


# ── HTML Templates ────────────────────────────────────────────────────────────

TEMPLATE_WELCOME = '''<body style="background-color: {bg}; padding: 16px;">
<div style="font-family: sans-serif; color: {fg};">
{logo}
<h1 style="color: {purple}; margin-top: 12px;">Welcome to the HTML Editor</h1>
{badge}
<p style="color: {cyan}; font-size: 14px; margin-top: 10px;">
A powerful WYSIWYG editor with inline SVG, tables, lists, and rich formatting.</p>
{wave}

<h2 style="color: {pink};">Features</h2>
<table border="1" cellpadding="8" cellspacing="0"
  style="border-collapse: collapse; border-color: #444466; width: 95%;">
  <tr style="background-color: #1e2a4a;">
    <th style="color: #e8b4f8; text-align: left; padding: 10px;">Feature</th>
    <th style="color: #e8b4f8; text-align: left; padding: 10px;">Description</th>
  </tr>
  <tr style="background-color: #141828;">
    <td style="color: #8be9fd; padding: 8px;">{icon_code} Rich Text</td>
    <td style="color: #ffffff; padding: 8px;"><b>Bold</b>, <i>italic</i>,
      <u>underline</u>, colors, fonts</td>
  </tr>
  <tr style="background-color: #1a2040;">
    <td style="color: #ff79c6; padding: 8px;">{icon_pen} Lists &amp; Layout</td>
    <td style="color: #ffffff; padding: 8px;">Bullet lists, numbered lists, blockquotes,
      alignment, indentation</td>
  </tr>
  <tr style="background-color: #141828;">
    <td style="color: #50fa7b; padding: 8px;">{icon_img} SVG &amp; Media</td>
    <td style="color: #ffffff; padding: 8px;">Inline SVG rendering, drag-and-drop images,
      tables with styling</td>
  </tr>
</table>

{divider}

<h2 style="color: {green};">Inline SVG Chart</h2>
<p style="color: {dim}; font-size: 12px;">Charts rendered natively via resvg:</p>
{chart}

{divider}

<h2 style="color: {orange};">Getting Started</h2>
<ol>
  <li style="color: {fg}; margin-bottom: 6px;">Use the <b style="color: {pink};">toolbar</b>
    above to format selected text</li>
  <li style="color: {fg}; margin-bottom: 6px;">Keyboard shortcuts: <b style="color: {cyan};">
    Ctrl+B</b> bold, <b style="color: {cyan};">Ctrl+I</b> italic,
    <b style="color: {cyan};">Ctrl+U</b> underline</li>
  <li style="color: {fg}; margin-bottom: 6px;">Load templates from the
    <b style="color: {purple};">Templates</b> window</li>
  <li style="color: {fg}; margin-bottom: 6px;">Open and save HTML files with the
    toolbar buttons</li>
</ol>

{stars}

<p style="color: {dim}; font-style: italic; font-size: 11px; margin-top: 16px;">
Powered by osz-htmledit — a GTK4 rich text editor with full HTML5 &amp; SVG support.</p>
</div></body>'''.format(
    bg=BG, fg=FG, purple=PURPLE, pink=PINK, cyan=CYAN, green=GREEN,
    orange=ORANGE, dim=DIM, hdr=HDR,
    logo=SVG_LOGO, badge=SVG_BADGE, wave=SVG_WAVE, divider=SVG_DIVIDER,
    chart=SVG_CHART, stars=SVG_STARS,
    icon_code=SVG_ICON_CODE, icon_pen=SVG_ICON_PEN, icon_img=SVG_ICON_IMG,
)

TEMPLATE_NEWSLETTER = '''<body style="background-color: {bg}; padding: 16px;">
<div style="font-family: Georgia, serif; color: {fg}; max-width: 560px;">
<div style="text-align: center; padding: 12px 0;">
{wave}
<h1 style="color: {pink}; font-size: 26px; margin: 8px 0;">The Weekly Digest</h1>
<p style="color: {dim}; font-size: 13px;">Issue #42 &bull; March 2026</p>
</div>
<hr style="border-color: #333355;">

<h2 style="color: {cyan};">Top Story</h2>
<p style="line-height: 1.7; color: {fg};">The latest release of the terminal framework
introduces a groundbreaking HTML editor widget. This rich-text component supports
inline SVG rendering, table editing, and full CSS styling — all embedded directly
within the terminal environment.</p>

{divider}

<h2 style="color: {green};">This Week in Numbers</h2>
{chart}
<p style="color: {dim}; font-size: 11px; margin-top: 4px;">Data refreshed weekly.</p>

{divider}

<h2 style="color: {orange};">Quick Links</h2>
<ul>
  <li style="margin-bottom: 6px; color: {fg};"><span style="color: {cyan};">Documentation</span>
    — Getting started guide</li>
  <li style="margin-bottom: 6px; color: {fg};"><span style="color: {cyan};">API Reference</span>
    — Full method listing</li>
  <li style="margin-bottom: 6px; color: {fg};"><span style="color: {cyan};">Examples</span>
    — Sample code &amp; demos</li>
</ul>

<hr style="border-color: #333355;">
<p style="color: {dim}; font-size: 11px; text-align: center;">
You received this because you subscribed.
<span style="color: {pink};">Unsubscribe</span></p>
</div></body>'''.format(
    bg=BG, fg=FG, pink=PINK, cyan=CYAN, green=GREEN, orange=ORANGE,
    dim=DIM, wave=SVG_WAVE, divider=SVG_DIVIDER, chart=SVG_CHART,
)

TEMPLATE_REPORT = '''<body style="background-color: {bg}; padding: 16px;">
<div style="font-family: sans-serif; color: {fg};">
{logo}
<h1 style="color: {purple};">Project Status Report</h1>
<p style="color: {dim};">Generated: March 5, 2026</p>
{wave}

<h2 style="color: {pink};">Summary</h2>
<table border="1" cellpadding="10" cellspacing="0"
  style="border-collapse: collapse; border-color: #444466; width: 95%;">
  <tr style="background-color: #1e2a4a;">
    <th style="color: #e8b4f8;">Metric</th>
    <th style="color: #e8b4f8;">Value</th>
    <th style="color: #e8b4f8;">Status</th>
  </tr>
  <tr style="background-color: #141828;">
    <td style="color: #ffffff;">Tasks Completed</td>
    <td style="color: #50fa7b; text-align: center;"><b>47 / 52</b></td>
    <td style="color: #50fa7b; text-align: center;">On Track</td>
  </tr>
  <tr style="background-color: #1a2040;">
    <td style="color: #ffffff;">Code Coverage</td>
    <td style="color: #8be9fd; text-align: center;"><b>94.2%</b></td>
    <td style="color: #50fa7b; text-align: center;">Excellent</td>
  </tr>
  <tr style="background-color: #141828;">
    <td style="color: #ffffff;">Open Issues</td>
    <td style="color: #ffb86c; text-align: center;"><b>8</b></td>
    <td style="color: #ffb86c; text-align: center;">Needs Attention</td>
  </tr>
  <tr style="background-color: #1a2040;">
    <td style="color: #ffffff;">Sprint Velocity</td>
    <td style="color: #ff79c6; text-align: center;"><b>38 pts</b></td>
    <td style="color: #50fa7b; text-align: center;">Above Average</td>
  </tr>
</table>

{divider}

<h2 style="color: {cyan};">Performance</h2>
{chart}

{divider}

<h2 style="color: {green};">Next Steps</h2>
<ol>
  <li style="color: {fg}; margin-bottom: 6px;">Finalize API documentation</li>
  <li style="color: {fg}; margin-bottom: 6px;">Address remaining code review comments</li>
  <li style="color: {fg}; margin-bottom: 6px;">Performance optimization pass</li>
  <li style="color: {fg}; margin-bottom: 6px;">Release candidate testing</li>
</ol>

{stars}
</div></body>'''.format(
    bg=BG, fg=FG, purple=PURPLE, pink=PINK, cyan=CYAN, green=GREEN,
    orange=ORANGE, dim=DIM, hdr=HDR,
    logo=SVG_LOGO, wave=SVG_WAVE, divider=SVG_DIVIDER, chart=SVG_CHART,
    stars=SVG_STARS,
)


# ── State ─────────────────────────────────────────────────────────────────────
preview_open = False
templates_open = False
current_file = None
waiting_for_save = False
waiting_for_preview = False

# ── Build UI ──────────────────────────────────────────────────────────────────
os.system('clear')
fd = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)

try:
    P("id:root;layout:vertical;spacing:0;margin:0;width:680;bg_color:{}".format(BG))
    W("type:titlebar;id:title;title:HTML Editor;panel:root")

    # ── Format Toolbar ────────────────────────────────────────────────────
    P("id:tb;layout:horizontal;spacing:2;margin:3;panel:root;bg_color:{}".format(TB))

    W("type:button;id:btn_bold;label:B;tooltip:Bold (Ctrl+B);panel:tb;width:30")
    W("type:button;id:btn_italic;label:I;tooltip:Italic (Ctrl+I);panel:tb;width:30")
    W("type:button;id:btn_underline;label:U;tooltip:Underline (Ctrl+U);panel:tb;width:30")
    W("type:separator;panel:tb")
    W("type:button;id:btn_bullet;label:• List;tooltip:Bullet list;panel:tb")
    W("type:button;id:btn_numbered;label:1. List;tooltip:Numbered list;panel:tb")
    W("type:separator;panel:tb")
    W("type:button;id:btn_left;label:Left;tooltip:Align left;panel:tb")
    W("type:button;id:btn_center;label:Center;tooltip:Align center;panel:tb")
    W("type:button;id:btn_right;label:Right;tooltip:Align right;panel:tb")
    W("type:separator;panel:tb")
    W("type:button;id:btn_hr;label:HR;tooltip:Horizontal rule;panel:tb")
    W("type:button;id:btn_clear;label:Tx;tooltip:Remove formatting;panel:tb;width:30")
    W("type:separator;panel:tb")
    W("type:button;id:btn_undo;label:Undo;tooltip:Undo (Ctrl+Z);panel:tb")
    W("type:button;id:btn_redo;label:Redo;tooltip:Redo (Ctrl+Shift+Z);panel:tb")
    # spacer
    W("type:label;id:sp;label:;panel:tb;hexpand:true")
    # file ops
    W("type:button;id:btn_new;label:New;tooltip:New document;panel:tb")
    W("type:button;id:btn_open;label:Open;tooltip:Open HTML file;panel:tb")
    W("type:button;id:btn_save;label:Save;tooltip:Save HTML file;panel:tb")
    W("type:separator;panel:tb")
    W("type:button;id:btn_templates;label:Templates;tooltip:Template gallery;panel:tb")
    W("type:button;id:btn_preview;label:Preview;tooltip:Preview in window;panel:tb")

    # ── Editor (base64-encoded HTML) ──────────────────────────────────────
    W("type:htmledit;id:editor;html64:{};width:660;height:460;panel:root;hexpand:true;vexpand:true".format(
        b64(TEMPLATE_WELCOME)))

    # ── Status Bar ────────────────────────────────────────────────────────
    P("id:sbar;layout:horizontal;spacing:8;margin:3;panel:root;bg_color:{}".format(TB))
    W("type:label;id:status;label:Ready;panel:sbar;fg_color:{}".format(DIM))
    W("type:label;id:sp2;label:;panel:sbar;hexpand:true")
    W("type:label;id:file_lbl;label:Untitled;panel:sbar;fg_color:{}".format(PURPLE))

    # ── Preview Window (hidden) ───────────────────────────────────────────
    WIN("id:preview_win;title:HTML Preview;width:600;height:500;modal:false")
    P("id:pw_body;layout:vertical;spacing:4;margin:8;panel:preview_win;bg_color:{}".format(CARD))
    W("type:htmledit;id:preview_ed;html64:{};width:580;height:420;editable:false;"
      "panel:pw_body;hexpand:true;vexpand:true".format(
        b64('<body style="background-color: {}; color: {}; padding: 12px;">'
            '<p>Preview will appear here</p></body>'.format(BG, DIM))))
    P("id:pw_btns;layout:horizontal;spacing:8;panel:pw_body;bg_color:{}".format(CARD))
    W("type:button;id:pw_close;label:Close;panel:pw_btns")
    WW("id:preview_win;visible:false")

    # ── Templates Window (hidden) ─────────────────────────────────────────
    WIN("id:tpl_win;title:Template Gallery;width:280;height:320;modal:false")
    P("id:tw_body;layout:vertical;spacing:8;margin:12;panel:tpl_win;bg_color:{}".format(CARD))
    W("type:label;id:tw_title;label:Choose a Template;panel:tw_body;fg_color:{}".format(PURPLE))
    W("type:separator;panel:tw_body")
    W("type:button;id:tpl_welcome;label:Welcome Page;panel:tw_body")
    W("type:button;id:tpl_newsletter;label:Newsletter;panel:tw_body")
    W("type:button;id:tpl_report;label:Project Report;panel:tw_body")
    W("type:button;id:tpl_blank;label:Blank Document;panel:tw_body")
    W("type:separator;panel:tw_body")
    W("type:button;id:tw_close;label:Close;panel:tw_body")
    WW("id:tpl_win;visible:false")

    # ── Event Loop ────────────────────────────────────────────────────────
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
        # Split on unescaped ; only (\; is an escaped semicolon, \\\; is escaped backslash + delimiter)
        evt_tmp = evt.replace('\\\\', '\x01').replace('\\;', '\x02')
        for p in evt_tmp.split(';'):
            p = p.replace('\x01', '\\\\').replace('\x02', '\\;')
            if ':' in p:
                k, v = p.split(':', 1)
                parts[k] = v

        eid    = parts.get('id', '')
        action = parts.get('action', '')
        value  = parts.get('value', '')

        # Close
        if eid == 'title' and action == 'close':
            break

        # ── Formatting ────────────────────────────────────────────────
        fmt_map = {
            'btn_bold':      ('bold',          'Bold toggled'),
            'btn_italic':    ('italic',        'Italic toggled'),
            'btn_underline': ('underline',     'Underline toggled'),
            'btn_bullet':    ('bullet',        'Bullet list'),
            'btn_numbered':  ('numbered',      'Numbered list'),
            'btn_left':      ('align_left',    'Left aligned'),
            'btn_center':    ('align_center',  'Center aligned'),
            'btn_right':     ('align_right',   'Right aligned'),
            'btn_hr':        ('hr',            'Rule inserted'),
            'btn_clear':     ('remove_format', 'Formatting cleared'),
            'btn_undo':      ('undo',          'Undo'),
            'btn_redo':      ('redo',          'Redo'),
        }
        if eid in fmt_map and action == 'clicked':
            act, msg = fmt_map[eid]
            WU("id:editor;action:{}".format(act))
            WU("id:status;text:{}".format(msg))
            continue

        # ── New ───────────────────────────────────────────────────────
        if eid == 'btn_new' and action == 'clicked':
            blank = '<body style="background-color: {}; color: {}; padding: 12px;"><p></p></body>'.format(BG, FG)
            WU("id:editor;html64:{}".format(b64(blank)))
            current_file = None
            WU("id:file_lbl;text:Untitled")
            WU("id:status;text:New document")
            T("msg:New document;duration:1500")
            continue

        # ── Open ──────────────────────────────────────────────────────
        if eid == 'btn_open' and action == 'clicked':
            FD("open;id:open_dlg;title:Open HTML;filter:*.html")
            continue

        if eid == 'open_dlg' and action == 'selected' and value:
            try:
                with open(value, 'r') as f:
                    html = f.read()
                WU("id:editor;html64:{}".format(b64(html)))
                current_file = value
                WU("id:file_lbl;text:{}".format(os.path.basename(value)))
                WU("id:status;text:Opened")
                T("msg:File opened;duration:1500")
            except Exception as e:
                A("id:err;title:Error;body:{}".format(str(e)))
            continue

        # ── Save ──────────────────────────────────────────────────────
        if eid == 'btn_save' and action == 'clicked':
            waiting_for_save = True
            waiting_for_preview = False
            if not current_file:
                FD("save;id:save_dlg;title:Save HTML;default:document.html;filter:*.html")
            else:
                GV("id:editor")
            continue

        if eid == 'save_dlg' and action == 'selected' and value:
            current_file = value
            GV("id:editor")
            continue

        if eid == 'editor' and action == 'value':
            html = value.replace("\\n", "\n").replace("\\;", ";").replace("\\\\", "\\")
            if waiting_for_save and current_file:
                try:
                    with open(current_file, 'w') as f:
                        f.write(html)
                    WU("id:file_lbl;text:{}".format(os.path.basename(current_file)))
                    WU("id:status;text:Saved")
                    T("msg:Saved;duration:1500")
                except Exception as e:
                    A("id:err;title:Error;body:{}".format(str(e)))
                waiting_for_save = False
            elif waiting_for_preview:
                WU("id:preview_ed;html64:{}".format(b64(html)))
                waiting_for_preview = False
            continue

        # ── Preview ───────────────────────────────────────────────────
        if eid == 'btn_preview' and action == 'clicked':
            if not preview_open:
                waiting_for_preview = True
                waiting_for_save = False
                GV("id:editor")
                WW("id:preview_win;visible:true")
                preview_open = True
                WU("id:status;text:Preview opened")
            else:
                WW("id:preview_win;visible:false")
                preview_open = False
                WU("id:status;text:Preview closed")
            continue

        if eid == 'pw_close' and action == 'clicked':
            WW("id:preview_win;visible:false")
            preview_open = False
            continue
        if eid == 'preview_win' and action == 'close':
            preview_open = False
            continue

        # ── Templates ─────────────────────────────────────────────────
        if eid == 'btn_templates' and action == 'clicked':
            if not templates_open:
                WW("id:tpl_win;visible:true")
                templates_open = True
            else:
                WW("id:tpl_win;visible:false")
                templates_open = False
            continue

        if eid == 'tw_close' and action == 'clicked':
            WW("id:tpl_win;visible:false")
            templates_open = False
            continue
        if eid == 'tpl_win' and action == 'close':
            templates_open = False
            continue

        tpl_map = {
            'tpl_welcome':    (TEMPLATE_WELCOME,    'Welcome Page'),
            'tpl_newsletter': (TEMPLATE_NEWSLETTER, 'Newsletter'),
            'tpl_report':     (TEMPLATE_REPORT,     'Project Report'),
            'tpl_blank':      ('<body style="background-color: {}; color: {}; padding: 12px;">'
                               '<p></p></body>'.format(BG, FG), 'Blank'),
        }
        if eid in tpl_map and action == 'clicked':
            html, name = tpl_map[eid]
            WU("id:editor;html64:{}".format(b64(html)))
            current_file = None
            WU("id:file_lbl;text:{} (template)".format(name))
            WU("id:status;text:Loaded: {}".format(name))
            T("msg:Template loaded;duration:1500")
            continue

finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
