use gtk4 as gtk;
use gtk::prelude::*;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;

use crate::css::{
    apply_css_cascade, apply_text_transform, apply_to_text_tag, collect_style_rules,
    format_list_marker, html_font_size_to_points, BorderStyle, CssProperties,
    ListStyleType, TextDirection, TextTransform, WhiteSpaceMode,
};

// ── Parse Context ──────────────────────────────────────────────────────────

pub struct ParseContext {
    pub css_rules: HashMap<String, String>,
    pub hover_rules: HashMap<String, String>,
    pub list_depth: i32,
    pub in_ordered_list: bool,
    pub ol_counters: Vec<i32>,
    pub in_pre: bool,
    pub display_none_depth: i32,
    pub list_style_type: Option<ListStyleType>,
    pub text_transform: Option<TextTransform>,
    pub css_rules_store: HashMap<String, String>,
    pub hover_variants: HashMap<String, String>,
    pub link_hover_tag: Option<String>,
    pub blockquote_depth: i32,
}

impl ParseContext {
    pub fn new(css_rules: HashMap<String, String>, hover_rules: HashMap<String, String>) -> Self {
        Self {
            css_rules,
            hover_rules,
            list_depth: 0,
            in_ordered_list: false,
            ol_counters: Vec::new(),
            in_pre: false,
            display_none_depth: 0,
            list_style_type: None,
            text_transform: None,
            css_rules_store: HashMap::new(),
            hover_variants: HashMap::new(),
            link_hover_tag: None,
            blockquote_depth: 0,
        }
    }
}

/// Result of parsing HTML into a buffer.
pub struct ParseResult {
    pub css_rules_store: HashMap<String, String>,
    pub hover_variants: HashMap<String, String>,
    pub link_hover_tag: Option<String>,
}

// ── Public Entry Point ─────────────────────────────────────────────────────

pub fn parse_html_to_buffer(
    view: &gtk::TextView,
    dom: &markup5ever_rcdom::RcDom,
    buffer: &gtk::TextBuffer,
) -> ParseResult {
    let mut css_rules = HashMap::new();
    let mut hover_rules = HashMap::new();
    collect_style_rules(&dom.document, &mut css_rules, &mut hover_rules);

    let mut ctx = ParseContext::new(css_rules, hover_rules);
    walk_dom(view, &dom.document, buffer, &mut ctx);
    ParseResult {
        css_rules_store: ctx.css_rules_store,
        hover_variants: ctx.hover_variants,
        link_hover_tag: ctx.link_hover_tag,
    }
}

// ── DOM Walker ─────────────────────────────────────────────────────────────

fn walk_dom(
    view: &gtk::TextView,
    node: &Handle,
    buffer: &gtk::TextBuffer,
    ctx: &mut ParseContext,
) {
    match node.data {
        NodeData::Text { ref contents } => {
            if ctx.display_none_depth > 0 {
                return;
            }
            let text = contents.borrow();
            let mut s = text.to_string();

            if !ctx.in_pre {
                // Collapse whitespace: all runs of whitespace become a single space
                let mut out = String::with_capacity(s.len());
                let mut last_space = false;
                for ch in s.chars() {
                    if ch == '\u{00A0}' {
                        // Preserve non-breaking spaces as-is
                        out.push(ch);
                        last_space = false;
                    } else if ch.is_whitespace() {
                        if !last_space {
                            out.push(' ');
                            last_space = true;
                        }
                    } else {
                        out.push(ch);
                        last_space = false;
                    }
                }
                s = out;
            } else {
                // In <pre>: normalize \r\n → \n, \r → \n, preserve all other whitespace
                s = s.replace("\r\n", "\n").replace('\r', "\n");
            }

            // Apply text-transform if active
            if let Some(tt) = ctx.text_transform {
                s = apply_text_transform(&s, tt);
            }

            if !s.is_empty() {
                let mut end_iter = buffer.end_iter();
                buffer.insert(&mut end_iter, &s);
            }
        }

        NodeData::Element { ref name, ref attrs, .. } => {
            let tag_name = name.local.to_string().to_lowercase();

            // ── Skip elements that should not produce content ──
            match tag_name.as_str() {
                "script" | "style" | "title" | "head" | "meta" | "link" | "noscript" => return,
                _ => {}
            }

            // ── Extract attributes for CSS cascade ──
            let attrs_borrow = attrs.borrow();
            let mut class_attr = None;
            let mut id_attr = None;
            let mut style_attr = None;
            let mut align_attr = None;
            let mut dir_attr = None;
            for attr in attrs_borrow.iter() {
                let aname = attr.name.local.to_string();
                match aname.as_str() {
                    "class" => class_attr = Some(attr.value.to_string()),
                    "id" => id_attr = Some(attr.value.to_string()),
                    "style" => style_attr = Some(attr.value.to_string()),
                    "align" => align_attr = Some(attr.value.to_string()),
                    "dir" => dir_attr = Some(attr.value.to_string()),
                    _ => {}
                }
            }
            drop(attrs_borrow);

            // Build extra inline CSS from HTML attributes (font, align, dir, etc.)
            let mut extra_css = String::new();
            build_extra_css_from_attrs(node, &tag_name, &mut extra_css);
            if let Some(ref align) = align_attr {
                extra_css.push_str(&format!("text-align: {};", align.to_lowercase()));
            }
            if let Some(ref dir) = dir_attr {
                extra_css.push_str(&format!("direction: {};", dir.to_lowercase()));
            }

            // Combine inline style with extra CSS from attributes
            let combined_inline = if extra_css.is_empty() {
                style_attr.clone()
            } else {
                let mut combined = extra_css;
                if let Some(ref style) = style_attr {
                    combined.push_str(style);
                }
                Some(combined)
            };

            // ── Resolve CSS cascade ──
            let css_props = apply_css_cascade(
                &tag_name,
                class_attr.as_deref(),
                id_attr.as_deref(),
                combined_inline.as_deref(),
                &ctx.css_rules,
            );

            // ── Handle display:none / visibility:hidden / opacity:0 ──
            if css_props.display_none || css_props.visibility_hidden
                || css_props.opacity == Some(0.0) {
                ctx.display_none_depth += 1;
                // Still recurse so we can properly decrement on close
                for child in node.children.borrow().iter() {
                    walk_dom(view, child, buffer, ctx);
                }
                ctx.display_none_depth -= 1;
                return;
            }

            if ctx.display_none_depth > 0 {
                return;
            }

            // ── Self-closing / special elements ──
            match tag_name.as_str() {
                "img" => {
                    insert_img_widget(view, node, buffer);
                    return;
                }
                "wbr" => {
                    let mut end_iter = buffer.end_iter();
                    buffer.insert(&mut end_iter, "\u{200B}");
                    return;
                }
                "br" => {
                    let mut end_iter = buffer.end_iter();
                    buffer.insert(&mut end_iter, "\n");
                    return;
                }
                "hr" => {
                    insert_hr_widget(view, buffer);
                    return;
                }
                _ => {}
            }

            // ── Table ──
            if tag_name == "table" {
                handle_table(view, node, buffer, ctx, &css_props);
                return;
            }

            // ── Lists ──
            if tag_name == "ul" || tag_name == "ol" {
                handle_list(view, node, buffer, ctx, &tag_name, &css_props);
                return;
            }
            if tag_name == "li" {
                handle_list_item(view, node, buffer, ctx, &css_props);
                return;
            }

            // ── <q> — curly quotes ──
            if tag_name == "q" {
                let mut end_iter = buffer.end_iter();
                buffer.insert(&mut end_iter, "\u{201C}"); // left double quote
                for child in node.children.borrow().iter() {
                    walk_dom(view, child, buffer, ctx);
                }
                let mut end_iter = buffer.end_iter();
                buffer.insert(&mut end_iter, "\u{201D}"); // right double quote
                return;
            }

            // ── <abbr> — abbreviation with tooltip ──
            if tag_name == "abbr" {
                let mut title = String::new();
                if let NodeData::Element { ref attrs, .. } = node.data {
                    for attr in attrs.borrow().iter() {
                        if attr.name.local.to_string() == "title" {
                            title = attr.value.to_string();
                        }
                    }
                }
                let start_mark = buffer.create_mark(None, &buffer.end_iter(), true);
                for child in node.children.borrow().iter() {
                    walk_dom(view, child, buffer, ctx);
                }
                let start_iter = buffer.iter_at_mark(&start_mark);
                let end_iter = buffer.end_iter();
                if start_iter != end_iter {
                    // Apply dotted underline style
                    let abbr_style_name = "abbr_style";
                    if buffer.tag_table().lookup(abbr_style_name).is_none() {
                        let tag = gtk::TextTag::builder()
                            .name(abbr_style_name)
                            .underline(gtk::pango::Underline::Low)
                            .build();
                        buffer.tag_table().add(&tag);
                    }
                    if let Some(tag) = buffer.tag_table().lookup(abbr_style_name) {
                        buffer.apply_tag(&tag, &start_iter, &end_iter);
                    }
                    // Store title for tooltip via a dynamic tag
                    if !title.is_empty() {
                        let title_tag_name = format!("abbr_title:{}", title);
                        if buffer.tag_table().lookup(&title_tag_name).is_none() {
                            let tag = gtk::TextTag::new(Some(&title_tag_name));
                            buffer.tag_table().add(&tag);
                        }
                        if let Some(tag) = buffer.tag_table().lookup(&title_tag_name) {
                            buffer.apply_tag(&tag, &start_iter, &end_iter);
                        }
                    }
                }
                buffer.delete_mark(&start_mark);
                return;
            }

            // ── <bdo> — bidirectional override ──
            if tag_name == "bdo" {
                let mut dir_val = String::new();
                if let NodeData::Element { ref attrs, .. } = node.data {
                    for attr in attrs.borrow().iter() {
                        if attr.name.local.to_string() == "dir" {
                            dir_val = attr.value.to_string().to_lowercase();
                        }
                    }
                }
                let start_mark = buffer.create_mark(None, &buffer.end_iter(), true);
                for child in node.children.borrow().iter() {
                    walk_dom(view, child, buffer, ctx);
                }
                let start_iter = buffer.iter_at_mark(&start_mark);
                let end_iter = buffer.end_iter();
                if start_iter != end_iter && !dir_val.is_empty() {
                    // Store as CSS tag for round-trip: "direction: rtl; unicode-bidi: bidi-override;"
                    let css_str = format!("direction: {}; unicode-bidi: bidi-override;", dir_val);
                    let mut bdo_props = CssProperties::default();
                    if dir_val == "rtl" {
                        bdo_props.direction = Some(TextDirection::Rtl);
                    }
                    let css_tag = create_or_get_css_tag(buffer, &bdo_props, false, &mut ctx.css_rules_store);
                    // Override the stored CSS string to include unicode-bidi
                    if let Some(tag_name) = css_tag.name() {
                        ctx.css_rules_store.insert(tag_name.to_string(), css_str);
                    }
                    buffer.apply_tag(&css_tag, &start_iter, &end_iter);
                }
                buffer.delete_mark(&start_mark);
                return;
            }

            // ── <a> — extract href for round-trip ──
            if tag_name == "a" {
                let mut href = None;
                if let NodeData::Element { ref attrs, .. } = node.data {
                    for attr in attrs.borrow().iter() {
                        if attr.name.local.to_string() == "href" {
                            href = Some(attr.value.to_string());
                        }
                    }
                }
                let start_mark = buffer.create_mark(None, &buffer.end_iter(), true);
                for child in node.children.borrow().iter() {
                    walk_dom(view, child, buffer, ctx);
                }
                let start_iter = buffer.iter_at_mark(&start_mark);
                let end_iter = buffer.end_iter();
                if start_iter != end_iter {
                    if let Some(tag) = buffer.tag_table().lookup("a") {
                        buffer.apply_tag(&tag, &start_iter, &end_iter);
                    }
                    // Store href as a link tag for serialization
                    if let Some(ref url) = href
                        && !url.is_empty() {
                            let link_tag_name = format!("link:{}", url);
                            let tag = if let Some(existing) = buffer.tag_table().lookup(&link_tag_name) {
                                existing
                            } else {
                                let new_tag = gtk::TextTag::new(Some(&link_tag_name));
                                buffer.tag_table().add(&new_tag);
                                new_tag
                            };
                            buffer.apply_tag(&tag, &start_iter, &end_iter);
                        }
                    // Apply CSS if any
                    if has_meaningful_css(&css_props) {
                        let css_tag = create_or_get_css_tag(buffer, &css_props, false, &mut ctx.css_rules_store);
                        buffer.apply_tag(&css_tag, &start_iter, &end_iter);
                    }
                    // Build a:hover variant for link: tags
                    if ctx.link_hover_tag.is_none() && !ctx.hover_rules.is_empty() {
                        let hover_delta = apply_css_cascade(
                            "a",
                            class_attr.as_deref(),
                            id_attr.as_deref(),
                            None,
                            &ctx.hover_rules,
                        );
                        if has_meaningful_css(&hover_delta) {
                            // Merge base link CSS with hover CSS
                            let mut hover_props = css_props.clone();
                            hover_props.merge(&hover_delta);
                            let hover_tag = create_or_get_css_tag(buffer, &hover_props, false, &mut ctx.css_rules_store);
                            ctx.link_hover_tag = hover_tag.name().map(|n| n.to_string());
                        }
                    }
                }
                buffer.delete_mark(&start_mark);
                return;
            }

            // ── <body> — apply default styles ──
            if tag_name == "body" {
                handle_body(view, node, buffer, ctx, &css_props);
                return;
            }

            // ── Block elements — ensure newline before ──
            let is_block = is_block_element(&tag_name);
            if is_block {
                ensure_newline(buffer);
            }

            // ── Save context state for nesting ──
            let saved_in_pre = ctx.in_pre;
            let saved_text_transform = ctx.text_transform;

            // white-space handling
            if tag_name == "pre" || css_props.white_space == Some(WhiteSpaceMode::Pre) || css_props.white_space == Some(WhiteSpaceMode::PreWrap) {
                ctx.in_pre = true;
            }

            // text-transform
            if let Some(tt) = css_props.text_transform {
                ctx.text_transform = Some(tt);
            }

            // blockquote depth tracking
            if tag_name == "blockquote" {
                ctx.blockquote_depth += 1;
            }

            // ── Record start position ──
            let start_mark = buffer.create_mark(None, &buffer.end_iter(), true);

            // ── Recurse into children ──
            for child in node.children.borrow().iter() {
                walk_dom(view, child, buffer, ctx);
            }

            let start_iter = buffer.iter_at_mark(&start_mark);
            let end_iter = buffer.end_iter();

            if start_iter != end_iter {
                // ── Apply structural HTML tag ──
                let mapped_tag = match tag_name.as_str() {
                    "b" | "strong" => Some("b"),
                    "i" | "em" | "cite" | "var" | "address" => Some("i"),
                    "u" | "ins" => Some("u"),
                    "s" | "strike" | "del" => Some("s"),
                    "sub" => Some("sub"),
                    "sup" => Some("sup"),
                    "small" => Some("small"),
                    "big" => Some("big"),
                    "mark" => Some("mark"),
                    "pre" => Some("pre"),
                    "code" | "tt" | "kbd" | "samp" => Some("code"),
                    "dd" => Some("blockquote"),
                    "dt" => Some("b"),
                    "center" => Some("align_center"),
                    "a" => Some("a"),
                    "h1" => Some("h1"),
                    "h2" => Some("h2"),
                    "h3" => Some("h3"),
                    "h4" => Some("h4"),
                    "h5" => Some("h5"),
                    "h6" => Some("h6"),
                    _ => None,
                };

                if let Some(tag_name_str) = mapped_tag
                    && let Some(tag) = buffer.tag_table().lookup(tag_name_str) {
                        buffer.apply_tag(&tag, &start_iter, &end_iter);
                    }

                // Apply depth-aware blockquote tag
                if tag_name == "blockquote" {
                    let depth = std::cmp::min(ctx.blockquote_depth, 5);
                    let bq_tag_name = if depth > 0 {
                        format!("blockquote_{}", depth)
                    } else {
                        "blockquote".to_string()
                    };
                    if let Some(tag) = buffer.tag_table().lookup(&bq_tag_name) {
                        buffer.apply_tag(&tag, &start_iter, &end_iter);
                    }
                }

                // ── Apply CSS properties as a TextTag ──
                if has_meaningful_css(&css_props) {
                    let css_tag = create_or_get_css_tag(buffer, &css_props, is_block, &mut ctx.css_rules_store);
                    buffer.apply_tag(&css_tag, &start_iter, &end_iter);

                    // Build hover variant if :hover rules match this element
                    if !ctx.hover_rules.is_empty() {
                        let hover_delta = apply_css_cascade(
                            &tag_name,
                            class_attr.as_deref(),
                            id_attr.as_deref(),
                            None,
                            &ctx.hover_rules,
                        );
                        if has_meaningful_css(&hover_delta) {
                            let mut hover_props = css_props.clone();
                            hover_props.merge(&hover_delta);
                            let hover_tag = create_or_get_css_tag(buffer, &hover_props, is_block, &mut ctx.css_rules_store);
                            let normal_name = css_tag.name().unwrap().to_string();
                            let hover_name = hover_tag.name().unwrap().to_string();
                            if normal_name != hover_name {
                                ctx.hover_variants.insert(normal_name, hover_name);
                            }
                        }
                    }
                }
            }

            buffer.delete_mark(&start_mark);

            // ── Block elements — ensure newline after ──
            if is_block {
                ensure_newline(buffer);
            }

            // ── Restore context ──
            if tag_name == "blockquote" {
                ctx.blockquote_depth -= 1;
            }
            ctx.in_pre = saved_in_pre;
            ctx.text_transform = saved_text_transform;
        }

        NodeData::Document => {
            for child in node.children.borrow().iter() {
                walk_dom(view, child, buffer, ctx);
            }
        }

        _ => {} // Comments, processing instructions, etc.
    }
}

// ── Element Classification ─────────────────────────────────────────────────

fn is_block_element(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            | "pre" | "blockquote" | "dl" | "dt" | "dd"
            | "center" | "address" | "article" | "section" | "aside" | "nav"
            | "header" | "footer" | "main" | "figure" | "figcaption"
    )
}

fn ensure_newline(buffer: &gtk::TextBuffer) {
    let end = buffer.end_iter();
    if end.offset() > 0 && !end.starts_line() {
        let mut end = end;
        buffer.insert(&mut end, "\n");
    }
}

// ── HTML Attribute → CSS ───────────────────────────────────────────────────

fn build_extra_css_from_attrs(node: &Handle, tag_name: &str, extra_css: &mut String) {
    if let NodeData::Element { ref attrs, .. } = node.data {
        let attrs = attrs.borrow();

        if tag_name == "font" {
            for attr in attrs.iter() {
                let aname = attr.name.local.to_string();
                let aval = attr.value.to_string();
                match aname.as_str() {
                    "color" => { extra_css.push_str(&format!("color: {};", aval)); }
                    "face" => { extra_css.push_str(&format!("font-family: {};", aval)); }
                    "size" => {
                        if let Ok(sz) = aval.parse::<i32>() {
                            let pt = html_font_size_to_points(sz);
                            extra_css.push_str(&format!("font-size: {}pt;", pt));
                        }
                    }
                    _ => {}
                }
            }
        }

        // body tag attributes: text, bgcolor, link
        if tag_name == "body" {
            for attr in attrs.iter() {
                let aname = attr.name.local.to_string();
                match aname.as_str() {
                    "text" => { extra_css.push_str(&format!("color: {};", attr.value)); }
                    "bgcolor" => { extra_css.push_str(&format!("background-color: {};", attr.value)); }
                    "link" => {} // link color — handled separately if needed
                    _ => {}
                }
            }
            return;
        }

        // bgcolor attribute → background-color (for non-body elements)
        for attr in attrs.iter() {
            let aname = attr.name.local.to_string();
            if aname == "bgcolor" {
                extra_css.push_str(&format!("background-color: {};", attr.value));
            }
        }
    }
}

// ── CSS Tag Creation ───────────────────────────────────────────────────────

fn has_meaningful_css(props: &CssProperties) -> bool {
    props.color.is_some()
        || props.background_color.is_some()
        || props.font_family.is_some()
        || props.font_size.is_some()
        || props.font_weight.is_some()
        || props.font_style.is_some()
        || props.text_decoration_underline.is_some()
        || props.text_decoration_line_through.is_some()
        || props.text_align.is_some()
        || props.margin_left.is_some()
        || props.margin_right.is_some()
        || props.margin_top.is_some()
        || props.margin_bottom.is_some()
        || props.padding_left.is_some()
        || props.padding_right.is_some()
        || props.padding_top.is_some()
        || props.padding_bottom.is_some()
        || props.text_indent.is_some()
        || props.line_height.is_some()
        || props.paragraph_background.is_some()
        || props.direction.is_some()
        || props.letter_spacing.is_some()
        || props.vertical_align.is_some()
        || props.font_variant_small_caps.is_some()
}

/// Create or reuse a GTK TextTag for a given set of CSS properties.
/// Uses deterministic naming via hash for deduplication.
fn create_or_get_css_tag(
    buffer: &gtk::TextBuffer,
    props: &CssProperties,
    is_block: bool,
    css_rules_store: &mut HashMap<String, String>,
) -> gtk::TextTag {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let css_string = props.to_css_string();
    let mut hasher = DefaultHasher::new();
    css_string.hash(&mut hasher);
    let is_block_flag = if is_block { "b" } else { "i" }; // block vs inline
    let name = format!("css_{}_{:x}", is_block_flag, hasher.finish());

    if let Some(existing) = buffer.tag_table().lookup(&name) {
        return existing;
    }

    let tag = gtk::TextTag::new(Some(&name));
    apply_to_text_tag(props, &tag, is_block);
    buffer.tag_table().add(&tag);

    // Store original CSS for serialization round-trip
    css_rules_store.insert(name, css_string);

    tag
}

// ── <body> Handling ────────────────────────────────────────────────────────

fn handle_body(
    view: &gtk::TextView,
    node: &Handle,
    buffer: &gtk::TextBuffer,
    ctx: &mut ParseContext,
    css_props: &CssProperties,
) {
    // Apply body-level direction for BiDi support
    if let Some(TextDirection::Rtl) = css_props.direction {
        view.set_direction(gtk::TextDirection::Rtl);
    }

    // Apply body background and text color via CSS provider on the view
    let mut css_parts = Vec::new();
    if let Some(ref bg) = css_props.background_color {
        css_parts.push(format!("background-color: {};", bg));
    }
    if let Some(ref color) = css_props.color {
        css_parts.push(format!("color: {};", color));
    }
    if !css_parts.is_empty() {
        #[allow(deprecated)]
        {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(&format!("textview {{ {} }} textview text {{ {} }}",
                css_parts.join(" "), css_parts.join(" ")));
            view.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    }

    for child in node.children.borrow().iter() {
        walk_dom(view, child, buffer, ctx);
    }
}

// ── <hr> Handling ──────────────────────────────────────────────────────────

fn insert_hr_widget(view: &gtk::TextView, buffer: &gtk::TextBuffer) {
    ensure_newline(buffer);
    let mut end_iter = buffer.end_iter();
    let anchor = buffer.create_child_anchor(&mut end_iter);

    let hr_line = gtk::Separator::new(gtk::Orientation::Horizontal);
    hr_line.set_margin_top(8);
    hr_line.set_margin_bottom(8);
    hr_line.set_hexpand(true);
    hr_line.set_halign(gtk::Align::Fill);
    hr_line.set_size_request(400, -1);
    hr_line.set_widget_name("hr_rule");

    #[allow(deprecated)]
    {
        let provider = gtk::CssProvider::new();
        provider.load_from_data("separator#hr_rule { min-height: 2px; background-color: black; }");
        hr_line.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    view.add_child_at_anchor(&hr_line, &anchor);
    let mut end_iter = buffer.end_iter();
    buffer.insert(&mut end_iter, "\n");
}

// ── <img> Handling ─────────────────────────────────────────────────────

/// Decode a `data:[mime];base64,[payload]` URI into a GDK Texture.
fn decode_data_uri_to_texture(data_uri: &str) -> Option<gtk::gdk::Texture> {
    use gtk::glib;
    // Expected format: data:[<mime>][;base64],<data>
    let rest = data_uri.strip_prefix("data:")?;
    let comma_pos = rest.find(',')?;
    let payload = &rest[comma_pos + 1..];
    let meta = &rest[..comma_pos];
    if !meta.contains("base64") {
        return None; // Only base64 encoding supported
    }
    let decoded = glib::base64_decode(payload);
    if decoded.is_empty() {
        return None;
    }
    let bytes = glib::Bytes::from_owned(decoded);
    gtk::gdk::Texture::from_bytes(&bytes).ok()
}

fn insert_img_widget(view: &gtk::TextView, node: &Handle, buffer: &gtk::TextBuffer) {
    let mut src = String::new();
    let mut alt = String::new();
    let mut width: Option<i32> = None;
    let mut height: Option<i32> = None;

    if let NodeData::Element { ref attrs, .. } = node.data {
        for attr in attrs.borrow().iter() {
            let aname = attr.name.local.to_string();
            let aval = attr.value.to_string();
            match aname.as_str() {
                "src" => src = aval,
                "alt" => alt = aval,
                "width" => width = aval.replace("px", "").trim().parse().ok(),
                "height" => height = aval.replace("px", "").trim().parse().ok(),
                _ => {}
            }
        }
    }

    if src.is_empty() {
        // No source — just insert alt text or placeholder
        if !alt.is_empty() {
            let mut end_iter = buffer.end_iter();
            buffer.insert(&mut end_iter, &format!("[{}]", alt));
        }
        return;
    }

    let mut end_iter = buffer.end_iter();
    let anchor = buffer.create_child_anchor(&mut end_iter);

    let picture = if src.starts_with("data:") {
        // data: URI — decode base64 payload into a texture
        match decode_data_uri_to_texture(&src) {
            Some(texture) => {
                let pic = gtk::Picture::for_paintable(&texture);
                let natural_w = texture.width();
                let natural_h = texture.height();
                let (display_w, display_h) = if let Some(w) = width {
                    (w, height.unwrap_or((natural_h as f64 * w as f64 / natural_w as f64) as i32))
                } else {
                    let max_w = 600;
                    if natural_w > max_w {
                        let scale = max_w as f64 / natural_w as f64;
                        (max_w, (natural_h as f64 * scale) as i32)
                    } else {
                        (natural_w, natural_h)
                    }
                };
                pic.set_size_request(display_w, display_h);
                pic
            }
            None => {
                // Failed to decode — show placeholder
                let mut ei = buffer.end_iter();
                buffer.insert(&mut ei, &format!("[image: {}]", if !alt.is_empty() { &alt } else { "data URI" }));
                return;
            }
        }
    } else if src.starts_with("cid:") {
        // cid: reference — placeholder for MIME-embedded images
        let content_id = src.strip_prefix("cid:").unwrap_or("");
        let mut ei = buffer.end_iter();
        let label_text = if !alt.is_empty() {
            format!("[image: {}]", alt)
        } else {
            format!("[image: {}]", content_id)
        };
        buffer.insert(&mut ei, &label_text);
        return;
    } else {
        let file = gtk::gio::File::for_path(&src);
        match gtk::gdk::Texture::from_file(&file) {
            Ok(texture) => {
                let pic = gtk::Picture::for_paintable(&texture);
                let natural_w = texture.width();
                let natural_h = texture.height();
                // Apply explicit dimensions or constrain to max width
                let (display_w, display_h) = if let Some(w) = width {
                    (w, height.unwrap_or((natural_h as f64 * w as f64 / natural_w as f64) as i32))
                } else {
                    let max_w = 600;
                    if natural_w > max_w {
                        let scale = max_w as f64 / natural_w as f64;
                        (max_w, (natural_h as f64 * scale) as i32)
                    } else {
                        (natural_w, natural_h)
                    }
                };
                pic.set_size_request(display_w, display_h);
                pic
            }
            Err(_) => {
                // Fallback for unsupported formats or missing files
                let pic = gtk::Picture::for_file(&file);
                pic.set_size_request(width.unwrap_or(400), height.unwrap_or(-1));
                pic
            }
        }
    };
    picture.set_can_shrink(true);
    picture.set_halign(gtk::Align::Start);
    // Store src (and alt if present) in widget_name for serialization
    if alt.is_empty() {
        picture.set_widget_name(&format!("img:{}", src));
    } else {
        picture.set_widget_name(&format!("img:{}|alt:{}", src, alt));
    }

    view.add_child_at_anchor(&picture, &anchor);
}

// ── List Handling ──────────────────────────────────────────────────────────

fn handle_list(
    view: &gtk::TextView,
    node: &Handle,
    buffer: &gtk::TextBuffer,
    ctx: &mut ParseContext,
    tag_name: &str,
    css_props: &CssProperties,
) {
    let start_mark = buffer.create_mark(None, &buffer.end_iter(), true);
    let saved_depth = ctx.list_depth;
    let saved_ordered = ctx.in_ordered_list;
    let saved_list_style = ctx.list_style_type;

    ctx.list_depth += 1;
    ctx.in_ordered_list = tag_name == "ol";

    // Determine list style type
    if let Some(lst) = css_props.list_style_type {
        ctx.list_style_type = Some(lst);
    } else if tag_name == "ol" {
        ctx.list_style_type = Some(ListStyleType::Decimal);
    } else {
        ctx.list_style_type = Some(ListStyleType::Disc);
    }

    let mut ol_start_val = 1;
    if tag_name == "ol" {
        // Support start= attribute
        if let NodeData::Element { ref attrs, .. } = node.data {
            for attr in attrs.borrow().iter() {
                if attr.name.local.to_string() == "start"
                    && let Ok(v) = attr.value.to_string().parse::<i32>() {
                        ol_start_val = v;
                    }
            }
        }
        ctx.ol_counters.push(ol_start_val);
    }

    for child in node.children.borrow().iter() {
        walk_dom(view, child, buffer, ctx);
    }

    if tag_name == "ol" {
        ctx.ol_counters.pop();
    }

    // Apply list depth tag (encode start= in tag name for OL round-trip)
    let depth = std::cmp::min(ctx.list_depth, 5);
    let depth_tag_name = if tag_name == "ol" && ol_start_val != 1 {
        format!("ol_{}_s{}", depth, ol_start_val)
    } else {
        format!("{}_{}", if tag_name == "ol" { "ol" } else { "ul" }, depth)
    };
    // Create dynamic tag if it doesn't exist (for non-standard start values)
    if buffer.tag_table().lookup(&depth_tag_name).is_none() {
        let margin = depth * 20;
        let tag = gtk::TextTag::builder().name(&depth_tag_name).left_margin(margin).build();
        buffer.tag_table().add(&tag);
    }
    if let Some(tag) = buffer.tag_table().lookup(&depth_tag_name) {
        let start_iter = buffer.iter_at_mark(&start_mark);
        buffer.apply_tag(&tag, &start_iter, &buffer.end_iter());
    }
    buffer.delete_mark(&start_mark);

    ensure_newline(buffer);

    ctx.list_depth = saved_depth;
    ctx.in_ordered_list = saved_ordered;
    ctx.list_style_type = saved_list_style;
}

fn handle_list_item(
    view: &gtk::TextView,
    node: &Handle,
    buffer: &gtk::TextBuffer,
    ctx: &mut ParseContext,
    css_props: &CssProperties,
) {
    ensure_newline(buffer);

    let start_mark = buffer.create_mark(None, &buffer.end_iter(), true);
    let marker_start_mark = buffer.create_mark(None, &buffer.end_iter(), true);

    // Determine list style — check HTML type= attribute first, then CSS, then context default
    let style = resolve_li_style(node, css_props, ctx);

    // Get counter value for ordered lists
    let index = if ctx.in_ordered_list {
        if let Some(c) = ctx.ol_counters.last_mut() {
            let val = *c;
            *c += 1;
            val
        } else {
            1
        }
    } else {
        0
    };

    let marker = format_list_marker(style, index);
    if !marker.is_empty() {
        let mut iter = buffer.end_iter();
        buffer.insert(&mut iter, &marker);

        // Tag the marker text so serializer can skip it
        let marker_start = buffer.iter_at_mark(&marker_start_mark);
        let marker_end = buffer.end_iter();
        if let Some(tag) = buffer.tag_table().lookup("list_marker") {
            buffer.apply_tag(&tag, &marker_start, &marker_end);
        }
    }
    buffer.delete_mark(&marker_start_mark);

    for child in node.children.borrow().iter() {
        walk_dom(view, child, buffer, ctx);
    }

    let start_iter = buffer.iter_at_mark(&start_mark);
    if let Some(tag) = buffer.tag_table().lookup("li") {
        buffer.apply_tag(&tag, &start_iter, &buffer.end_iter());
    }

    // Apply CSS properties to the list item
    if has_meaningful_css(css_props) {
        let css_tag = create_or_get_css_tag(buffer, css_props, true, &mut ctx.css_rules_store);
        buffer.apply_tag(&css_tag, &start_iter, &buffer.end_iter());
    }

    buffer.delete_mark(&start_mark);
    ensure_newline(buffer);
}

fn resolve_li_style(node: &Handle, css_props: &CssProperties, ctx: &ParseContext) -> ListStyleType {
    // HTML type= attribute takes highest precedence
    if let NodeData::Element { ref attrs, .. } = node.data {
        for attr in attrs.borrow().iter() {
            if attr.name.local.to_string() == "type" {
                let val = attr.value.to_string();
                match val.as_str() {
                    "disc" => return ListStyleType::Disc,
                    "square" => return ListStyleType::Square,
                    "circle" => return ListStyleType::Circle,
                    "1" => return ListStyleType::Decimal,
                    "a" => return ListStyleType::LowerAlpha,
                    "A" => return ListStyleType::UpperAlpha,
                    "i" => return ListStyleType::LowerRoman,
                    "I" => return ListStyleType::UpperRoman,
                    _ => {}
                }
            }
        }
    }
    // CSS list-style-type
    if let Some(lst) = css_props.list_style_type {
        return lst;
    }
    // Inherit from list context
    ctx.list_style_type.unwrap_or(if ctx.in_ordered_list {
        ListStyleType::Decimal
    } else {
        ListStyleType::Disc
    })
}

// ── Table Handling ─────────────────────────────────────────────────────────

fn handle_table(
    view: &gtk::TextView,
    node: &Handle,
    buffer: &gtk::TextBuffer,
    ctx: &mut ParseContext,
    css_props: &CssProperties,
) {
    let mut table_bg = css_props.background_color.clone();
    let mut table_border_width = css_props.border_top_width;
    let mut cellpadding: Option<i32> = None;
    let mut cellspacing: Option<i32> = None;
    let mut border_collapse = false;
    let mut table_width: Option<String> = None;
    let mut table_align: Option<gtk::Align> = None;

    // Parse table-specific HTML attributes
    if let NodeData::Element { ref attrs, .. } = node.data {
        for attr in attrs.borrow().iter() {
            let aname = attr.name.local.to_string();
            let aval = attr.value.to_string();
            match aname.as_str() {
                "bgcolor" => table_bg = Some(aval),
                "border" => {
                    if let Ok(v) = aval.parse::<i32>() {
                        table_border_width = Some(v);
                    }
                }
                "cellpadding" => cellpadding = aval.parse().ok(),
                "cellspacing" => cellspacing = aval.parse().ok(),
                "width" => table_width = Some(aval),
                "align" => {
                    match aval.to_lowercase().as_str() {
                        "center" => table_align = Some(gtk::Align::Center),
                        "right" => table_align = Some(gtk::Align::End),
                        "left" => table_align = Some(gtk::Align::Start),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    // Check for border-collapse in CSS
    if let Some(ref style) = css_props.border_style
        && *style == BorderStyle::None {
            table_border_width = Some(0);
        }
    // Scan inline style for border-collapse
    // (already parsed in css_props, but also check for border-collapse specifically)
    if let NodeData::Element { ref attrs, .. } = node.data {
        for attr in attrs.borrow().iter() {
            if attr.name.local.to_string() == "style" {
                let style_str = attr.value.to_string().to_lowercase();
                if style_str.contains("border-collapse") && style_str.contains("collapse") {
                    border_collapse = true;
                    cellspacing = Some(0);
                }
            }
        }
    }

    // Handle <caption> — render as bold centered text above the table
    let mut caption_nodes = Vec::new();
    extract_elements_by_tag(node, &["caption"], &mut caption_nodes);
    if let Some(caption_node) = caption_nodes.first() {
        ensure_newline(buffer);
        let cap_start = buffer.create_mark(None, &buffer.end_iter(), true);
        for child in caption_node.children.borrow().iter() {
            walk_dom(view, child, buffer, ctx);
        }
        let cap_start_iter = buffer.iter_at_mark(&cap_start);
        let cap_end_iter = buffer.end_iter();
        if cap_start_iter != cap_end_iter {
            if let Some(b_tag) = buffer.tag_table().lookup("b") {
                buffer.apply_tag(&b_tag, &cap_start_iter, &cap_end_iter);
            }
            if let Some(center_tag) = buffer.tag_table().lookup("align_center") {
                buffer.apply_tag(&center_tag, &cap_start_iter, &cap_end_iter);
            }
        }
        buffer.delete_mark(&cap_start);
        ensure_newline(buffer);
    }

    let pad = cellpadding.unwrap_or(4);
    let spacing = if border_collapse { 0 } else { cellspacing.unwrap_or(1) };

    let grid = gtk::Grid::new();
    grid.set_column_spacing(spacing as u32);
    grid.set_row_spacing(spacing as u32);
    grid.set_focusable(false); // Let focus pass through to child TextViews
    grid.set_can_target(true);

    // Store ALL raw HTML attributes in widget_name for generic round-trip
    {
        let mut raw_attrs: Vec<(String, String)> = Vec::new();
        if let NodeData::Element { ref attrs, .. } = node.data {
            for attr in attrs.borrow().iter() {
                raw_attrs.push((attr.name.local.to_string(), attr.value.to_string()));
            }
        }
        // Merge cascade-resolved styles not already in raw attributes
        let has_bgcolor = raw_attrs.iter().any(|(k, _)| k == "bgcolor");
        let raw_style = raw_attrs.iter().find(|(k, _)| k == "style").map(|(_, v)| v.as_str()).unwrap_or("");
        let mut extra_css = Vec::new();
        if let Some(ref bg) = table_bg {
            if !has_bgcolor && !raw_style.contains("background") {
                extra_css.push(format!("background-color: {}", bg));
            }
        }
        if let Some(bw) = table_border_width {
            let has_border = raw_attrs.iter().any(|(k, _)| k == "border");
            if !has_border && !raw_style.contains("border") {
                extra_css.push(format!("border: {}px solid", bw));
            }
        }
        if border_collapse && !raw_style.contains("border-collapse") {
            extra_css.push("border-collapse: collapse".to_string());
        }
        if !extra_css.is_empty() {
            let extra = extra_css.join("; ");
            if let Some((_, v)) = raw_attrs.iter_mut().find(|(k, _)| k == "style") {
                let trimmed = v.trim_end().trim_end_matches(';');
                *v = format!("{}; {}", trimmed, extra);
            } else {
                raw_attrs.push(("style".to_string(), extra));
            }
        }
        let attrs_str = raw_attrs.iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, v.replace('"', "&quot;")))
            .collect::<Vec<_>>()
            .join(" ");
        grid.set_widget_name(&attrs_str);
    }

    // Apply table width
    if let Some(ref w) = table_width {
        if let Some(px) = w.strip_suffix('%') {
            // Percentage width — expand to fill
            if let Ok(_pct) = px.trim().parse::<i32>() {
                grid.set_hexpand(true);
                grid.set_halign(gtk::Align::Fill);
            }
        } else if let Ok(px_val) = w.replace("px", "").trim().parse::<i32>() {
            grid.set_size_request(px_val, -1);
            grid.set_hexpand(false);
        }
    } else {
        grid.set_hexpand(true);
    }

    // Apply table alignment
    if let Some(align) = table_align {
        grid.set_halign(align);
    } else {
        grid.set_halign(gtk::Align::Fill);
    }

    // Apply table-level CSS only when explicitly specified
    #[allow(deprecated)]
    {
        let mut css_parts = Vec::new();
        if let Some(ref bg) = table_bg {
            css_parts.push(format!("background-color: {};", bg));
        }
        if let Some(bw) = table_border_width {
            if bw == 0 {
                css_parts.push("border: none;".to_string());
            } else {
                css_parts.push(format!("border: {}px solid alpha(currentColor, 0.3);", bw));
            }
        }
        if !css_parts.is_empty() {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(&format!("grid {{ {} }}", css_parts.join(" ")));
            grid.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    }

    // Extract rows
    let mut tr_nodes = Vec::new();
    extract_elements_by_tag(node, &["tr", "thead", "tbody", "tfoot"], &mut tr_nodes);
    // Flatten: if we got thead/tbody/tfoot, extract tr from them
    let mut actual_tr_nodes = Vec::new();
    for n in &tr_nodes {
        if let NodeData::Element { ref name, .. } = n.data {
            let t = name.local.to_string().to_lowercase();
            if t == "tr" {
                actual_tr_nodes.push(n.clone());
            } else {
                // thead/tbody/tfoot — extract their tr children
                let mut inner_trs = Vec::new();
                extract_elements_by_tag(n, &["tr"], &mut inner_trs);
                actual_tr_nodes.extend(inner_trs);
            }
        }
    }
    let tr_nodes = actual_tr_nodes;

    // Pre-calculate column widths
    let mut col_max_chars: HashMap<i32, i32> = HashMap::new();
    let mut occupied: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();

    for (row_idx, tr_node) in tr_nodes.iter().enumerate() {
        let mut cell_nodes = Vec::new();
        extract_elements_by_tag(tr_node, &["td", "th"], &mut cell_nodes);

        let mut current_col = 0;
        for cell_node in cell_nodes.iter() {
            while occupied.contains(&(current_col, row_idx as i32)) {
                current_col += 1;
            }

            let (colspan, rowspan) = get_span_attrs(cell_node);

            let mut text_len = 0;
            for content in cell_node.children.borrow().iter() {
                count_text_length(content, &mut text_len);
            }

            let text_len_per_col = text_len / colspan;
            for c in 0..colspan {
                let col = current_col + c;
                let current_max = *col_max_chars.get(&col).unwrap_or(&0);
                col_max_chars.insert(col, std::cmp::max(current_max, text_len_per_col));
            }

            for r in 0..rowspan {
                for c in 0..colspan {
                    occupied.insert((current_col + c, row_idx as i32 + r));
                }
            }
            current_col += colspan;
        }
    }

    // Build grid cells
    let mut occupied_create: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();

    for (row_idx, tr_node) in tr_nodes.iter().enumerate() {
        let mut cell_nodes = Vec::new();
        extract_elements_by_tag(tr_node, &["td", "th"], &mut cell_nodes);

        // Get row-level attributes (bgcolor, valign)
        let mut row_bg: Option<String> = None;
        let mut row_valign: Option<gtk::Align> = None;
        if let NodeData::Element { ref attrs, .. } = tr_node.data {
            for attr in attrs.borrow().iter() {
                let aname = attr.name.local.to_string();
                match aname.as_str() {
                    "bgcolor" => row_bg = Some(attr.value.to_string()),
                    "valign" => {
                        match attr.value.to_string().to_lowercase().as_str() {
                            "top" => row_valign = Some(gtk::Align::Start),
                            "middle" | "center" => row_valign = Some(gtk::Align::Center),
                            "bottom" => row_valign = Some(gtk::Align::End),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut current_col = 0;
        for cell_node in cell_nodes.iter() {
            while occupied_create.contains(&(current_col, row_idx as i32)) {
                current_col += 1;
            }

            let (colspan, rowspan) = get_span_attrs(cell_node);
            let mut bg_color = row_bg.clone();
            let mut justif = gtk::Justification::Left;
            let mut is_header = false;
            let mut cell_border_width: Option<i32> = None;
            let mut cell_border_color: Option<String> = None;
            let mut cell_border_style_str: Option<String> = None;
            let mut cell_valign: Option<gtk::Align> = row_valign;
            let mut cell_nowrap = false;
            let mut cell_width: Option<i32> = None;
            let mut cell_color: Option<String> = None;
            let mut cell_font_family: Option<String> = None;
            let mut cell_font_size: Option<String> = None;
            let mut cell_padding: Option<i32> = None;
            let mut cell_hover_bg: Option<String> = None;

            if let NodeData::Element { ref name, ref attrs, .. } = cell_node.data {
                if name.local.to_string() == "th" {
                    is_header = true;
                    justif = gtk::Justification::Center;
                }

                // Resolve CSS cascade for the cell
                let cell_attrs = attrs.borrow();
                let mut cell_class = None;
                let mut cell_id = None;
                let mut cell_style = None;
                for attr in cell_attrs.iter() {
                    let aname = attr.name.local.to_string();
                    match aname.as_str() {
                        "class" => cell_class = Some(attr.value.to_string()),
                        "id" => cell_id = Some(attr.value.to_string()),
                        "style" => cell_style = Some(attr.value.to_string()),
                        "bgcolor" => bg_color = Some(attr.value.to_string()),
                        "align" => {
                            match attr.value.to_string().to_lowercase().as_str() {
                                "center" => justif = gtk::Justification::Center,
                                "right" => justif = gtk::Justification::Right,
                                "justify" | "fill" => justif = gtk::Justification::Fill,
                                _ => {}
                            }
                        }
                        "valign" => {
                            match attr.value.to_string().to_lowercase().as_str() {
                                "top" => cell_valign = Some(gtk::Align::Start),
                                "middle" | "center" => cell_valign = Some(gtk::Align::Center),
                                "bottom" => cell_valign = Some(gtk::Align::End),
                                _ => {}
                            }
                        }
                        "nowrap" => cell_nowrap = true,
                        "width" => {
                            let wval = attr.value.to_string();
                            if !wval.contains('%') {
                                cell_width = wval.replace("px", "").trim().parse().ok();
                            }
                        }
                        _ => {}
                    }
                }
                drop(cell_attrs);

                let cell_tag = if is_header { "th" } else { "td" };
                let cell_css = apply_css_cascade(
                    cell_tag,
                    cell_class.as_deref(),
                    cell_id.as_deref(),
                    cell_style.as_deref(),
                    &ctx.css_rules,
                );

                if cell_css.text_align.is_some() {
                    justif = cell_css.text_align.unwrap();
                }
                if cell_css.background_color.is_some() {
                    bg_color = cell_css.background_color.clone();
                }
                if cell_css.has_border() {
                    cell_border_width = cell_css.border_top_width;
                    cell_border_color = cell_css.border_color.clone();
                    cell_border_style_str = Some(match cell_css.border_style {
                        Some(BorderStyle::Dashed) => "dashed".to_string(),
                        Some(BorderStyle::Dotted) => "dotted".to_string(),
                        Some(BorderStyle::Double) => "double".to_string(),
                        _ => "solid".to_string(),
                    });
                }
                if cell_css.color.is_some() {
                    cell_color = cell_css.color.clone();
                }
                if cell_css.font_family.is_some() {
                    cell_font_family = cell_css.font_family.clone();
                }
                if let Some(fs) = cell_css.font_size {
                    cell_font_size = Some(format!("{}pt", fs));
                }
                if cell_css.padding_top.is_some() {
                    cell_padding = cell_css.padding_top;
                }
                // Resolve :hover rules for this cell
                if !ctx.hover_rules.is_empty() {
                    let hover_delta = apply_css_cascade(
                        cell_tag,
                        cell_class.as_deref(),
                        cell_id.as_deref(),
                        None,
                        &ctx.hover_rules,
                    );
                    if hover_delta.background_color.is_some() {
                        cell_hover_bg = hover_delta.background_color;
                    }
                }
            }

            for r in 0..rowspan {
                for c in 0..colspan {
                    occupied_create.insert((current_col + c, row_idx as i32 + r));
                }
            }

            let cell_view = gtk::TextView::new();
            cell_view.set_wrap_mode(if cell_nowrap { gtk::WrapMode::None } else { gtk::WrapMode::WordChar });
            cell_view.set_hexpand(true);
            cell_view.set_vexpand(false);
            cell_view.set_halign(gtk::Align::Fill);
            cell_view.set_valign(cell_valign.unwrap_or(gtk::Align::Fill));
            cell_view.set_justification(justif);
            cell_view.set_left_margin(pad);
            cell_view.set_right_margin(pad);
            cell_view.set_top_margin(pad);
            cell_view.set_bottom_margin(pad);
            cell_view.set_pixels_above_lines(2);
            cell_view.set_pixels_below_lines(2);
            cell_view.set_focusable(true);
            cell_view.set_can_focus(true);
            cell_view.set_editable(true);

            // Grab focus on click so the cell becomes editable
            let cv = cell_view.clone();
            let click = gtk::GestureClick::new();
            click.connect_pressed(move |gesture, _n, _x, _y| {
                cv.grab_focus();
                gesture.set_state(gtk::EventSequenceState::Claimed);
            });
            cell_view.add_controller(click);
            // Store tag name + ALL raw HTML attributes for generic round-trip
            let cell_type = if is_header { "th" } else { "td" };
            {
                let mut raw_attrs: Vec<(String, String)> = Vec::new();
                if let NodeData::Element { ref attrs, .. } = cell_node.data {
                    for attr in attrs.borrow().iter() {
                        raw_attrs.push((attr.name.local.to_string(), attr.value.to_string()));
                    }
                }
                // Merge cascade/inherited styles not in raw attributes
                let has_bgcolor = raw_attrs.iter().any(|(k, _)| k == "bgcolor");
                let has_align = raw_attrs.iter().any(|(k, _)| k == "align");
                let has_valign = raw_attrs.iter().any(|(k, _)| k == "valign");
                let raw_style = raw_attrs.iter().find(|(k, _)| k == "style").map(|(_, v)| v.clone()).unwrap_or_default();
                let mut extra_css = Vec::new();
                // Background from row inheritance or CSS cascade
                if let Some(ref bg) = bg_color {
                    if !has_bgcolor && !raw_style.contains("background") {
                        extra_css.push(format!("background-color: {}", bg));
                    }
                }
                // Text alignment from row inheritance or CSS cascade
                let default_justif = if is_header { gtk::Justification::Center } else { gtk::Justification::Left };
                if justif != default_justif && !has_align && !raw_style.contains("text-align") {
                    let a = match justif {
                        gtk::Justification::Center => "center",
                        gtk::Justification::Right => "right",
                        gtk::Justification::Fill => "justify",
                        _ => "left",
                    };
                    extra_css.push(format!("text-align: {}", a));
                }
                // Border from CSS cascade (not inherited from table)
                if let Some(bw) = cell_border_width {
                    if !raw_style.contains("border") {
                        let bs = cell_border_style_str.as_deref().unwrap_or("solid");
                        let bc = cell_border_color.as_deref().unwrap_or("currentColor");
                        extra_css.push(format!("border: {}px {} {}", bw, bs, bc));
                    }
                }
                // Valign from row inheritance
                if let Some(va) = cell_valign {
                    if !has_valign {
                        let v = match va {
                            gtk::Align::Start => "top",
                            gtk::Align::Center => "middle",
                            gtk::Align::End => "bottom",
                            _ => "",
                        };
                        if !v.is_empty() {
                            raw_attrs.push(("valign".to_string(), v.to_string()));
                        }
                    }
                }
                if !extra_css.is_empty() {
                    let extra = extra_css.join("; ");
                    if let Some((_, v)) = raw_attrs.iter_mut().find(|(k, _)| k == "style") {
                        let trimmed = v.trim_end().trim_end_matches(';');
                        *v = format!("{}; {}", trimmed, extra);
                    } else {
                        raw_attrs.push(("style".to_string(), extra));
                    }
                }
                let attrs_str = raw_attrs.iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v.replace('"', "&quot;")))
                    .collect::<Vec<_>>()
                    .join(" ");
                if attrs_str.is_empty() {
                    cell_view.set_widget_name(cell_type);
                } else {
                    cell_view.set_widget_name(&format!("{}|{}", cell_type, attrs_str));
                }
            }

            // Apply cell background, borders, color, and font via CSS
            #[allow(deprecated)]
            {
                let mut css_parts = Vec::new();
                if let Some(ref bg) = bg_color {
                    css_parts.push(format!("background-color: {};", bg));
                }
                if let Some(ref c) = cell_color {
                    css_parts.push(format!("color: {};", c));
                }
                if let Some(ref ff) = cell_font_family {
                    css_parts.push(format!("font-family: {};", ff));
                }
                if let Some(ref fs) = cell_font_size {
                    css_parts.push(format!("font-size: {};", fs));
                }
                if let Some(bw) = cell_border_width {
                    let bs = cell_border_style_str.as_deref().unwrap_or("solid");
                    let bc = cell_border_color.as_deref().unwrap_or("alpha(currentColor, 0.3)");
                    css_parts.push(format!("border: {}px {} {};", bw, bs, bc));
                } else if let Some(tbw) = table_border_width
                    && tbw > 0 {
                        // Only inherit border when table explicitly has border= attribute
                        css_parts.push("border: 1px solid alpha(currentColor, 0.3);".to_string());
                    }
                if let Some(p) = cell_padding {
                    // Override the default cellpadding with cell-specific padding
                    cell_view.set_left_margin(p);
                    cell_view.set_right_margin(p);
                    cell_view.set_top_margin(p);
                    cell_view.set_bottom_margin(p);
                }
                let mut hover_parts = Vec::new();
                if let Some(ref hbg) = cell_hover_bg {
                    hover_parts.push(format!("background-color: {};", hbg));
                }
                if !css_parts.is_empty() || !hover_parts.is_empty() {
                    let provider = gtk::CssProvider::new();
                    let mut css_str = String::new();
                    if !css_parts.is_empty() {
                        css_str.push_str(&format!("textview {{ {} }}", css_parts.join(" ")));
                    }
                    if !hover_parts.is_empty() {
                        css_str.push_str(&format!(" textview:hover {{ {} }}", hover_parts.join(" ")));
                    }
                    provider.load_from_data(&css_str);
                    cell_view.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
                }
            }

            let cell_buffer = cell_view.buffer();
            crate::setup_tags(&cell_buffer);

            for content in cell_node.children.borrow().iter() {
                walk_dom(&cell_view, content, &cell_buffer, ctx);
            }

            // For th cells, apply bold visually via CSS on the widget rather than
            // a TextTag, so the serializer doesn't emit <b> inside <th>.
            if is_header {
                #[allow(deprecated)]
                {
                    let provider = gtk::CssProvider::new();
                    provider.load_from_data("textview { font-weight: bold; }");
                    cell_view.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
                }
            }

            // Apply explicit cell width or estimate from content
            if let Some(w) = cell_width {
                cell_view.set_size_request(w, -1);
            } else {
                let mut max_chars_in_cols = 0;
                for c in 0..colspan {
                    max_chars_in_cols += *col_max_chars.get(&(current_col + c)).unwrap_or(&0);
                }
                let min_width = std::cmp::max(20, max_chars_in_cols * 8).min(600);
                cell_view.set_size_request(min_width, -1);
            }

            grid.attach(&cell_view, current_col, row_idx as i32, colspan, rowspan);
            current_col += colspan;
        }
    }

    ensure_newline(buffer);
    let mut end_iter = buffer.end_iter();
    let anchor = buffer.create_child_anchor(&mut end_iter);
    view.add_child_at_anchor(&grid, &anchor);
    buffer.insert(&mut end_iter, "\n");
}

fn get_span_attrs(node: &Handle) -> (i32, i32) {
    let mut colspan = 1;
    let mut rowspan = 1;
    if let NodeData::Element { ref attrs, .. } = node.data {
        for attr in attrs.borrow().iter() {
            let aname = attr.name.local.to_string();
            let aval = attr.value.to_string();
            match aname.as_str() {
                "colspan" => { if let Ok(v) = aval.parse::<i32>() { colspan = v.max(1); } }
                "rowspan" => { if let Ok(v) = aval.parse::<i32>() { rowspan = v.max(1); } }
                _ => {}
            }
        }
    }
    (colspan, rowspan)
}

/// Recursively count text length in a subtree (for column width estimation).
/// Stops at nested <table> boundaries — those have their own sizing.
fn count_text_length(node: &Handle, len: &mut i32) {
    match node.data {
        NodeData::Text { ref contents } => {
            *len += contents.borrow().chars().count() as i32;
        }
        NodeData::Element { ref name, .. } => {
            if name.local.eq_str_ignore_ascii_case("table") {
                return; // Nested table — skip, it sizes itself
            }
            for child in node.children.borrow().iter() {
                count_text_length(child, len);
            }
        }
        _ => {
            for child in node.children.borrow().iter() {
                count_text_length(child, len);
            }
        }
    }
}

// ── Helper: Extract Elements by Tag ────────────────────────────────────────

pub fn extract_elements_by_tag(node: &Handle, target_tags: &[&str], results: &mut Vec<Handle>) {
    if let NodeData::Element { ref name, .. } = node.data {
        let tag = name.local.to_string().to_lowercase();
        if target_tags.contains(&tag.as_str()) {
            results.push(node.clone());
            return;
        }
    }
    for child in node.children.borrow().iter() {
        // Don't recurse into nested <table> elements — they have their own scope
        // and will be processed by their own handle_table() call via walk_dom.
        if let NodeData::Element { ref name, .. } = child.data {
            if name.local.eq_str_ignore_ascii_case("table") {
                continue;
            }
        }
        extract_elements_by_tag(child, target_tags, results);
    }
}
