use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box, Button, Orientation, ScrolledWindow, TextView, Image};
use osz_htmledit::OHtmlEdit;
use std::cell::Cell;
use std::rc::Rc;

fn main() {
    let app = Application::builder()
        .application_id("org.example.OHtmlEditDemo")
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("OSZ HTML Editor Demo")
        .default_width(600)
        .default_height(500)
        .build();

    let vbox = Box::new(Orientation::Vertical, 6);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);

    let editor = Rc::new(OHtmlEdit::new());
    editor.connect_undo_signals();

    // Toolbar
    let toolbar = Box::new(Orientation::Horizontal, 4);
    
    let btn_bold = Button::builder().child(&Image::from_file("res/icon_bold.png")).tooltip_text("Bold (Ctrl+B)").build();
    let btn_italic = Button::builder().child(&Image::from_file("res/icon_italic.png")).tooltip_text("Italic (Ctrl+I)").build();
    let btn_underline = Button::builder().child(&Image::from_file("res/icon_underline.png")).tooltip_text("Underline (Ctrl+U)").build();
    // List type dropdown (bullet, numbered)
    let list_popover = gtk::Popover::new();
    let list_vbox = Box::new(Orientation::Vertical, 2);
    list_vbox.set_margin_top(4);
    list_vbox.set_margin_bottom(4);
    list_vbox.set_margin_start(4);
    list_vbox.set_margin_end(4);
    let btn_bullet_item = Button::builder().label("\u{2022} Bullet List").build();
    btn_bullet_item.set_has_frame(false);
    let btn_numbered_item = Button::builder().label("1. Numbered List").build();
    btn_numbered_item.set_has_frame(false);
    list_vbox.append(&btn_bullet_item);
    list_vbox.append(&btn_numbered_item);
    list_popover.set_child(Some(&list_vbox));

    let btn_list = gtk::MenuButton::builder()
        .child(&Image::from_file("res/icon_bullet.png"))
        .tooltip_text("List")
        .popover(&list_popover)
        .build();

    let btn_align_left = Button::builder().child(&Image::from_file("res/icon_align_left.png")).tooltip_text("Align Left").build();
    let btn_align_center = Button::builder().child(&Image::from_file("res/icon_align_center.png")).tooltip_text("Align Center").build();
    let btn_align_right = Button::builder().child(&Image::from_file("res/icon_align_right.png")).tooltip_text("Align Right").build();
    let btn_align_justify = Button::builder().child(&Image::from_file("res/icon_justify.png")).tooltip_text("Justify").build();

    let btn_indent_decrease = Button::builder().child(&Image::from_file("res/icon_indent_left.png")).tooltip_text("Decrease Indent").build();
    let btn_indent_increase = Button::builder().child(&Image::from_file("res/icon_indent_right.png")).tooltip_text("Increase Indent").build();

    let btn_undo = Button::builder().child(&Image::from_file("res/icon_undo.png")).tooltip_text("Undo (Ctrl+Z)").build();
    let btn_redo = Button::builder().child(&Image::from_file("res/icon_redo.png")).tooltip_text("Redo (Ctrl+Shift+Z)").build();
    let btn_hr = Button::builder().child(&Image::from_file("res/icon_hr.png")).tooltip_text("Insert Horizontal Rule").build();
    let btn_image = Button::builder().child(&Image::from_file("res/icon_image.png")).tooltip_text("Insert Image").build();
    let btn_link = Button::builder().label("Link").tooltip_text("Insert Link (Ctrl+K)").build();
    let btn_table = Button::builder().label("Table").tooltip_text("Insert Table").build();
    let btn_quote = Button::builder().label(">").tooltip_text("Quote").build();
    let btn_unquote = Button::builder().label("<").tooltip_text("Unquote").build();
    let btn_clear_fmt = Button::builder().label("Tx").tooltip_text("Remove Formatting (Ctrl+\\)").build();
    let btn_sig = Button::builder().label("Sig").tooltip_text("Insert Signature").build();
    let btn_emoji = Button::builder().label("\u{263A}").tooltip_text("Insert Emoji").build();

    // Guard flag to prevent color signal feedback loops during cursor tracking
    let color_updating = Rc::new(Cell::new(false));

    // Foreground color picker with pen emoji
    let fg_color_dialog = gtk::ColorDialog::new();
    let btn_color = gtk::ColorDialogButton::new(Some(fg_color_dialog));
    btn_color.set_tooltip_text(Some("Text Color"));
    let fg_box = Box::new(Orientation::Horizontal, 2);
    let fg_label = gtk::Label::new(Some("\u{1F58A}")); // 🖊 pen
    fg_box.append(&fg_label);
    fg_box.append(&btn_color);

    // Background color picker with paint bucket emoji
    let bg_color_dialog = gtk::ColorDialog::new();
    let btn_bg_color = gtk::ColorDialogButton::new(Some(bg_color_dialog));
    btn_bg_color.set_rgba(&gtk::gdk::RGBA::new(1.0, 1.0, 1.0, 1.0));
    btn_bg_color.set_tooltip_text(Some("Background Color"));
    let bg_box = Box::new(Orientation::Horizontal, 2);
    let bg_label = gtk::Label::new(Some("\u{1F3A8}")); // 🎨 palette
    bg_box.append(&bg_label);
    bg_box.append(&btn_bg_color);

    // Font family dropdown
    let font_dropdown = gtk::DropDown::from_strings(&[
        "Default", "Sans", "Serif", "Monospace",
        "Arial", "Helvetica", "Times New Roman", "Georgia",
        "Courier New", "Verdana", "Trebuchet MS", "Comic Sans MS",
    ]);
    font_dropdown.set_selected(0);
    font_dropdown.set_tooltip_text(Some("Font Family"));

    // Font size dropdown
    let size_dropdown = gtk::DropDown::from_strings(&[
        "Default", "8", "9", "10", "11", "12", "14", "16", "18",
        "20", "24", "28", "32", "36", "48", "72",
    ]);
    size_dropdown.set_selected(0);
    size_dropdown.set_tooltip_text(Some("Font Size"));

    let e_bold = editor.clone(); btn_bold.connect_clicked(move |_| e_bold.toggle_bold());
    let e_italic = editor.clone(); btn_italic.connect_clicked(move |_| e_italic.toggle_italic());
    let e_underline = editor.clone(); btn_underline.connect_clicked(move |_| e_underline.toggle_underline());
    let e_bullet = editor.clone();
    let lp1 = list_popover.clone();
    btn_bullet_item.connect_clicked(move |_| { e_bullet.insert_bullet(); lp1.popdown(); });
    let e_num = editor.clone();
    let lp2 = list_popover.clone();
    btn_numbered_item.connect_clicked(move |_| { e_num.insert_numbered_list(); lp2.popdown(); });

    let e_al_left = editor.clone(); btn_align_left.connect_clicked(move |_| e_al_left.align_left());
    let e_al_center = editor.clone(); btn_align_center.connect_clicked(move |_| e_al_center.align_center());
    let e_al_right = editor.clone(); btn_align_right.connect_clicked(move |_| e_al_right.align_right());
    let e_al_justify = editor.clone(); btn_align_justify.connect_clicked(move |_| e_al_justify.align_justify());

    let e_id_dec = editor.clone(); btn_indent_decrease.connect_clicked(move |_| e_id_dec.decrease_indent());
    let e_id_inc = editor.clone(); btn_indent_increase.connect_clicked(move |_| e_id_inc.increase_indent());

    let e_undo = editor.clone(); btn_undo.connect_clicked(move |_| e_undo.undo());
    let e_redo = editor.clone(); btn_redo.connect_clicked(move |_| e_redo.redo());
    let e_hr = editor.clone(); btn_hr.connect_clicked(move |_| e_hr.insert_hr());
    let e_link = editor.clone(); btn_link.connect_clicked(move |_| e_link.insert_link());
    let e_table = editor.clone(); btn_table.connect_clicked(move |_| e_table.show_table_dialog());
    let e_quote = editor.clone(); btn_quote.connect_clicked(move |_| e_quote.insert_quote());
    let e_unquote = editor.clone(); btn_unquote.connect_clicked(move |_| e_unquote.remove_quote());
    let e_clear = editor.clone(); btn_clear_fmt.connect_clicked(move |_| e_clear.remove_formatting());
    let e_sig = editor.clone(); btn_sig.connect_clicked(move |_| {
        e_sig.insert_signature("John Doe\nSoftware Engineer\njohn@example.com");
    });
    let e_emoji = editor.clone(); btn_emoji.connect_clicked(move |_| e_emoji.show_emoji_picker());

    let e_image = editor.clone();
    let win_clone = window.clone();
    btn_image.connect_clicked(move |_| {
        let editor_ref = e_image.clone();
        let dialog = gtk::FileDialog::new();
        let filter = gtk::FileFilter::new();
        filter.add_mime_type("image/*");
        filter.set_name(Some("Images"));
        let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        dialog.open(Some(&win_clone), gtk::gio::Cancellable::NONE, move |result| {
            if let Ok(file) = result {
                editor_ref.insert_image_from_file(&file);
            }
        });
    });

    let e_font = editor.clone();
    font_dropdown.connect_selected_notify(move |dd| {
        let families = [
            "", "Sans", "Serif", "Monospace",
            "Arial", "Helvetica", "Times New Roman", "Georgia",
            "Courier New", "Verdana", "Trebuchet MS", "Comic Sans MS",
        ];
        let idx = dd.selected() as usize;
        if idx < families.len() {
            e_font.apply_font_family(families[idx]);
        }
    });

    let e_size = editor.clone();
    size_dropdown.connect_selected_notify(move |dd| {
        let sizes: [f64; 16] = [
            0.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0,
            20.0, 24.0, 28.0, 32.0, 36.0, 48.0, 72.0,
        ];
        let idx = dd.selected() as usize;
        if idx < sizes.len() {
            e_size.apply_font_size(sizes[idx]);
        }
    });

    let e_color = editor.clone();
    let fg_guard = color_updating.clone();
    btn_color.connect_rgba_notify(move |cb| {
        if fg_guard.get() { return; }
        let rgba = cb.rgba();
        let r = (rgba.red() * 255.0) as u8;
        let g = (rgba.green() * 255.0) as u8;
        let b = (rgba.blue() * 255.0) as u8;
        let a = rgba.alpha();
        let hex = if a < 1.0 {
            format!("rgba({}, {}, {}, {:.2})", r, g, b, a)
        } else {
            format!("#{:02x}{:02x}{:02x}", r, g, b)
        };
        e_color.apply_color(&hex);
    });

    let e_bg_color = editor.clone();
    let bg_guard = color_updating.clone();
    btn_bg_color.connect_rgba_notify(move |cb| {
        if bg_guard.get() { return; }
        let rgba = cb.rgba();
        let r = (rgba.red() * 255.0) as u8;
        let g = (rgba.green() * 255.0) as u8;
        let b = (rgba.blue() * 255.0) as u8;
        let a = rgba.alpha();
        let hex = if a < 1.0 {
            format!("rgba({}, {}, {}, {:.2})", r, g, b, a)
        } else {
            format!("#{:02x}{:02x}{:02x}", r, g, b)
        };
        e_bg_color.apply_background_color(&hex);
    });

    toolbar.append(&btn_bold);
    toolbar.append(&btn_italic);
    toolbar.append(&btn_underline);
    toolbar.append(&btn_list);

    let separator1 = gtk::Separator::new(Orientation::Vertical);
    toolbar.append(&separator1);

    toolbar.append(&btn_align_left);
    toolbar.append(&btn_align_center);
    toolbar.append(&btn_align_right);
    toolbar.append(&btn_align_justify);

    let separator2 = gtk::Separator::new(Orientation::Vertical);
    toolbar.append(&separator2);

    toolbar.append(&btn_indent_decrease);
    toolbar.append(&btn_indent_increase);

    let separator3 = gtk::Separator::new(Orientation::Vertical);
    toolbar.append(&separator3);

    toolbar.append(&btn_undo);
    toolbar.append(&btn_redo);

    let separator4 = gtk::Separator::new(Orientation::Vertical);
    toolbar.append(&separator4);

    toolbar.append(&btn_hr);
    toolbar.append(&btn_image);
    toolbar.append(&btn_link);
    toolbar.append(&btn_table);

    let separator5 = gtk::Separator::new(Orientation::Vertical);
    toolbar.append(&separator5);

    toolbar.append(&btn_quote);
    toolbar.append(&btn_unquote);
    toolbar.append(&btn_clear_fmt);
    toolbar.append(&btn_sig);
    toolbar.append(&btn_emoji);

    let separator6 = gtk::Separator::new(Orientation::Vertical);
    toolbar.append(&separator6);

    toolbar.append(&font_dropdown);
    toolbar.append(&size_dropdown);
    toolbar.append(&fg_box);
    toolbar.append(&bg_box);
    vbox.append(&toolbar);

    // Update font/size/color on cursor move
    let font_dd = font_dropdown.clone();
    let size_dd = size_dropdown.clone();
    let color_btn = btn_color.clone();
    let bg_color_btn = btn_bg_color.clone();
    let cursor_guard = color_updating.clone();
    let e_cursor = editor.clone();
    editor.connect_cursor_changed(move || {
        let font_families = [
            "", "Sans", "Serif", "Monospace",
            "Arial", "Helvetica", "Times New Roman", "Georgia",
            "Courier New", "Verdana", "Trebuchet MS", "Comic Sans MS",
        ];
        let sizes: [f64; 16] = [
            0.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0,
            20.0, 24.0, 28.0, 32.0, 36.0, 48.0, 72.0,
        ];

        // Match font family
        let cur_font = e_cursor.current_font_family();
        let font_idx = if let Some(ref f) = cur_font {
            let lower = f.to_lowercase();
            font_families.iter().position(|ff| ff.to_lowercase() == lower).unwrap_or(0)
        } else { 0 };
        font_dd.set_selected(font_idx as u32);

        // Match font size
        let cur_size = e_cursor.current_font_size();
        let size_idx = if let Some(sz) = cur_size {
            sizes.iter().position(|s| (*s - sz).abs() < 0.5).unwrap_or(0)
        } else { 0 };
        size_dd.set_selected(size_idx as u32);

        // Guard: prevent set_rgba from triggering apply_color/apply_background_color
        cursor_guard.set(true);

        // Match foreground color
        if let Some(rgba) = e_cursor.current_color() {
            color_btn.set_rgba(&rgba);
        } else {
            color_btn.set_rgba(&gtk::gdk::RGBA::new(0.0, 0.0, 0.0, 1.0));
        }

        // Match background color
        if let Some(rgba) = e_cursor.current_background_color() {
            bg_color_btn.set_rgba(&rgba);
        } else {
            bg_color_btn.set_rgba(&gtk::gdk::RGBA::new(1.0, 1.0, 1.0, 1.0));
        }

        cursor_guard.set(false);
    });

    // Put editor inside scrolled window
    let editor_scroll = ScrolledWindow::new();
    let editor_widget = editor.widget();
    editor_widget.set_hexpand(true);
    editor_widget.set_vexpand(true);
    editor_scroll.set_child(Some(editor_widget));
    vbox.append(&editor_scroll);

    let get_html_btn = Button::with_label("Serialize HTML ->");
    vbox.append(&get_html_btn);

    // Provide a box to see the raw HTML equivalent
    let source_view = TextView::new();
    source_view.set_editable(false);
    source_view.set_monospace(true);
    source_view.set_wrap_mode(gtk::WrapMode::WordChar);
    
    let source_scroll = ScrolledWindow::new();
    source_scroll.set_child(Some(&source_view));
    source_scroll.set_size_request(-1, 150);
    vbox.append(&source_scroll);

    let editor_clone = editor.clone();
    let tv_buffer = source_view.buffer();
    
    editor.set_html(r##"
<body style="background: linear-gradient(to bottom, #fffdea, #e8f4f8, #fde8f0);" text="#2c3e50">
<style>
  /* CSS class selectors */
  .highlight { background-color: #ffffcc; color: #333; padding: 4px; }
  .important { font-weight: bold; color: darkred; }
  p.note { margin-left: 20px; padding: 8px; background-color: #f0f0ff; border: 1px solid #ccccff; }
  #greeting { font-size: 18pt; color: navy; font-family: Georgia, serif; }

  /* Table styling via classes */
  .email-table { width: 100%; border-collapse: collapse; }
  .email-table th { background-color: #2c3e50; color: white; padding: 10px; font-family: Arial, sans-serif; }
  .email-table td { padding: 8px; border: 1px solid #ddd; }
  .email-table .alt-row { background-color: #f9f9f9; }
  .email-table .total-row { font-weight: bold; background-color: #ecf0f1; }

  /* Typography classes */
  .serif { font-family: "Times New Roman", Times, serif; }
  .mono { font-family: "Courier New", Courier, monospace; }
  .sans { font-family: Arial, Helvetica, sans-serif; }
  .handwriting { font-family: "Comic Sans MS", cursive; font-size: 14pt; color: #555; }
  .fancy-heading { font-size: 22pt; color: #2c3e50; letter-spacing: 2px; text-transform: uppercase; font-family: Georgia, serif; }
  .badge { background-color: #e74c3c; color: white; padding: 2px 6px; font-size: 9pt; font-weight: bold; }
  .muted { color: #999; font-size: 10pt; font-style: italic; }

  /* Hover effects */
  a:hover { color: red; }
  .badge:hover { background-color: #c0392b; }
  .highlight:hover { background-color: #ffee88; }

  /* Menu styling */
  .menu-item { padding: 10px 16px; font-family: Arial, sans-serif; font-size: 11pt; border-bottom: 1px solid #e0e0e0; }
  .menu-item:hover { background-color: #eaf2f8; }
  .menu-header { background-color: #2c3e50; color: white; font-weight: bold; font-size: 12pt; padding: 12px 16px; border-bottom: 1px solid #e0e0e0; }
  .menu-active { background-color: #3498db; color: white; font-weight: bold; padding: 10px 16px; font-family: Arial, sans-serif; font-size: 11pt; border-bottom: 1px solid #e0e0e0; }
  .menu-active:hover { background-color: #2980b9; }
</style>

<!-- Section 1: Heading & Greeting -->
<h1 style="color: #2c3e50; font-family: Georgia, serif;">HTML Editor Feature Showcase</h1>
<p id="greeting">مرحبا بالعالم — Hello World!</p>
<p class="muted">A comprehensive demo of all supported HTML and CSS features.</p>

<hr width="80%" size="14" color="#2c3e50" align="center">

<!-- Section: SVG Support -->
<h2>SVG Support</h2>
<div style="display: flex; gap: 16px;">
  <div style="padding: 8px; background-color: #fafafa; border: 1px solid #eee; border-radius: 8px;">
    <svg width="180" height="180" viewBox="0 0 180 180">
      <circle cx="90" cy="90" r="85" fill="#ffeaa7"/>
      <circle cx="90" cy="90" r="80" fill="#fdcb6e" stroke="#f39c12" stroke-width="2"/>
      <circle cx="65" cy="72" r="10" fill="#2d3436"/>
      <circle cx="115" cy="72" r="10" fill="#2d3436"/>
      <circle cx="68" cy="69" r="3" fill="white"/>
      <circle cx="118" cy="69" r="3" fill="white"/>
      <path d="M60 115 Q90 145 120 115" stroke="#2d3436" stroke-width="3" fill="none" stroke-linecap="round"/>
    </svg>
  </div>
  <div style="padding: 8px; background-color: #fafafa; border: 1px solid #eee; border-radius: 8px;">
    <svg width="200" height="180" viewBox="0 0 200 160">
      <rect x="0" y="60" width="200" height="100" rx="8" fill="#74b9ff"/>
      <rect x="10" y="70" width="180" height="80" rx="4" fill="#0984e3"/>
      <polygon points="100,5 130,55 70,55" fill="#e17055"/>
      <polygon points="100,15 122,50 78,50" fill="#fab1a0"/>
      <rect x="85" y="95" width="30" height="45" rx="2" fill="#dfe6e9"/>
      <rect x="55" y="80" width="20" height="20" rx="2" fill="#dfe6e9"/>
      <rect x="125" y="80" width="20" height="20" rx="2" fill="#dfe6e9"/>
      <circle cx="160" cy="30" r="18" fill="#ffeaa7"/>
      <circle cx="30" cy="50" r="14" fill="white" opacity="0.6"/>
      <circle cx="50" cy="40" r="10" fill="white" opacity="0.4"/>
    </svg>
  </div>
  <div style="padding: 8px; background-color: #fafafa; border: 1px solid #eee; border-radius: 8px;">
    <svg width="160" height="180" viewBox="0 0 160 160">
      <rect x="10" y="130" width="140" height="20" rx="10" fill="#b2bec3"/>
      <ellipse cx="80" cy="120" rx="35" ry="12" fill="#636e72"/>
      <rect x="55" y="30" width="50" height="90" rx="6" fill="#6c5ce7"/>
      <rect x="60" y="45" width="40" height="55" rx="3" fill="#a29bfe"/>
      <circle cx="80" cy="110" r="5" fill="#2d3436"/>
      <rect x="65" y="35" width="30" height="4" rx="2" fill="#a29bfe"/>
      <line x1="72" y1="55" x2="72" y2="90" stroke="#dfe6e9" stroke-width="1"/>
      <line x1="80" y1="55" x2="80" y2="90" stroke="#dfe6e9" stroke-width="1"/>
      <line x1="88" y1="55" x2="88" y2="90" stroke="#dfe6e9" stroke-width="1"/>
      <line x1="60" y1="65" x2="100" y2="65" stroke="#dfe6e9" stroke-width="1"/>
      <line x1="60" y1="75" x2="100" y2="75" stroke="#dfe6e9" stroke-width="1"/>
      <line x1="60" y1="85" x2="100" y2="85" stroke="#dfe6e9" stroke-width="1"/>
    </svg>
  </div>
</div>

<hr>

<!-- Section 2: Font specification — attribute vs CSS -->
<h2>Fonts: Attribute vs CSS</h2>
<p><font face="Georgia" size="4" color="#8e44ad">Font via &lt;font&gt; attribute: Georgia, size 4, purple</font></p>
<p style="font-family: Georgia, serif; font-size: 14pt; color: #8e44ad;">Font via CSS style: Georgia, 14pt, purple</p>
<p><font face="Courier New" size="3" color="#2980b9">Monospace via attribute: Courier New</font></p>
<p style="font-family: 'Courier New', monospace; font-size: 12pt; color: #2980b9;">Monospace via CSS: Courier New</p>
<p class="serif">Serif font via CSS class: <b>Times New Roman</b></p>
<p class="sans">Sans-serif font via CSS class: <b>Arial / Helvetica</b></p>
<p class="mono">Monospace font via CSS class: <code>Courier New</code></p>
<p class="handwriting">Handwriting-style via CSS class (Comic Sans MS)</p>
<p><font size="6" color="green" face="monospace">Big <b>bold</b> green mono via &lt;font&gt;</font></p>

<hr>

<!-- Section 3: Rich inline formatting -->
<h2>Inline Formatting</h2>
<p>Normal, <b>bold</b>, <i>italic</i>, <u>underline</u>, <s>strikethrough</s>,
   <b><i>bold italic</i></b>, <b><u>bold underline</u></b>, <i><u>italic underline</u></i>,
   <b><i><u>all three</u></i></b>.</p>
<p><mark>Highlighted text</mark>, <small>small text</small>, <big>big text</big>,
   <code>inline code</code>, <sup>super</sup>script, <sub>sub</sub>script.</p>
<p>Water: H<sub>2</sub>O &nbsp; &nbsp; Einstein: E=mc<sup>2</sup></p>
<p><abbr title="HyperText Markup Language">HTML</abbr> and <abbr title="Cascading Style Sheets">CSS</abbr> abbreviations with tooltips.</p>
<p>Colors: <span style="color: red;">red</span>, <span style="color: #e67e22;">orange</span>,
   <span style="color: green;">green</span>, <span style="color: blue;">blue</span>,
   <span style="color: #8e44ad;">purple</span>.</p>
<p>Backgrounds: <span style="background-color: #ffcccc;">red bg</span>,
   <span style="background-color: #ccffcc;">green bg</span>,
   <span style="background-color: #ccccff;">blue bg</span>,
   <span class="badge">badge</span>.</p>

<hr>

<!-- Section 4: CSS classes and ID selectors -->
<h2>CSS Class &amp; ID Selectors</h2>
<p class="note">This paragraph uses <code>p.note</code> class: left margin, padding, light blue background, and border.</p>
<div class="highlight">Highlighted block via <code>.highlight</code> CSS class with padding.</div>
<span class="important">Important text via <code>.important</code> class.</span>
<div style="display: none;">This should NOT appear (display:none).</div>

<hr>

<!-- Section 5: Text transforms & spacing -->
<h2>Text Effects</h2>
<p style="text-transform: uppercase;">this text should be all uppercase</p>
<p style="text-transform: capitalize;">this text should be capitalized</p>
<p style="letter-spacing: 4px;">Wide letter spacing (4px)</p>
<p style="text-indent: 40px;">This paragraph has a 40px text indent on the first line. Lorem ipsum dolor sit amet, consectetur adipiscing elit.</p>
<p style="margin: 8px 24px; padding: 8px; background-color: #e8ffe8; border-left: 4px solid #27ae60;">Block with margin, padding, green background, and left accent border.</p>
<p class="fancy-heading">Fancy Heading Class</p>

<hr>

<!-- Section 6: RTL and BiDi text -->
<h2>RTL &amp; BiDirectional Text</h2>
<p style="direction: rtl; text-align: right;">هذا نص عربي يُعرض من اليمين إلى اليسار</p>
<p style="direction: rtl;">مختلط: Hello مرحبا World عالم — mixed BiDi text</p>

<hr>

<!-- Section 7: Links -->
<h2>Links</h2>
<p>Visit <a href="https://example.com">Example Website</a> or send <a href="mailto:user@example.com">an email</a> or call <a href="tel:+1234567890">+1 234 567 890</a>.</p>

<hr>

<!-- Section 8: Lists -->
<h2>Lists</h2>
<ul>
  <li>Disc bullet (default)</li>
  <li style="list-style-type: square;">Square bullet</li>
  <li style="list-style-type: circle;">Circle bullet</li>
</ul>
<ol start="5" type="a">
  <li>Starting at 'e'</li>
  <li>Next: 'f'</li>
  <li>Next: 'g'</li>
</ol>
<ol>
  <li>Ordered item 1</li>
  <li>Ordered item 2</li>
  <li>Ordered item 3</li>
</ol>
<dl><dt><b>Definition Term</b></dt><dd>The definition of this term goes here.</dd></dl>

<hr>

<!-- Section 9: Blockquote & Preformatted -->
<h2>Block Elements</h2>
<blockquote>
    First-level quote — commonly used for email reply quoting.
    <blockquote>Nested second-level quote — deeper reply.</blockquote>
</blockquote>
<pre>Preformatted   code    with   preserved   spacing
    indented line
        doubly indented</pre>
<q>Inline curly-quoted text</q>

<hr>

<!-- Section 10: Alignment -->
<h2>Text Alignment</h2>
<p style="text-align: left;">Left-aligned text (default).</p>
<p style="text-align: center;">Center-aligned text.</p>
<p style="text-align: right;">Right-aligned text.</p>
<p style="text-align: justify;">Justified text. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.</p>

<hr>

<!-- Section 11: Styled Email Table with classes -->
<h2>Styled Table (CSS Classes)</h2>
<table class="email-table" data-purpose="invoice">
  <tr><th>Item</th><th>Qty</th><th>Price</th></tr>
  <tr><td>Widget Pro</td><td style="text-align: center;">3</td><td style="text-align: right;">$29.99</td></tr>
  <tr class="alt-row"><td>Gadget Plus</td><td style="text-align: center;">1</td><td style="text-align: right;">$49.99</td></tr>
  <tr><td>Cable Kit</td><td style="text-align: center;">5</td><td style="text-align: right;">$9.99</td></tr>
  <tr class="total-row"><td colspan="2" style="text-align: right;">Total:</td><td style="text-align: right;">$189.91</td></tr>
</table>

<!-- Section 12: Classic HTML attribute table -->
<h2>Classic Table (HTML Attributes)</h2>
<table border="1" cellpadding="6" cellspacing="0" bgcolor="#fafafa" width="100%" align="center">
  <tr bgcolor="#34495e">
    <th colspan="3" style="color: white; font-family: Arial, sans-serif;">Project Status Report</th>
  </tr>
  <tr>
    <td rowspan="2" bgcolor="#eaf2f8" align="center" valign="middle" width="120"><b>Q1 2026</b></td>
    <td bgcolor="#d5f5e3" align="left"><font color="#27ae60" face="Arial">&#10004; Feature A — Complete</font></td>
    <td align="right" style="font-size: 10pt; color: #999;">Jan 15</td>
  </tr>
  <tr>
    <td bgcolor="#fdebd0"><font color="#e67e22" face="Arial">&#9998; Feature B — In Progress</font></td>
    <td align="right" style="font-size: 10pt; color: #999;">Mar 01</td>
  </tr>
  <tr>
    <td bgcolor="#eaf2f8" align="center"><b>Q2 2026</b></td>
    <td bgcolor="#fadbd8"><font color="#e74c3c" face="Arial">&#10008; Feature C — Planned</font></td>
    <td align="right" style="font-size: 10pt; color: #999;">TBD</td>
  </tr>
</table>

<!-- Section 13: Sidebar menu with hover -->
<h2>Navigation Menu (Hover)</h2>
<table cellpadding="0" cellspacing="0" width="200" style="border: 1px solid #ccc; border-collapse: collapse;">
  <tr><td class="menu-header">Navigation</td></tr>
  <tr><td class="menu-active"><a href="#" style="color: white; text-decoration: none;">Dashboard</a></td></tr>
  <tr><td class="menu-item"><a href="#" style="color: #333; text-decoration: none;">Inbox</a> <span class="badge">3</span></td></tr>
  <tr><td class="menu-item"><a href="#" style="color: #333; text-decoration: none;">Sent Mail</a></td></tr>
  <tr><td class="menu-item"><a href="#" style="color: #333; text-decoration: none;">Drafts</a></td></tr>
  <tr><td class="menu-item"><a href="#" style="color: #333; text-decoration: none;">Contacts</a></td></tr>
  <tr><td class="menu-item" style="color: #999; font-size: 9pt; border-bottom: none;">v2.1.0</td></tr>
</table>

<!-- Section 14: Nested table — typical email layout -->
<h2>Nested Table (Email Layout)</h2>
<table width="100%" cellpadding="0" cellspacing="0" bgcolor="#e0e0e0" style="border-collapse: collapse;">
  <tr>
    <td align="center" style="padding: 20px;">
      <table width="480" cellpadding="0" cellspacing="0" bgcolor="#ffffff" style="border-collapse: collapse; border: 1px solid #cccccc;">
        <tr>
          <td bgcolor="#2c3e50" style="padding: 16px; color: white; font-family: Arial, sans-serif; font-size: 18pt;" align="center">
            <b>Monthly Newsletter</b>
          </td>
        </tr>
        <tr>
          <td style="padding: 16px; font-family: Arial, sans-serif;">
            <p style="margin: 0 0 10px 0;">Dear subscriber,</p>
            <p style="margin: 0 0 10px 0;">Here are this month's highlights:</p>
            <table width="100%" cellpadding="8" cellspacing="0" border="1" style="border-collapse: collapse; border-color: #ddd;">
              <tr bgcolor="#f7f9fa">
                <th align="left" style="font-family: Arial, sans-serif;">Feature</th>
                <th align="center" style="font-family: Arial, sans-serif;">Status</th>
              </tr>
              <tr>
                <td><font face="Arial" color="#27ae60">&#10004; Table round-trip</font></td>
                <td align="center"><span style="background-color: #d5f5e3; padding: 2px 8px;">Done</span></td>
              </tr>
              <tr bgcolor="#f9f9f9">
                <td><font face="Arial" color="#e67e22">&#9998; Nested tables</font></td>
                <td align="center"><span style="background-color: #fdebd0; padding: 2px 8px;">WIP</span></td>
              </tr>
              <tr>
                <td><font face="Arial" color="#e74c3c">&#10008; Image resize</font></td>
                <td align="center"><span style="background-color: #fadbd8; padding: 2px 8px;">Planned</span></td>
              </tr>
            </table>
            <p style="margin: 12px 0 0 0; font-size: 10pt; color: #999;">You received this because you subscribed at <a href="https://example.com">example.com</a>.</p>
          </td>
        </tr>
        <tr>
          <td bgcolor="#ecf0f1" align="center" style="padding: 10px; font-size: 9pt; color: #777; font-family: Arial, sans-serif;">
            &copy; 2026 Acme Corp &bull; <a href="mailto:unsubscribe@example.com">Unsubscribe</a>
          </td>
        </tr>
      </table>
    </td>
  </tr>
</table>

<hr>

<!-- Section 14: Complex spans and data attributes -->
<h2>CSS Positioning (Round-trip)</h2>
<div style="width: 50%; float: left; padding: 8px; background-color: #ebf5fb; border: 1px solid #3498db;">
  Left-floated block at 50% width.
</div>
<div style="width: 50%; float: right; padding: 8px; background-color: #fdedec; border: 1px solid #e74c3c;">
  Right-floated block at 50% width.
</div>
<div style="clear: both;"></div>
<div style="position: relative; top: 4px; left: 20px; background-color: #f9e79f; padding: 4px; display: inline-block;">
  Relatively positioned, nudged right &amp; down.
</div>

<hr>

<!-- Section: Flexbox Layout -->
<h2>Flexbox Layout</h2>
<p style="color: #666; font-size: 10pt;">Horizontal flex with 12px gap:</p>
<div style="display: flex; gap: 12px;">
  <div style="background-color: #eaf2f8; padding: 12px; border: 1px solid #3498db; font-family: Arial, sans-serif;">
    <b>Card 1</b><br>Flex items arranged horizontally with gap.
  </div>
  <div style="background-color: #fef9e7; padding: 12px; border: 1px solid #f1c40f; font-family: Arial, sans-serif;">
    <b>Card 2</b><br>Each card is a separate editable region.
  </div>
  <div style="background-color: #fdedec; padding: 12px; border: 1px solid #e74c3c; font-family: Arial, sans-serif;">
    <b>Card 3</b><br>Background, border, padding, font.
  </div>
</div>

<p style="color: #666; font-size: 10pt;">No gap, flush cells:</p>
<div style="display: flex;">
  <div style="background-color: #2c3e50; color: white; padding: 10px; font-family: Arial, sans-serif; font-weight: bold;">Nav</div>
  <div style="background-color: #ecf0f1; padding: 10px; font-family: Arial, sans-serif;">Main content area with no gaps between cells</div>
  <div style="background-color: #bdc3c7; padding: 10px; font-family: Arial, sans-serif;">Sidebar</div>
</div>

<p style="color: #666; font-size: 10pt;">Vertical column flex:</p>
<div style="display: flex; flex-direction: column; gap: 4px;">
  <div style="padding: 8px; background-color: #d5f5e3; border-left: 4px solid #27ae60;">Vertical flex — Row 1</div>
  <div style="padding: 8px; background-color: #d6eaf8; border-left: 4px solid #2980b9;">Vertical flex — Row 2</div>
  <div style="padding: 8px; background-color: #fadbd8; border-left: 4px solid #e74c3c;">Vertical flex — Row 3</div>
</div>

<p style="color: #666; font-size: 10pt;">Flex wrap — items wrap to next row when they overflow:</p>
<div style="display: flex; flex-wrap: wrap; gap: 8px;">
  <div style="background-color: #eaf2f8; padding: 10px; border: 1px solid #3498db; width: 180px;">Wrap 1</div>
  <div style="background-color: #fef9e7; padding: 10px; border: 1px solid #f1c40f; width: 180px;">Wrap 2</div>
  <div style="background-color: #fdedec; padding: 10px; border: 1px solid #e74c3c; width: 180px;">Wrap 3</div>
  <div style="background-color: #e8daef; padding: 10px; border: 1px solid #8e44ad; width: 180px;">Wrap 4</div>
  <div style="background-color: #d5f5e3; padding: 10px; border: 1px solid #27ae60; width: 180px;">Wrap 5</div>
  <div style="background-color: #fdebd0; padding: 10px; border: 1px solid #e67e22; width: 180px;">Wrap 6</div>
</div>

<hr>

<!-- Section: CSS Grid Layout -->
<h2>CSS Grid Layout</h2>
<p style="color: #666; font-size: 10pt;">3-column grid with gap:</p>
<div style="display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 10px;">
  <div style="background-color: #eaf2f8; padding: 10px; border: 1px solid #aaa;">Grid Cell 1</div>
  <div style="background-color: #fef9e7; padding: 10px; border: 1px solid #aaa;">Grid Cell 2</div>
  <div style="background-color: #fdedec; padding: 10px; border: 1px solid #aaa;">Grid Cell 3</div>
  <div style="background-color: #e8daef; padding: 10px; border: 1px solid #aaa;">Grid Cell 4</div>
  <div style="background-color: #d5f5e3; padding: 10px; border: 1px solid #aaa;">Grid Cell 5</div>
  <div style="background-color: #fdebd0; padding: 10px; border: 1px solid #aaa;">Grid Cell 6</div>
</div>

<p style="color: #666; font-size: 10pt;">Grid with column spans (dashboard layout):</p>
<div style="display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 8px;">
  <div style="grid-column: span 3; background-color: #2c3e50; color: white; padding: 12px; font-family: Arial, sans-serif; font-size: 14pt; font-weight: bold;">Dashboard Header (span 3)</div>
  <div style="grid-column: span 2; background-color: #eaf2f8; padding: 10px; border: 1px solid #3498db;">Main chart area (span 2)</div>
  <div style="background-color: #fef9e7; padding: 10px; border: 1px solid #f1c40f;">Stats panel</div>
  <div style="background-color: #d5f5e3; padding: 10px; border: 1px solid #27ae60;">Metric A</div>
  <div style="background-color: #fadbd8; padding: 10px; border: 1px solid #e74c3c;">Metric B</div>
  <div style="background-color: #e8daef; padding: 10px; border: 1px solid #8e44ad;">Metric C</div>
  <div style="grid-column: span 3; background-color: #ecf0f1; padding: 8px; font-size: 9pt; color: #777;">Footer — spans all 3 columns</div>
</div>

<p style="color: #666; font-size: 10pt;">No-gap grid (flush tiles):</p>
<div style="display: grid; grid-template-columns: 1fr 1fr 1fr 1fr;">
  <div style="background-color: #e74c3c; color: white; padding: 8px;">Red</div>
  <div style="background-color: #3498db; color: white; padding: 8px;">Blue</div>
  <div style="background-color: #2ecc71; color: white; padding: 8px;">Green</div>
  <div style="background-color: #f39c12; color: white; padding: 8px;">Orange</div>
</div>

<hr>

<!-- Section: Headings hierarchy -->
<h3>Heading 3</h3>
<h4>Heading 4</h4>
<h5>Heading 5</h5>
<h6>Heading 6</h6>

<hr>

<!-- Section: Blockquote -->
<h2>Blockquotes</h2>
<blockquote>
  This is a simple blockquote. It should be indented with a subtle background.
</blockquote>

<p>Nested blockquotes (email reply chains):</p>
<blockquote>
  Original message from Alice.
  <blockquote>
    Bob's reply to Alice.
    <blockquote>
      Charlie's reply to Bob — third level of nesting.
    </blockquote>
  </blockquote>
</blockquote>

<hr>

<!-- Section: Code -->
<h2>Code &amp; Preformatted Text</h2>
<p>Inline code: Use <code>git commit -m "message"</code> to commit your changes.</p>
<p>Code block:</p>
<pre>
fn main() {
    println!("Hello, world!");
    let x = 42;
    if x > 0 {
        println!("positive");
    }
}
</pre>

<p>Another example with <code>HTML</code> inline and <kbd>Ctrl+C</kbd> keyboard shortcuts.</p>

<hr>

<!-- Section: Superscript / Subscript -->
<h2>Superscript &amp; Subscript</h2>
<p>The equation E = mc<sup>2</sup> changed physics forever.</p>
<p>Water is H<sub>2</sub>O. Carbon dioxide is CO<sub>2</sub>.</p>
<p>Footnote reference<sup>[1]</sup> and another<sup>[2]</sup>.</p>

<hr>

<!-- Section: CSS Text Properties -->
<h2>CSS Text Properties</h2>
<p style="font-style: italic;">This paragraph uses CSS font-style: italic.</p>
<p style="text-decoration: underline;">This paragraph uses CSS text-decoration: underline.</p>
<p style="text-decoration: line-through;">This paragraph uses CSS text-decoration: line-through.</p>
<p style="text-decoration: underline line-through;">This has both underline and line-through via CSS.</p>
<p style="line-height: 32px;">This paragraph has line-height: 32px. The extra spacing between lines should be visible when the text wraps to multiple lines in the editor view.</p>
<p style="line-height: 12px;">This paragraph has a tight line-height: 12px. Lines should be closer together than normal.</p>

<hr>

<!-- Section: Text Transform & Spacing -->
<h2>Text Transform &amp; Spacing</h2>
<p style="text-transform: uppercase;">This text is transformed to uppercase via CSS.</p>
<p style="text-transform: capitalize;">this text has each word capitalized via css text-transform.</p>
<p style="text-transform: lowercase;">THIS TEXT IS TRANSFORMED TO LOWERCASE VIA CSS.</p>
<p style="letter-spacing: 5px;">This paragraph has 5px letter-spacing for a wide look.</p>
<p style="letter-spacing: -1px;">This paragraph has tight -1px letter-spacing.</p>

<hr>

<!-- Section: Opacity -->
<h2>Opacity</h2>
<p style="opacity: 1.0;">Full opacity (1.0) — normal text.</p>
<p style="opacity: 0.6;">Reduced opacity (0.6) — partially transparent text.</p>
<p style="opacity: 0.3;">Low opacity (0.3) — very faded text.</p>
<p style="opacity: 0.6; color: #e74c3c;">Red text at 0.6 opacity.</p>
<p style="opacity: 0.5; background-color: #3498db; color: white; padding: 4px;">Blue background at 0.5 opacity.</p>

<hr>

<!-- Section: White-space -->
<h2>White-space</h2>
<p style="white-space: nowrap; background-color: #f0f0f0; padding: 4px; overflow: hidden;">This text has white-space: nowrap — it should not wrap to the next line even if it extends beyond the visible area of the editor.</p>
<pre style="background-color: #f6f8fa; padding: 8px;">Preformatted:
    Indented with spaces
    Preserves    multiple    spaces
    And line breaks</pre>

<hr>

<!-- Section: Vertical Align -->
<h2>Vertical Align</h2>
<p>Normal text <span style="vertical-align: super; font-size: 10pt;">superscript</span> and <span style="vertical-align: sub; font-size: 10pt;">subscript</span> alignment.</p>
<p>Baseline <span style="vertical-align: top; background-color: #eaf2f8;">top</span> and <span style="vertical-align: middle; background-color: #fef9e7;">middle</span> and <span style="vertical-align: bottom; background-color: #fdedec;">bottom</span> aligned.</p>
<p>Shifted up <span style="vertical-align: 5px; background-color: #d5f5e3;">+5px</span> and down <span style="vertical-align: -3px; background-color: #fadbd8;">-3px</span> from baseline.</p>

<hr>

<!-- Section: Box Shadow -->
<h2>Box Shadow</h2>
<div style="display: flex; gap: 16px;">
  <div style="padding: 16px; background-color: white; box-shadow: 2px 2px 6px rgba(0,0,0,0.3); border-radius: 6px;">Light shadow</div>
  <div style="padding: 16px; background-color: white; box-shadow: 4px 4px 12px rgba(0,0,0,0.5); border-radius: 6px;">Deeper shadow</div>
  <div style="padding: 16px; background-color: #3498db; color: white; box-shadow: 0 4px 8px rgba(52,152,219,0.4); border-radius: 6px;">Colored shadow</div>
</div>

<hr>

<!-- Section: Text Shadow -->
<h2>Text Shadow</h2>
<div style="display: flex; gap: 16px;">
  <div style="padding: 12px; font-size: 16pt; text-shadow: 2px 2px 4px rgba(0,0,0,0.3);">Shadow text</div>
  <div style="padding: 12px; font-size: 16pt; color: #e74c3c; text-shadow: 1px 1px 2px rgba(0,0,0,0.4);">Red glow</div>
  <div style="padding: 12px; font-size: 16pt; color: white; background-color: #2c3e50; text-shadow: 0 0 8px #3498db;">Neon effect</div>
</div>

<hr>

<!-- Section: Linear Gradient -->
<h2>Linear Gradient Backgrounds</h2>
<div style="display: flex; gap: 12px;">
  <div style="padding: 16px; color: white; background-image: linear-gradient(to right, #3498db, #8e44ad); border-radius: 6px;">Blue to Purple</div>
  <div style="padding: 16px; color: white; background-image: linear-gradient(135deg, #e74c3c, #f39c12); border-radius: 6px;">Red to Orange</div>
  <div style="padding: 16px; color: #333; background-image: linear-gradient(to bottom, #dfe6e9, #b2bec3); border-radius: 6px;">Subtle gray</div>
</div>

<hr>

<!-- Section: Percentage Width Examples -->
<h2>Percentage Widths</h2>

<p style="color: #666; font-size: 10pt;">HR at 50%, left-aligned:</p>
<hr width="50%" size="3" color="#e74c3c" align="left">

<p style="color: #666; font-size: 10pt;">HR at 30%, right-aligned:</p>
<hr width="30%" size="2" color="#3498db" align="right">

<p style="color: #666; font-size: 10pt;">Table at 60% width, centered:</p>
<table border="1" cellpadding="6" cellspacing="0" width="60%" align="center" bgcolor="#fafafa">
  <tr bgcolor="#2c3e50">
    <th style="color: white;">Name</th>
    <th style="color: white;">Role</th>
  </tr>
  <tr>
    <td width="40%">Alice</td>
    <td width="60%">Engineer</td>
  </tr>
  <tr>
    <td width="40%">Bob</td>
    <td width="60%">Designer</td>
  </tr>
</table>

<p style="color: #666; font-size: 10pt;">Image at 50% width:</p>
<img src="res/icon_bold.png" width="50%">

<p style="color: #666; font-size: 10pt;">Flex children with percentage widths:</p>
<div style="display: flex; gap: 8px;">
  <div style="width: 30%; padding: 10px; background-color: #eaf2f8; border: 1px solid #3498db;">30% wide</div>
  <div style="width: 50%; padding: 10px; background-color: #fef9e7; border: 1px solid #f1c40f;">50% wide</div>
  <div style="width: 20%; padding: 10px; background-color: #fdedec; border: 1px solid #e74c3c;">20% wide</div>
</div>

<hr>

<!-- Section: Email CSS Features -->
<h2>Email CSS Features</h2>

<p style="color: #666; font-size: 10pt;">Table centered with margin: 0 auto (400px wide):</p>
<table width="400" style="margin: 0 auto; border: 2px solid #3498db; border-collapse: collapse;" cellpadding="8">
  <tr bgcolor="#3498db"><th style="color: white;">Feature</th><th style="color: white;">Status</th></tr>
  <tr><td>margin: auto</td><td>Centered!</td></tr>
</table>

<p style="color: #666; font-size: 10pt;">display: none preheader (invisible):</p>
<div style="display: none; max-height: 0; overflow: hidden;">
  This preheader text should NOT be visible
</div>
<p style="color: green; font-size: 10pt;">(If you see nothing above this line, display:none works)</p>

<p style="color: #666; font-size: 10pt;">max-width: 300px centered wrapper:</p>
<table width="100%" style="max-width: 300px; margin: 0 auto; border: 1px solid #e74c3c; border-collapse: collapse;" cellpadding="8">
  <tr><td style="background-color: #fdedec;">This table is capped at 300px and centered</td></tr>
</table>

<p style="color: #666; font-size: 10pt;">Background image on table cell (local file):</p>
<table width="100%" cellpadding="20" border="0">
  <tr>
    <td style="background-image: url('silicon.png'); background-repeat: no-repeat; background-color: #f0f0f0; color: #333;">
      Cell with background image (url)
    </td>
    <td style="background-image: linear-gradient(to right, #3498db, #8e44ad); color: white;">
      Cell with gradient background
    </td>
  </tr>
</table>

</body>
"##);

    get_html_btn.connect_clicked(move |_| {
        let html = editor_clone.get_html();
        tv_buffer.set_text(&html);
    });

    window.set_child(Some(&vbox));
    window.present();
}
