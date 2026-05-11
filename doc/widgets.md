# Terminal Widget System

The terminal supports embedding interactive GTK widgets and complex layouts directly in the text buffer or in detached panels. This is achieved using the `OSC 1337` terminal sequence.

## Protocol Syntax

All widget commands use the following format:
`ESC ] 1337 ; Command=key1:value1;key2:value2;... ST`

- **ESC**: `\x1b`
- **ST**: `\x07` (Bell) or `\x1b\\` (String Terminator)
- **Command**: One of `Panel`, `Widget`, or `WidgetUpdate`.
- **Props**: Semicolon-separated `key:value` pairs.

### Example: Create a Button
```bash
echo -e "\x1b]1337;Widget=type:button;id:btn1;label:Click Me\x07"
```

---

## Commands

### 1. `Panel`
Creates a container/layout. Panels can be nested or placed inline at the current cursor position.

**Properties:**
- `id`: Unique identifier (required for nesting).
- `layout`: `vertical` (default), `horizontal`, or `grid`.
- `panel`: Parent panel ID to nest inside. If omitted, it's placed inline.
- `title`: If provided, the panel is wrapped in a labeled Frame.
- `width` / `height`: Size requests in pixels.
- `spacing`: Pixels between children.
- `margin`: Internal padding.
- `expand`: `true` to fill available space.
- `row`, `col`, `colspan`, `rowspan`: Positioning if the parent is a `grid`.

### 2. `Widget`
Creates an interactive UI component.

**Properties:**
- `type`: The widget type (see [Available Widgets](#available-widgets)).
- `id`: Unique identifier for events and updates.
- `panel`: ID of the panel to place the widget in. If omitted, it's placed inline.
- `expand`: `true` to fill available space.
- `row`, `col`, `colspan`, `rowspan`: Grid positioning.

### 3. `WidgetUpdate`
Updates properties of an existing widget or panel.

**General Properties:**
- `id`: Target widget ID.
- `text`: Updates text/label (Label, Button, Entry, TextView).
- `value`: Updates numeric value (0-100 for Progress, Scale, Spin, Level).
- `sensitive`: `true`/`false` to enable/disable.
- `visible`: `true`/`false` to show/hide.

---

## Interaction (Widget Events)

When a user interacts with a widget, the terminal sends a response sequence back to the PTY's `stdin`:

`ESC ] 1337 ; WidgetEvent=id:ID;action:ACTION;value:VALUE ST`

**Common Actions:**
- `clicked`: Button pressed.
- `submit`: Entry activated (Enter key).
- `toggled`: Checkbox/Switch state changed.
- `selected`: Dropdown, Radio, or Calendar selection.
- `changed`: Slider or SpinButton value changed.
- `activated`: MenuItem clicked.

---

## Available Widgets

### Basic Components
| Type | Properties | Events |
| :--- | :--- | :--- |
| `button` | `label` | `clicked` |
| `label` | `text`, `class` (CSS) | - |
| `entry` | `placeholder`, `width` | `submit` (returns text) |
| `password` | `placeholder` | `submit` (returns text) |
| `checkbox` | `label`, `checked` | `toggled` (`true`/`false`) |
| `radio` | `label`, `group` | `selected` (`true`) |
| `switch` | `active` | `toggled` (`true`/`false`) |

### Selection & Input
| Type | Properties | Events |
| :--- | :--- | :--- |
| `dropdown` | `items` (csv), `selected` (idx) | `selected` (text) |
| `slider` | `min`, `max`, `value`, `width` | `changed` (number) |
| `spin` | `min`, `max`, `value`, `step` | `changed` (number) |
| `calendar` | - | `selected` (YYYY-MM-DD) |
| `color` | `value` (hex), `alpha` (bool) | `selected` (hex) |

### Visuals
| Type | Properties | Updates |
| :--- | :--- | :--- |
| `progress` | `value` (0-100), `text` | `value` |
| `level` | `min`, `max`, `value` | `value` |
| `image` | `path`, `width`, `height` | - |
| `picturebox`| `path`, `data` (base64) | `path`, `data` |

### Complex Layouts
- `expander`: A collapsible panel. `label`, `expanded`.
- `textview`: Multi-line text area. `text`, `editable`, `wrap`. Update `append` to add lines.
- `notebook`: Tabbed container. `tabpos` (`top`, `bottom`, `left`, `right`).
- `tab`: Adds a page to a notebook. `notebook` (id), `label`. Event: `switched`.
- `splitview` / `paned`: A resizable split container. `layout` (`vertical` or `horizontal`), `pos` (divider position).
    > [!NOTE]
    > Creating a `splitview` with `id:myview` automatically creates two panels: `myview-start` and `myview-end`. Use these IDs as the `panel` property for other widgets to place them in the split panes.

### Data Views (Tree & List)
| Type | Properties | Events |
| :--- | :--- | :--- |
| `treeview` | `width`, `height` | `selected` (returns row text) |
| `listview` | `cols` (pipe-separated), `widths` (pipe-separated), `width`, `height` | `selected` (pipe-separated row values) |

**Update Actions for Data Views:**
- `action:addrow`: Adds a row to a list or tree.
    - For `listview`: Use `cols:val1|val2|...`.
    - For `treeview`: Use `label:Text`, `rowid:ID` (optional), `parent:PARENT_ID` (optional for nesting).
- `action:clear`: Removes all rows from the store.
- `action:expand_all`: Expands all nodes in a `treeview`.

### Menus & Toolbars
- `menubar`: Container for top-level menus.
- `menu`: A dropdown menu. `label`, `menubar:ID` or `menu:PARENT_ID`.
- `menuitem`: A clickable item in a menu. `label`, `menu:ID`. Event: `activated`.
- `menucheck`: A toggle item in a menu. `label`, `menu:ID`, `checked`. Event: `toggled`.
- `toolbar`: Horizontal bar for icons.
- `toolbutton`: `label`, `icon`, `tooltip`. Event: `clicked`.
- `tooltoggle`: `label`, `icon`, `active`, `tooltip`. Event: `toggled`.

### Specialized
- `close`: A destructive button that sends `Ctrl+C` (ETX) to the PTY and then an event. `label`.

---

## Advanced: Grid Layout Example

```bash
# Create a 2x2 grid container
echo -e "\x1b]1337;Panel=id:mygrid;layout:grid;spacing:10;title:Dashboard\x07"

# Place items in the grid
echo -e "\x1b]1337;Widget=type:label;panel:mygrid;row:0;col:0;text:Status:\x07"
echo -e "\x1b]1337;Widget=type:progress;id:p1;panel:mygrid;row:0;col:1;value:45\x07"
echo -e "\x1b]1337;Widget=type:button;panel:mygrid;row:1;col:0;colspan:2;label:Reset System\x07"
```
