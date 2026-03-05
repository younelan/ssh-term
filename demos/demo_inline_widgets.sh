#!/bin/bash
# ─────────────────────────────────────────────────────────
# SmartTerm Widget Demo
# Demonstrates all supported inline GTK widgets embedded
# directly inside the terminal via our OSC 1337 Widget= protocol.
# ─────────────────────────────────────────────────────────

# Helper: send an OSC 1337 Widget command
widget() {
    printf '\033]1337;Widget=%s\007' "$1"
}

clear
echo "╔══════════════════════════════════════════════════════════╗"
echo "║           SmartTerm Inline Widget Demo                  ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

# ── Button ──────────────────────────────────────────────
echo -n "  Button:       "
widget "type:button;id:btn_hello;label:Say Hello"
echo -n "  "
widget "type:button;id:btn_world;label:Click Me!"
echo ""
echo ""

# ── Text Entry ──────────────────────────────────────────
echo -n "  Text Input:   "
widget "type:entry;id:input_name;placeholder:Enter your name...;width:30"
echo ""
echo ""

# ── Checkbox ────────────────────────────────────────────
echo -n "  Checkboxes:   "
widget "type:checkbox;id:chk_a;label:Option A"
echo -n "  "
widget "type:checkbox;id:chk_b;label:Option B;checked:true"
echo -n "  "
widget "type:checkbox;id:chk_c;label:Option C"
echo ""
echo ""

# ── Switch / Toggle ─────────────────────────────────────
echo -n "  Switch:       "
widget "type:switch;id:sw_dark;active:false"
echo -n "  Dark Mode  "
echo -n "  "
widget "type:switch;id:sw_notify;active:true"
echo -n "  Notifications"
echo ""
echo ""

# ── Dropdown / Select ───────────────────────────────────
echo -n "  Dropdown:     "
widget "type:dropdown;id:dd_color;items:Red|Green|Blue|Yellow|Purple"
echo ""
echo ""

# ── Slider / Scale ──────────────────────────────────────
echo -n "  Slider:       "
widget "type:slider;id:sl_volume;min:0;max:100;value:50;width:250"
echo ""
echo ""

# ── Spin Button ─────────────────────────────────────────
echo -n "  Spin Button:  "
widget "type:spinbutton;id:spin_qty;min:1;max:99;value:5;step:1"
echo ""
echo ""

# ── Progress Bar ────────────────────────────────────────
echo -n "  Progress:     "
widget "type:progressbar;id:pb1;value:75;width:250;text:75% Complete"
echo ""
echo ""

# ── Labels ──────────────────────────────────────────────
echo -n "  Labels:       "
widget "type:label;id:lbl1;text:Status: OK"
echo -n "  "
widget "type:label;id:lbl2;text:Welcome to SmartTerm"
echo ""
echo ""

echo "──────────────────────────────────────────────────────────"
echo "  Interact with the widgets above!"
echo "  Events are sent back to the terminal as OSC 1337"
echo "  WidgetEvent responses."
echo ""
echo "  Listening for widget events... (press Ctrl+C to quit)"
echo "──────────────────────────────────────────────────────────"
echo ""

# Read and display widget events from stdin
# (The terminal emulator writes OSC responses back as input)
while IFS= read -r -n 1 ch; do
    if [[ "$ch" == $'\033' ]]; then
        # Start of an escape sequence — read until BEL (\x07)
        seq=""
        while IFS= read -r -n 1 c; do
            [[ "$c" == $'\007' ]] && break
            seq+="$c"
        done
        if [[ "$seq" == *"WidgetEvent"* ]]; then
            echo "  📨 Event: ${seq#*WidgetEvent=}"
        fi
    fi
done
