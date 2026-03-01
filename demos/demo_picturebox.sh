#!/bin/bash

# Create two offline PNG images using python
python3 -c "import struct; import zlib
def make_png(width, height, color):
    raw = bytearray()
    for _ in range(height):
        raw.append(0)
        for _ in range(width): raw.extend(color)
    idat = zlib.compress(raw)
    def chunk(type_, data):
        return struct.pack('>I', len(data)) + type_ + data + struct.pack('>I', zlib.crc32(type_ + data) & 0xffffffff)
    png = b'\x89PNG\r\n\x1a\n'
    png += chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
    png += chunk(b'IDAT', idat)
    png += chunk(b'IEND', b'')
    return png
open('/tmp/demo_img1.png', 'wb').write(make_png(200, 200, (255, 60, 60)))
open('/tmp/demo_img2.png', 'wb').write(make_png(200, 200, (60, 60, 255)))
"

widget() { printf "\033]1337;Widget=%s\007" "$1"; }
widget_update() { printf "\033]1337;WidgetUpdate=%s\007" "$1"; }

clear
echo "=========================================="
echo "         Picturebox Widget Demo           "
echo "=========================================="
echo ""

echo -n "   "
# Output the initial picture widget
widget "type:picturebox;id:mypic;width:200;height:200;path:/tmp/demo_img1.png"
echo ""
echo ""

echo -n "   "
# Output the two toggle buttons
widget "type:button;id:btn_img1;label:Show Red Image"
echo -n "  "
widget "type:button;id:btn_img2;label:Show Blue Image"
echo ""
echo ""

echo "Listening for button clicks (Press Ctrl+C to exit)..."

stty -echo
trap 'stty sane; exit 0' INT TERM EXIT

# Read OSC 1337 WidgetEvent sequences from stdin
while IFS= read -r -n1 ch; do
    if [[ "$ch" == $'\033' ]]; then
        seq=""
        while IFS= read -r -n1 c; do
            [[ "$c" == $'\007' ]] && break
            seq+="$c"
        done
        if [[ "$seq" == *"WidgetEvent"* ]]; then
            evt="${seq#*WidgetEvent=}"
            ev_id=""; ev_action=""
            IFS=';' read -ra parts <<< "$evt"
            for p in "${parts[@]}"; do
                case "$p" in
                    id:*)     ev_id="${p#id:}" ;;
                    action:*) ev_action="${p#action:}" ;;
                esac
            done
            if [[ "$ev_action" == "clicked" ]]; then
                case "$ev_id" in
                    btn_img1) widget_update "id:mypic;path:/tmp/demo_img1.png" ;;
                    btn_img2) widget_update "id:mypic;path:/tmp/demo_img2.png" ;;
                esac
            fi
        fi
    fi
done
