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

/// Metadata for an HTML element with an `id`, used for class manipulation.
pub struct ElementMeta {
    pub tag_name: String,
    pub classes: Vec<String>,
    pub inline_style: Option<String>,
}

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
    pub element_meta: HashMap<String, ElementMeta>,
    pub element_providers: HashMap<String, gtk::CssProvider>,
    pub cid_resolver: Option<std::rc::Rc<dyn Fn(&str) -> Option<Vec<u8>>>>,
}

impl ParseContext {
    pub fn new(
        css_rules: HashMap<String, String>,
        hover_rules: HashMap<String, String>,
        cid_resolver: Option<std::rc::Rc<dyn Fn(&str) -> Option<Vec<u8>>>>,
    ) -> Self {
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
            element_meta: HashMap::new(),
            element_providers: HashMap::new(),
            cid_resolver,
        }
    }
}

/// Result of parsing HTML into a buffer.
pub struct ParseResult {
    pub css_rules_store: HashMap<String, String>,
    pub hover_variants: HashMap<String, String>,
    pub link_hover_tag: Option<String>,
    pub style_rules: HashMap<String, String>,
    pub hover_rules: HashMap<String, String>,
    pub element_meta: HashMap<String, ElementMeta>,
    pub element_providers: HashMap<String, gtk::CssProvider>,
}

// ── Public Entry Point ─────────────────────────────────────────────────────

pub fn parse_html_to_buffer(
    view: &gtk::TextView,
    dom: &markup5ever_rcdom::RcDom,
    buffer: &gtk::TextBuffer,
    cid_resolver: Option<std::rc::Rc<dyn Fn(&str) -> Option<Vec<u8>>>>,
) -> ParseResult {
    let mut css_rules = HashMap::new();
    let mut hover_rules = HashMap::new();
    collect_style_rules(&dom.document, &mut css_rules, &mut hover_rules);

    let mut ctx = ParseContext::new(css_rules.clone(), hover_rules.clone(), cid_resolver);
    walk_dom(view, &dom.document, buffer, &mut ctx);
    ParseResult {
        css_rules_store: ctx.css_rules_store,
        hover_variants: ctx.hover_variants,
        link_hover_tag: ctx.link_hover_tag,
        style_rules: css_rules,
        hover_rules,
        element_meta: ctx.element_meta,
        element_providers: ctx.element_providers,
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

            // ── Flex / Grid layouts ──
            if let Some(ref d) = css_props.display {
                match d.as_str() {
                    "flex" | "inline-flex" => {
                        handle_flex(view, node, buffer, ctx, &css_props);
                        return;
                    }
                    "grid" | "inline-grid" => {
                        handle_css_grid(view, node, buffer, ctx, &css_props);
                        return;
                    }
                    _ => {}
                }
            }

            // ── Self-closing / special elements ──
            match tag_name.as_str() {
                "img" => {
                    insert_img_widget(view, node, buffer, ctx);
                    return;
                }
                "svg" => {
                    insert_svg_widget(view, node, buffer, ctx);
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

            // ── Apply editable_id: marker for readonly-except support ──
            if let Some(ref id) = id_attr {
                if !id.is_empty() && start_iter != end_iter {
                    let id_tag_name = format!("editable_id:{}", id);
                    let id_tag = if let Some(existing) = buffer.tag_table().lookup(&id_tag_name) {
                        existing
                    } else {
                        let new_tag = gtk::TextTag::new(Some(&id_tag_name));
                        buffer.tag_table().add(&new_tag);
                        new_tag
                    };
                    buffer.apply_tag(&id_tag, &start_iter, &end_iter);

                    // Record element metadata for class manipulation
                    let classes = class_attr.as_deref().unwrap_or("")
                        .split_whitespace().map(|s| s.to_string()).collect();
                    ctx.element_meta.insert(id.clone(), ElementMeta {
                        tag_name: tag_name.clone(),
                        classes,
                        inline_style: style_attr.clone(),
                    });
                    // Store class attr in css_rules_store for serialization
                    if let Some(ref cls) = class_attr {
                        if !cls.is_empty() {
                            ctx.css_rules_store.insert(
                                format!("classattr:{}", id), cls.clone(),
                            );
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

        NodeData::Comment { ref contents } => {
            // Preserve HTML comments as zero-width tagged markers for round-trip
            let comment_text = contents.to_string();
            let tag_name = format!("comment:{}", comment_text);
            let tag = if let Some(existing) = buffer.tag_table().lookup(&tag_name) {
                existing
            } else {
                let new_tag = gtk::TextTag::new(Some(&tag_name));
                new_tag.set_invisible(true);
                buffer.tag_table().add(&new_tag);
                new_tag
            };
            let mut end_iter = buffer.end_iter();
            let offset = end_iter.offset();
            buffer.insert(&mut end_iter, "\u{200B}");
            let start = buffer.iter_at_offset(offset);
            let end = buffer.end_iter();
            buffer.apply_tag(&tag, &start, &end);
        }

        _ => {} // Processing instructions, etc.
    }
}

// ── Element Classification ─────────────────────────────────────────────────

pub fn is_block_element(tag: &str) -> bool {
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

pub fn has_meaningful_css(props: &CssProperties) -> bool {
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
pub fn create_or_get_css_tag(
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

// ── SVG Handling ──────────────────────────────────────────────────────

/// Walk a markup5ever DOM subtree and reconstruct XML text.
fn serialize_dom_to_xml(node: &Handle) -> String {
    let mut xml = String::new();
    match &node.data {
        NodeData::Element { name, attrs, .. } => {
            let tag = &name.local;
            xml.push_str(&format!("<{}", tag));
            for attr in attrs.borrow().iter() {
                xml.push_str(&format!(" {}=\"{}\"", attr.name.local, attr.value));
            }
            let children = node.children.borrow();
            if children.is_empty() {
                xml.push_str("/>");
            } else {
                xml.push('>');
                for child in children.iter() {
                    xml.push_str(&serialize_dom_to_xml(child));
                }
                xml.push_str(&format!("</{}>", tag));
            }
        }
        NodeData::Text { contents } => {
            xml.push_str(&contents.borrow().to_string());
        }
        _ => {
            for child in node.children.borrow().iter() {
                xml.push_str(&serialize_dom_to_xml(child));
            }
        }
    }
    xml
}

/// Decode percent-encoded strings (e.g. URL-encoded SVG in data URIs).
fn urlish_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(b'0');
            let lo = chars.next().unwrap_or(b'0');
            let val = u8::from_str_radix(&format!("{}{}", hi as char, lo as char), 16).unwrap_or(b'?');
            result.push(val as char);
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

/// FNV-1a hash for content-addressable SVG storage.
fn simple_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Render SVG bytes to a GDK texture via resvg.
fn render_svg_to_texture(svg_data: &[u8]) -> Option<(gtk::gdk::Texture, i32, i32)> {
    let tree = resvg::usvg::Tree::from_data(svg_data, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let w = size.width().ceil() as u32;
    let h = size.height().ceil() as u32;
    if w == 0 || h == 0 {
        return None;
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());
    let bytes = gtk::glib::Bytes::from(pixmap.data());
    let texture = gtk::gdk::MemoryTexture::new(
        w as i32,
        h as i32,
        gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied,
        &bytes,
        (w * 4) as usize,
    );
    Some((texture.upcast(), w as i32, h as i32))
}

/// Handle an inline `<svg>` element: render to texture and embed as Picture.
fn insert_svg_widget(
    view: &gtk::TextView,
    node: &Handle,
    buffer: &gtk::TextBuffer,
    ctx: &mut ParseContext,
) {
    let svg_source = serialize_dom_to_xml(node);
    // html5ever stores SVG namespace internally but doesn't emit xmlns as an attribute.
    // resvg needs xmlns="http://www.w3.org/2000/svg" to parse correctly.
    let svg_source = if !svg_source.contains("xmlns") {
        svg_source.replacen("<svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"", 1)
    } else {
        svg_source
    };
    let svg_bytes = svg_source.as_bytes();

    // Check for explicit width/height attributes
    let mut attr_width: Option<i32> = None;
    let mut attr_height: Option<i32> = None;
    if let NodeData::Element { ref attrs, .. } = node.data {
        for attr in attrs.borrow().iter() {
            match attr.name.local.to_string().as_str() {
                "width" => attr_width = attr.value.to_string().replace("px", "").trim().parse().ok(),
                "height" => attr_height = attr.value.to_string().replace("px", "").trim().parse().ok(),
                _ => {}
            }
        }
    }

    match render_svg_to_texture(svg_bytes) {
        Some((texture, natural_w, natural_h)) => {
            let picture = gtk::Picture::for_paintable(&texture);
            let display_w = attr_width.unwrap_or(natural_w);
            let display_h = attr_height.unwrap_or(natural_h);
            picture.set_size_request(display_w, display_h);
            // Store SVG source by hash for round-trip serialization
            let hash = simple_hash(svg_bytes);
            let key = format!("svg:{:x}", hash);
            ctx.css_rules_store.insert(key.clone(), svg_source);
            picture.set_widget_name(&key);
            setup_image_click_resize(&picture);
            let mut end_iter = buffer.end_iter();
            let anchor = buffer.create_child_anchor(&mut end_iter);
            view.add_child_at_anchor(&picture, &anchor);
        }
        None => {
            let mut end_iter = buffer.end_iter();
            buffer.insert(&mut end_iter, "[SVG]");
        }
    }
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

fn insert_img_widget(view: &gtk::TextView, node: &Handle, buffer: &gtk::TextBuffer, ctx: &mut ParseContext) {
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

    // Helper to size a picture from a texture
    fn size_picture(pic: &gtk::Picture, natural_w: i32, natural_h: i32, width: Option<i32>, height: Option<i32>) {
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
    }

    let mut end_iter = buffer.end_iter();
    let anchor = buffer.create_child_anchor(&mut end_iter);

    let picture = if src.starts_with("data:image/svg+xml") {
        // SVG data URI — decode and render via resvg
        let rest = match src.strip_prefix("data:") {
            Some(r) => r,
            None => {
                let mut ei = buffer.end_iter();
                buffer.insert(&mut ei, "[image: invalid SVG data URI]");
                return;
            }
        };
        let svg_bytes = if let Some(comma_pos) = rest.find(',') {
            let meta = &rest[..comma_pos];
            let payload = &rest[comma_pos + 1..];
            if meta.contains("base64") {
                gtk::glib::base64_decode(payload)
            } else {
                // URL-encoded SVG
                urlish_decode(payload).into_bytes()
            }
        } else {
            Vec::new()
        };
        if svg_bytes.is_empty() {
            let mut ei = buffer.end_iter();
            buffer.insert(&mut ei, &format!("[image: {}]", if !alt.is_empty() { &alt } else { "SVG data URI" }));
            return;
        }
        match render_svg_to_texture(&svg_bytes) {
            Some((texture, natural_w, natural_h)) => {
                let pic = gtk::Picture::for_paintable(&texture);
                size_picture(&pic, natural_w, natural_h, width, height);
                pic
            }
            None => {
                let mut ei = buffer.end_iter();
                buffer.insert(&mut ei, &format!("[image: {}]", if !alt.is_empty() { &alt } else { "SVG" }));
                return;
            }
        }
    } else if src.starts_with("data:") {
        // data: URI — decode base64 payload into a texture
        match decode_data_uri_to_texture(&src) {
            Some(texture) => {
                let pic = gtk::Picture::for_paintable(&texture);
                size_picture(&pic, texture.width(), texture.height(), width, height);
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
        let content_id = src.strip_prefix("cid:").unwrap_or("");

        // Try resolving via callback
        let resolved = ctx.cid_resolver.as_ref().and_then(|r| r(content_id));
        if let Some(bytes) = resolved {
            let glib_bytes = gtk::glib::Bytes::from_owned(bytes);
            match gtk::gdk::Texture::from_bytes(&glib_bytes) {
                Ok(texture) => {
                    let pic = gtk::Picture::for_paintable(&texture);
                    size_picture(&pic, texture.width(), texture.height(), width, height);
                    pic.set_widget_name(&format!("img:cid:{}", content_id));
                    if !alt.is_empty() {
                        pic.set_widget_name(&format!("img:cid:{}|alt:{}", content_id, alt));
                    }
                    setup_image_click_resize(&pic);
                    let mut end_iter = buffer.end_iter();
                    let anchor = buffer.create_child_anchor(&mut end_iter);
                    view.add_child_at_anchor(&pic, &anchor);
                    return;
                }
                Err(_) => {} // Fall through to placeholder
            }
        }

        // Fallback: placeholder text
        let mut ei = buffer.end_iter();
        let label_text = if !alt.is_empty() {
            format!("[image: {}]", alt)
        } else {
            format!("[image: {}]", content_id)
        };
        buffer.insert(&mut ei, &label_text);
        return;
    } else if src.to_lowercase().ends_with(".svg") {
        // .svg file — render via resvg
        match std::fs::read(&src) {
            Ok(svg_bytes) => {
                match render_svg_to_texture(&svg_bytes) {
                    Some((texture, natural_w, natural_h)) => {
                        let pic = gtk::Picture::for_paintable(&texture);
                        size_picture(&pic, natural_w, natural_h, width, height);
                        pic
                    }
                    None => {
                        let pic = gtk::Picture::for_file(&gtk::gio::File::for_path(&src));
                        pic.set_size_request(width.unwrap_or(400), height.unwrap_or(-1));
                        pic
                    }
                }
            }
            Err(_) => {
                let pic = gtk::Picture::for_file(&gtk::gio::File::for_path(&src));
                pic.set_size_request(width.unwrap_or(400), height.unwrap_or(-1));
                pic
            }
        }
    } else {
        let file = gtk::gio::File::for_path(&src);
        match gtk::gdk::Texture::from_file(&file) {
            Ok(texture) => {
                let pic = gtk::Picture::for_paintable(&texture);
                size_picture(&pic, texture.width(), texture.height(), width, height);
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
    if alt.is_empty() {
        picture.set_widget_name(&format!("img:{}", src));
    } else {
        picture.set_widget_name(&format!("img:{}|alt:{}", src, alt));
    }
    setup_image_click_resize(&picture);
    view.add_child_at_anchor(&picture, &anchor);
}

// Shared state: the currently selected image (if any).
thread_local! {
    static SELECTED_IMAGE: std::cell::RefCell<Option<gtk::Picture>> = const { std::cell::RefCell::new(None) };
}

/// Get the currently selected image, if any.
pub(crate) fn selected_image() -> Option<gtk::Picture> {
    SELECTED_IMAGE.with(|sel| sel.borrow().clone())
}

/// Clear the current image selection (remove highlight and forget).
pub(crate) fn clear_image_selection() {
    SELECTED_IMAGE.with(|sel| {
        if let Some(prev) = sel.borrow_mut().take() {
            prev.remove_css_class("image-selected");
        }
    });
}

/// Attach click handlers to a Picture: single click = select, double click = resize dialog.
pub(crate) fn setup_image_click_resize(picture: &gtk::Picture) {
    picture.set_can_target(true);

    // Register CSS once
    static CSS_REGISTERED: std::sync::Once = std::sync::Once::new();
    CSS_REGISTERED.call_once(|| {
        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_data("picture.image-selected { border: 2px solid @accent_color; }");
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().unwrap(),
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });

    let click = gtk::GestureClick::new();
    click.set_button(1);
    let pic = picture.clone();
    click.connect_pressed(move |gesture, n_press, _x, _y| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        // Don't interact in readonly mode; ensure focus stays on the TextView for key events
        let mut editable = true;
        if let Some(view_widget) = pic.parent() {
            if let Some(tv) = view_widget.downcast_ref::<gtk::TextView>() {
                if !tv.is_editable() { editable = false; }
                tv.grab_focus();
            }
        }
        if !editable { return; }
        if n_press == 1 {
            let already_selected = pic.has_css_class("image-selected");
            // Deselect previous
            SELECTED_IMAGE.with(|sel| {
                if let Some(prev) = sel.borrow_mut().take() {
                    prev.remove_css_class("image-selected");
                }
            });
            if !already_selected {
                pic.add_css_class("image-selected");
                SELECTED_IMAGE.with(|sel| {
                    *sel.borrow_mut() = Some(pic.clone());
                });
            }
        } else if n_press == 2 {
            show_resize_dialog(&pic);
        }
    });
    picture.add_controller(click);
}

/// Find the current paragraph justification for the line containing a Picture's child anchor.
fn get_image_justification(pic: &gtk::Picture) -> gtk::Justification {
    let Some(tv) = pic.parent().and_then(|p| p.downcast::<gtk::TextView>().ok()) else {
        return gtk::Justification::Left;
    };
    let buffer = tv.buffer();
    // Walk through the buffer to find the child anchor that holds this picture
    let mut iter = buffer.start_iter();
    loop {
        if let Some(anchor) = iter.child_anchor() {
            for w in anchor.widgets() {
                if w.eq(pic.upcast_ref::<gtk::Widget>()) {
                    // Found it — check justification tags on this line
                    let line_start = {
                        let mut ls = iter;
                        ls.set_line_offset(0);
                        ls
                    };
                    for tag in line_start.tags() {
                        if let Some(name) = tag.name() {
                            if name.contains("center") || name.contains("Center") {
                                return gtk::Justification::Center;
                            }
                            if name.contains("right") || name.contains("Right") {
                                return gtk::Justification::Right;
                            }
                        }
                    }
                    return gtk::Justification::Left;
                }
            }
        }
        if !iter.forward_char() { break; }
    }
    gtk::Justification::Left
}

/// Apply paragraph justification to the line containing a Picture's child anchor.
fn apply_image_justification(pic: &gtk::Picture, justification: gtk::Justification) {
    let Some(tv) = pic.parent().and_then(|p| p.downcast::<gtk::TextView>().ok()) else { return };
    let buffer = tv.buffer();
    // Find the iter at the child anchor
    let mut iter = buffer.start_iter();
    loop {
        if let Some(anchor) = iter.child_anchor() {
            for w in anchor.widgets() {
                if w.eq(pic.upcast_ref::<gtk::Widget>()) {
                    // Found it — apply justification to this line
                    let mut line_start = iter;
                    line_start.set_line_offset(0);
                    let mut line_end = iter;
                    if !line_end.ends_line() { line_end.forward_to_line_end(); }

                    // Remove existing alignment tags
                    for tag_name in &["text-align: left", "text-align: center", "text-align: right"] {
                        if let Some(tag) = buffer.tag_table().lookup(tag_name) {
                            buffer.remove_tag(&tag, &line_start, &line_end);
                        }
                    }

                    let tag_name = match justification {
                        gtk::Justification::Center => "text-align: center",
                        gtk::Justification::Right => "text-align: right",
                        _ => "text-align: left",
                    };
                    let tag = if let Some(t) = buffer.tag_table().lookup(tag_name) {
                        t
                    } else {
                        let t = gtk::TextTag::builder()
                            .name(tag_name)
                            .justification(justification)
                            .build();
                        buffer.tag_table().add(&t);
                        t
                    };
                    buffer.apply_tag(&tag, &line_start, &line_end);
                    return;
                }
            }
        }
        if !iter.forward_char() { break; }
    }
}

fn show_resize_dialog(pic: &gtk::Picture) {
    let cur_w = pic.width();
    let cur_h = pic.height();

    let toplevel = pic.root().and_then(|r| r.downcast::<gtk::Window>().ok());

    let dialog = gtk::Window::builder()
        .title("Image Properties")
        .modal(true)
        .resizable(false)
        .default_width(280)
        .build();
    if let Some(ref win) = toplevel {
        dialog.set_transient_for(Some(win));
    }

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    let grid = gtk::Grid::new();
    grid.set_column_spacing(8);
    grid.set_row_spacing(6);

    let w_label = gtk::Label::new(Some("Width:"));
    w_label.set_halign(gtk::Align::End);
    let w_spin = gtk::SpinButton::with_range(16.0, 4000.0, 1.0);
    w_spin.set_value(cur_w as f64);

    let h_label = gtk::Label::new(Some("Height:"));
    h_label.set_halign(gtk::Align::End);
    let h_spin = gtk::SpinButton::with_range(16.0, 4000.0, 1.0);
    h_spin.set_value(cur_h as f64);

    grid.attach(&w_label, 0, 0, 1, 1);
    grid.attach(&w_spin, 1, 0, 1, 1);
    grid.attach(&h_label, 0, 1, 1, 1);
    grid.attach(&h_spin, 1, 1, 1, 1);

    // Alignment (applies to the paragraph in the TextBuffer)
    let align_label = gtk::Label::new(Some("Align:"));
    align_label.set_halign(gtk::Align::End);
    let align_dropdown = gtk::DropDown::from_strings(&["Left", "Center", "Right"]);
    let cur_just = get_image_justification(pic);
    let cur_align_idx = match cur_just {
        gtk::Justification::Center => 1,
        gtk::Justification::Right => 2,
        _ => 0,
    };
    align_dropdown.set_selected(cur_align_idx);
    grid.attach(&align_label, 0, 2, 1, 1);
    grid.attach(&align_dropdown, 1, 2, 1, 1);

    vbox.append(&grid);

    let aspect_check = gtk::CheckButton::with_label("Lock aspect ratio");
    aspect_check.set_active(true);
    vbox.append(&aspect_check);

    let aspect_ratio = if cur_h > 0 { cur_w as f64 / cur_h as f64 } else { 1.0 };
    let updating = std::rc::Rc::new(std::cell::Cell::new(false));

    let h_spin_ref = h_spin.clone();
    let aspect_check_ref = aspect_check.clone();
    let ar = aspect_ratio;
    let upd = updating.clone();
    w_spin.connect_value_changed(move |w| {
        if upd.get() { return; }
        if aspect_check_ref.is_active() {
            upd.set(true);
            h_spin_ref.set_value((w.value() / ar).round());
            upd.set(false);
        }
    });

    let w_spin_ref = w_spin.clone();
    let aspect_check_ref = aspect_check.clone();
    let upd = updating.clone();
    h_spin.connect_value_changed(move |h| {
        if upd.get() { return; }
        if aspect_check_ref.is_active() {
            upd.set(true);
            w_spin_ref.set_value((h.value() * aspect_ratio).round());
            upd.set(false);
        }
    });

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk::Align::End);

    let apply_btn = gtk::Button::with_label("Apply");
    apply_btn.add_css_class("suggested-action");
    let cancel_btn = gtk::Button::with_label("Cancel");
    btn_box.append(&cancel_btn);
    btn_box.append(&apply_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    let pic_ref = pic.clone();
    let dlg = dialog.clone();
    apply_btn.connect_clicked(move |_| {
        let new_w = w_spin.value() as i32;
        let new_h = h_spin.value() as i32;
        pic_ref.set_size_request(new_w, new_h);

        let justification = match align_dropdown.selected() {
            1 => gtk::Justification::Center,
            2 => gtk::Justification::Right,
            _ => gtk::Justification::Left,
        };
        apply_image_justification(&pic_ref, justification);

        dlg.close();
    });

    let dlg = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dlg.close();
    });

    dialog.present();
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

// ── Flex Layout Handling ───────────────────────────────────────────────────

fn handle_flex(
    view: &gtk::TextView,
    node: &Handle,
    buffer: &gtk::TextBuffer,
    ctx: &mut ParseContext,
    css_props: &CssProperties,
) {
    let is_column = css_props.flex_direction.as_deref() == Some("column")
        || css_props.flex_direction.as_deref() == Some("column-reverse");
    let is_wrap = css_props.flex_wrap.as_deref() == Some("wrap")
        || css_props.flex_wrap.as_deref() == Some("wrap-reverse");
    let gap = css_props.gap.unwrap_or(0);

    let tag_name = if let NodeData::Element { ref name, .. } = node.data {
        name.local.to_string()
    } else {
        "div".to_string()
    };
    let raw_attrs = collect_raw_attrs(node, css_props);
    let justify = css_props.justify_content.as_deref().unwrap_or("flex-start");
    let align = css_props.align_items.as_deref().unwrap_or("stretch");

    let available_width = {
        let vw = view.allocated_width();
        if vw > 100 { vw } else { 700 }
    };

    // ── Pre-scan: content widths + explicit CSS widths ──
    let mut child_infos: Vec<(i32, Option<i32>)> = Vec::new(); // (content_w, explicit_px)
    for child in node.children.borrow().iter() {
        if let NodeData::Text { ref contents } = child.data {
            if contents.borrow().trim().is_empty() {
                continue;
            }
        }
        let (explicit_w, pad_h) = if let NodeData::Element { ref name, ref attrs, .. } = child.data {
            let child_tag = name.local.to_string().to_lowercase();
            let child_attrs = attrs.borrow();
            let mut child_style = None;
            let mut child_class = None;
            let mut child_id = None;
            for attr in child_attrs.iter() {
                match attr.name.local.to_string().as_str() {
                    "class" => child_class = Some(attr.value.to_string()),
                    "id" => child_id = Some(attr.value.to_string()),
                    "style" => child_style = Some(attr.value.to_string()),
                    _ => {}
                }
            }
            drop(child_attrs);
            let ccss = apply_css_cascade(
                &child_tag, child_class.as_deref(), child_id.as_deref(),
                child_style.as_deref(), &ctx.css_rules,
            );
            let ew = ccss.width.as_ref().and_then(|w| w.replace("px", "").trim().parse::<i32>().ok());
            let ph = ccss.padding_left.unwrap_or(0) + ccss.padding_right.unwrap_or(0);
            (ew, ph)
        } else {
            (None, 0)
        };
        let mut text_len = 0i32;
        if let NodeData::Element { .. } = child.data {
            for content in child.children.borrow().iter() {
                count_text_length(content, &mut text_len);
            }
        } else if let NodeData::Text { ref contents } = child.data {
            text_len = contents.borrow().trim().len() as i32;
        }
        let content_w = std::cmp::max(20, text_len * 8 + pad_h);
        child_infos.push((content_w, explicit_w));
    }

    // Scale factor for no-wrap mode: shrink children proportionally to fit view
    let num_children = child_infos.len() as i32;
    let scale = if !is_wrap && !is_column && num_children > 0 {
        let total_gaps = gap * (num_children - 1).max(0);
        let available_for_children = available_width - total_gaps;
        let total_content: i32 = child_infos.iter()
            .map(|(cw, ew)| ew.unwrap_or(*cw))
            .sum();
        if total_content > available_for_children && total_content > 0 {
            available_for_children as f64 / total_content as f64
        } else {
            1.0
        }
    } else {
        1.0
    };

    // ── Build container: FlowBox for wrap, Box for no-wrap ──
    let container: gtk::Widget = if is_wrap {
        let fb = gtk::FlowBox::new();
        fb.set_orientation(if is_column {
            gtk::Orientation::Vertical
        } else {
            gtk::Orientation::Horizontal
        });
        fb.set_column_spacing(gap as u32);
        fb.set_row_spacing(gap as u32);
        fb.set_homogeneous(false);
        fb.set_selection_mode(gtk::SelectionMode::None);
        fb.set_hexpand(true);
        fb.set_halign(gtk::Align::Fill);
        fb.set_focusable(false);
        fb.set_can_target(true);
        fb.set_max_children_per_line(if is_column { 1 } else { 100 });
        fb.set_min_children_per_line(0);
        fb.set_widget_name(&format!("flex:{}|{}", tag_name, raw_attrs));
        // FlowBox always needs a width constraint to know when to wrap
        fb.set_size_request(available_width, -1);
        fb.upcast::<gtk::Widget>()
    } else {
        let orientation = if is_column {
            gtk::Orientation::Vertical
        } else {
            gtk::Orientation::Horizontal
        };
        let gbox = gtk::Box::new(orientation, gap);
        gbox.set_hexpand(true);
        gbox.set_halign(gtk::Align::Fill);
        gbox.set_focusable(false);
        gbox.set_can_target(true);
        gbox.set_widget_name(&format!("flex:{}|{}", tag_name, raw_attrs));
        // Only force 100% width if content needed to be scaled down
        if scale < 1.0 {
            gbox.set_size_request(available_width, -1);
        }
        gbox.upcast::<gtk::Widget>()
    };

    // ── Create child widgets ──
    let mut child_idx = 0usize;
    for child in node.children.borrow().iter() {
        if let NodeData::Text { ref contents } = child.data {
            if contents.borrow().trim().is_empty() {
                continue;
            }
        }

        let (child_tag_str, child_class, child_id, child_style) =
            if let NodeData::Element { ref name, ref attrs, .. } = child.data {
                let tag = name.local.to_string().to_lowercase();
                let child_attrs = attrs.borrow();
                let mut cls = None;
                let mut id = None;
                let mut sty = None;
                for attr in child_attrs.iter() {
                    match attr.name.local.to_string().as_str() {
                        "class" => cls = Some(attr.value.to_string()),
                        "id" => id = Some(attr.value.to_string()),
                        "style" => sty = Some(attr.value.to_string()),
                        _ => {}
                    }
                }
                (tag, cls, id, sty)
            } else {
                ("div".to_string(), None, None, None)
            };

        let child_css = apply_css_cascade(
            &child_tag_str, child_class.as_deref(), child_id.as_deref(),
            child_style.as_deref(), &ctx.css_rules,
        );

        // Resolve :hover CSS for this child
        let child_hover_css = if !ctx.hover_rules.is_empty() {
            let delta = apply_css_cascade(
                &child_tag_str, child_class.as_deref(), child_id.as_deref(),
                child_style.as_deref(), &ctx.hover_rules,
            );
            if has_meaningful_css(&delta) { Some(delta) } else { None }
        } else { None };

        let child_view = gtk::TextView::new();
        child_view.set_wrap_mode(gtk::WrapMode::WordChar);
        child_view.set_vexpand(false);
        child_view.set_focusable(true);
        child_view.set_can_focus(true);
        child_view.set_editable(true);

        // Apply alignment from parent's justify-content / align-items
        if is_column {
            match align {
                "center" => child_view.set_halign(gtk::Align::Center),
                "flex-end" | "end" => child_view.set_halign(gtk::Align::End),
                _ => child_view.set_halign(gtk::Align::Fill),
            }
            match justify {
                "center" => child_view.set_valign(gtk::Align::Center),
                "flex-end" | "end" => child_view.set_valign(gtk::Align::End),
                "space-between" | "space-around" | "space-evenly" => child_view.set_vexpand(true),
                _ => {}
            }
        } else {
            match justify {
                "center" => child_view.set_halign(gtk::Align::Center),
                "flex-end" | "end" => child_view.set_halign(gtk::Align::End),
                _ => {}
            }
            match align {
                "center" => child_view.set_valign(gtk::Align::Center),
                "flex-start" | "start" => child_view.set_valign(gtk::Align::Start),
                "flex-end" | "end" => child_view.set_valign(gtk::Align::End),
                _ => child_view.set_valign(gtk::Align::Fill),
            }
        }

        let child_raw = collect_raw_attrs(child, &child_css);
        let child_tag = if let NodeData::Element { ref name, .. } = child.data {
            name.local.to_string()
        } else {
            "div".to_string()
        };
        child_view.set_widget_name(&format!("flexchild:{}|{}", child_tag, child_raw));

        let cv = child_view.clone();
        let click = gtk::GestureClick::new();
        click.connect_pressed(move |gesture, _n, _x, _y| {
            cv.grab_focus();
            gesture.set_state(gtk::EventSequenceState::Claimed);
        });
        child_view.add_controller(click);

        let child_buffer = child_view.buffer();
        crate::setup_tags(&child_buffer);

        if let NodeData::Element { .. } = child.data {
            for content in child.children.borrow().iter() {
                walk_dom(&child_view, content, &child_buffer, ctx);
            }
        } else if let NodeData::Text { ref contents } = child.data {
            let text = contents.borrow().to_string();
            if !text.trim().is_empty() {
                let mut end_iter = child_buffer.end_iter();
                child_buffer.insert(&mut end_iter, text.trim());
            }
        }

        // Apply child width
        if let Some((content_w, explicit_w)) = child_infos.get(child_idx) {
            let raw_w = explicit_w.unwrap_or(*content_w);
            if is_wrap {
                // Wrap mode: use content width as-is, FlowBox handles overflow
                child_view.set_size_request(raw_w, -1);
            } else if is_column {
                child_view.set_size_request(raw_w, -1);
            } else {
                let final_w = ((raw_w as f64 * scale) as i32).max(20);
                child_view.set_size_request(final_w, -1);
            }
            if explicit_w.is_some() {
                child_view.set_hexpand(false);
            }
        }

        let provider = apply_child_css_provider_with_hover(&child_view, &child_css, child_hover_css.as_ref());

        // Store CssProvider per element ID for live class manipulation
        if let Some(ref id) = child_id {
            if let Some(prov) = provider {
                ctx.element_providers.insert(id.clone(), prov);
            }
            // Record element metadata
            let classes = child_class.as_deref().unwrap_or("")
                .split_whitespace().map(|s| s.to_string()).collect();
            ctx.element_meta.insert(id.clone(), ElementMeta {
                tag_name: child_tag_str.clone(),
                classes,
                inline_style: child_style.clone(),
            });
            if let Some(ref cls) = child_class {
                if !cls.is_empty() {
                    ctx.css_rules_store.insert(format!("classattr:{}", id), cls.clone());
                }
            }
        }

        // Append to container
        if is_wrap {
            if let Some(fb) = container.downcast_ref::<gtk::FlowBox>() {
                fb.insert(&child_view, -1);
            }
        } else if let Some(gbox) = container.downcast_ref::<gtk::Box>() {
            gbox.append(&child_view);
        }
        child_idx += 1;
    }

    ensure_newline(buffer);
    let mut end_iter = buffer.end_iter();
    let anchor = buffer.create_child_anchor(&mut end_iter);
    view.add_child_at_anchor(&container, &anchor);
    buffer.insert(&mut end_iter, "\n");
}

// ── CSS Grid Layout Handling ──────────────────────────────────────────────

fn handle_css_grid(
    view: &gtk::TextView,
    node: &Handle,
    buffer: &gtk::TextBuffer,
    ctx: &mut ParseContext,
    css_props: &CssProperties,
) {
    // Parse grid-template-columns to determine column count and widths
    let template = css_props.grid_template_columns.as_deref().unwrap_or("1fr");
    let col_specs: Vec<&str> = template.split_whitespace().collect();
    let num_cols = col_specs.len().max(1) as i32;

    let gap = css_props.gap.unwrap_or(0);
    let grid = gtk::Grid::new();
    grid.set_column_spacing(gap as u32);
    grid.set_row_spacing(gap as u32);
    grid.set_focusable(false);
    grid.set_can_target(true);
    grid.set_hexpand(true);
    grid.set_halign(gtk::Align::Fill);

    // Store original element + style for round-trip serialization
    let tag_name = if let NodeData::Element { ref name, .. } = node.data {
        name.local.to_string()
    } else {
        "div".to_string()
    };
    let raw_attrs = collect_raw_attrs(node, css_props);
    grid.set_widget_name(&format!("cssgrid:{}|{}", tag_name, raw_attrs));

    // ── Pass 1: pre-calculate column widths (like table cells) ──
    let mut col_max_chars: HashMap<i32, i32> = HashMap::new();
    let mut col_explicit_px: HashMap<i32, i32> = HashMap::new();
    {
        let mut col = 0i32;
        for child in node.children.borrow().iter() {
            if let NodeData::Text { ref contents } = child.data {
                if contents.borrow().trim().is_empty() {
                    continue;
                }
            }

            // Resolve child CSS to get grid-column span
            let child_css = if let NodeData::Element { ref name, ref attrs, .. } = child.data {
                let child_tag = name.local.to_string().to_lowercase();
                let child_attrs = attrs.borrow();
                let mut child_style = None;
                let mut child_class = None;
                let mut child_id = None;
                for attr in child_attrs.iter() {
                    match attr.name.local.to_string().as_str() {
                        "class" => child_class = Some(attr.value.to_string()),
                        "id" => child_id = Some(attr.value.to_string()),
                        "style" => child_style = Some(attr.value.to_string()),
                        _ => {}
                    }
                }
                drop(child_attrs);
                apply_css_cascade(
                    &child_tag,
                    child_class.as_deref(),
                    child_id.as_deref(),
                    child_style.as_deref(),
                    &ctx.css_rules,
                )
            } else {
                CssProperties::default()
            };

            let colspan = child_css.grid_column.as_ref()
                .map(|gc| parse_grid_span(gc, num_cols))
                .unwrap_or(1);

            // Count text in this child + padding
            let mut text_len = 0i32;
            if let NodeData::Element { .. } = child.data {
                for content in child.children.borrow().iter() {
                    count_text_length(content, &mut text_len);
                }
            } else if let NodeData::Text { ref contents } = child.data {
                text_len = contents.borrow().trim().len() as i32;
            }
            let pad_h = child_css.padding_left.unwrap_or(0) + child_css.padding_right.unwrap_or(0);

            // Store as chars + padding (padding added once, not per-column)
            let per_col = text_len / colspan.max(1);
            let pad_per_col = pad_h / colspan.max(1);
            for c in 0..colspan {
                let column = col + c;
                let current_max = *col_max_chars.get(&column).unwrap_or(&0);
                col_max_chars.insert(column, std::cmp::max(current_max, per_col * 8 + pad_per_col));
            }

            col += colspan;
            if col >= num_cols {
                col = 0;
            }
        }

        // Store explicit px widths from grid-template-columns
        for (i, spec) in col_specs.iter().enumerate() {
            if let Ok(px) = spec.replace("px", "").trim().parse::<i32>() {
                col_explicit_px.insert(i as i32, px);
            }
        }
    }

    // Calculate available width and scale factor
    let grid_available_width = {
        let vw = view.allocated_width();
        if vw > 100 { vw } else { 700 }
    };
    let total_col_gaps = gap * (num_cols - 1).max(0);
    let available_for_cols = grid_available_width - total_col_gaps;

    // Sum up column widths (explicit px or content-based)
    let total_col_content: i32 = (0..num_cols).map(|c| {
        col_explicit_px.get(&c).copied()
            .unwrap_or_else(|| std::cmp::max(20, col_max_chars.get(&c).copied().unwrap_or(0)))
    }).sum();

    let grid_scale = if total_col_content > available_for_cols && total_col_content > 0 {
        available_for_cols as f64 / total_col_content as f64
    } else {
        1.0
    };

    // ── Pass 2: create child widgets ──
    let mut col = 0i32;
    let mut row = 0i32;

    for child in node.children.borrow().iter() {
        // Skip whitespace text nodes
        if let NodeData::Text { ref contents } = child.data {
            if contents.borrow().trim().is_empty() {
                continue;
            }
        }

        // Resolve child CSS
        let (child_tag_str, child_class, child_id, child_style) =
            if let NodeData::Element { ref name, ref attrs, .. } = child.data {
                let tag = name.local.to_string().to_lowercase();
                let child_attrs = attrs.borrow();
                let mut cls = None;
                let mut id = None;
                let mut sty = None;
                for attr in child_attrs.iter() {
                    match attr.name.local.to_string().as_str() {
                        "class" => cls = Some(attr.value.to_string()),
                        "id" => id = Some(attr.value.to_string()),
                        "style" => sty = Some(attr.value.to_string()),
                        _ => {}
                    }
                }
                (tag, cls, id, sty)
            } else {
                ("div".to_string(), None, None, None)
            };

        let child_css = apply_css_cascade(
            &child_tag_str, child_class.as_deref(), child_id.as_deref(),
            child_style.as_deref(), &ctx.css_rules,
        );

        // Resolve :hover CSS for this grid child
        let child_hover_css = if !ctx.hover_rules.is_empty() {
            let delta = apply_css_cascade(
                &child_tag_str, child_class.as_deref(), child_id.as_deref(),
                child_style.as_deref(), &ctx.hover_rules,
            );
            if has_meaningful_css(&delta) { Some(delta) } else { None }
        } else { None };

        let child_view = gtk::TextView::new();
        child_view.set_wrap_mode(gtk::WrapMode::WordChar);
        child_view.set_hexpand(true);
        child_view.set_vexpand(false);
        child_view.set_halign(gtk::Align::Fill);
        child_view.set_focusable(true);
        child_view.set_can_focus(true);
        child_view.set_editable(true);

        // Parse grid-column span (e.g. "span 2", "1 / 3", "1 / span 2")
        let mut colspan = 1i32;
        let mut rowspan = 1i32;
        if let Some(ref gc) = child_css.grid_column {
            colspan = parse_grid_span(gc, num_cols);
        }
        if let Some(ref gr) = child_css.grid_row {
            rowspan = parse_grid_span(gr, 100);
        }

        // Store child's raw attrs for round-trip
        let child_raw = collect_raw_attrs(child, &child_css);
        let child_tag = if let NodeData::Element { ref name, .. } = child.data {
            name.local.to_string()
        } else {
            "div".to_string()
        };
        child_view.set_widget_name(&format!("gridchild:{}|{}", child_tag, child_raw));

        // Focus on click
        let cv = child_view.clone();
        let click = gtk::GestureClick::new();
        click.connect_pressed(move |gesture, _n, _x, _y| {
            cv.grab_focus();
            gesture.set_state(gtk::EventSequenceState::Claimed);
        });
        child_view.add_controller(click);

        let child_buffer = child_view.buffer();
        crate::setup_tags(&child_buffer);

        // Walk child content
        if let NodeData::Element { .. } = child.data {
            for content in child.children.borrow().iter() {
                walk_dom(&child_view, content, &child_buffer, ctx);
            }
        } else if let NodeData::Text { ref contents } = child.data {
            let text = contents.borrow().to_string();
            if !text.trim().is_empty() {
                let mut end_iter = child_buffer.end_iter();
                child_buffer.insert(&mut end_iter, text.trim());
            }
        }

        // Apply cell width: explicit px from grid-template-columns, or
        // content-based scaled to fit view width (like flex)
        if let Some(&px) = col_explicit_px.get(&col) {
            child_view.set_size_request(px * colspan, -1);
            child_view.set_hexpand(false);
        } else {
            let mut max_chars_in_cols = 0;
            for c in 0..colspan {
                max_chars_in_cols += *col_max_chars.get(&(col + c)).unwrap_or(&0);
            }
            let raw_w = std::cmp::max(20, max_chars_in_cols);
            let final_w = ((raw_w as f64 * grid_scale) as i32).max(20);
            child_view.set_size_request(final_w, -1);
        }

        // Apply child CSS via provider (with hover if available)
        let provider = apply_child_css_provider_with_hover(&child_view, &child_css, child_hover_css.as_ref());

        // Store CssProvider per element ID for live class manipulation
        if let Some(ref id) = child_id {
            if let Some(prov) = provider {
                ctx.element_providers.insert(id.clone(), prov);
            }
            let classes = child_class.as_deref().unwrap_or("")
                .split_whitespace().map(|s| s.to_string()).collect();
            ctx.element_meta.insert(id.clone(), ElementMeta {
                tag_name: child_tag_str.clone(),
                classes,
                inline_style: child_style.clone(),
            });
            if let Some(ref cls) = child_class {
                if !cls.is_empty() {
                    ctx.css_rules_store.insert(format!("classattr:{}", id), cls.clone());
                }
            }
        }

        grid.attach(&child_view, col, row, colspan, rowspan);

        col += colspan;
        if col >= num_cols {
            col = 0;
            row += 1;
        }
    }

    // Only force 100% width if content needed to be scaled down
    if grid_scale < 1.0 {
        grid.set_size_request(grid_available_width, -1);
    }

    ensure_newline(buffer);
    let mut end_iter = buffer.end_iter();
    let anchor = buffer.create_child_anchor(&mut end_iter);
    view.add_child_at_anchor(&grid, &anchor);
    buffer.insert(&mut end_iter, "\n");
}

// ── Shared helpers for flex/grid ──────────────────────────────────────────

/// Parse a CSS grid-column/grid-row value into a span count.
/// Supports: "span 2", "1 / 3" (=> span 2), "1 / span 2", "1 / -1" (full row).
fn parse_grid_span(val: &str, max_cols: i32) -> i32 {
    let val = val.trim();
    // "span N"
    if let Some(rest) = val.strip_prefix("span") {
        return rest.trim().parse::<i32>().unwrap_or(1).max(1);
    }
    // "start / end" or "start / span N"
    if let Some((start_s, end_s)) = val.split_once('/') {
        let end_s = end_s.trim();
        if let Some(rest) = end_s.strip_prefix("span") {
            return rest.trim().parse::<i32>().unwrap_or(1).max(1);
        }
        let start: i32 = start_s.trim().parse().unwrap_or(1);
        let end: i32 = end_s.parse().unwrap_or(start + 1);
        if end == -1 {
            return (max_cols - start + 1).max(1);
        }
        return (end - start).max(1);
    }
    1
}

/// Collect raw HTML attributes from a DOM node, merging in cascade-resolved
/// styles that aren't already present in the raw style attribute.
fn collect_raw_attrs(node: &Handle, css_props: &CssProperties) -> String {
    let mut raw_attrs: Vec<(String, String)> = Vec::new();
    if let NodeData::Element { ref attrs, .. } = node.data {
        for attr in attrs.borrow().iter() {
            raw_attrs.push((attr.name.local.to_string(), attr.value.to_string()));
        }
    }
    // If no inline style but cascade resolved properties, add them
    let has_style = raw_attrs.iter().any(|(k, _)| k == "style");
    if !has_style {
        let css_str = css_props.to_css_string();
        if !css_str.is_empty() {
            raw_attrs.push(("style".to_string(), css_str));
        }
    }
    raw_attrs
        .iter()
        .map(|(k, v)| format!("{}=\"{}\"", k, v.replace('"', "&quot;")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build a GTK CSS string from CssProperties (and optional hover variant).
pub fn build_widget_css_string(css: &CssProperties, hover_css: Option<&CssProperties>) -> String {
    let mut css_parts = Vec::new();
    if let Some(ref bg) = css.background_color {
        css_parts.push(format!("background-color: {};", bg));
    }
    if let Some(ref c) = css.color {
        css_parts.push(format!("color: {};", c));
    }
    if let Some(ref ff) = css.font_family {
        css_parts.push(format!("font-family: {};", ff));
    }
    if let Some(fs) = css.font_size {
        css_parts.push(format!("font-size: {}pt;", fs));
    }
    if css.has_border() {
        for (side, has, w, st, c) in [
            ("top", css.has_border_top(), css.border_top_width, css.border_top_style, &css.border_top_color),
            ("right", css.has_border_right(), css.border_right_width, css.border_right_style, &css.border_right_color),
            ("bottom", css.has_border_bottom(), css.border_bottom_width, css.border_bottom_style, &css.border_bottom_color),
            ("left", css.has_border_left(), css.border_left_width, css.border_left_style, &css.border_left_color),
        ] {
            if has {
                let wv = w.unwrap_or(1);
                let sv = CssProperties::border_style_str(st);
                let cv = c.as_deref().unwrap_or("alpha(currentColor, 0.3)");
                css_parts.push(format!("border-{}: {}px {} {};", side, wv, sv, cv));
            }
        }
    }
    if let Some(ref br) = css.border_radius {
        css_parts.push(format!("border-radius: {};", br));
    }
    if let Some(ref mw) = css.max_width {
        if let Ok(px) = mw.replace("px", "").trim().parse::<i32>() {
            css_parts.push(format!("max-width: {}px;", px));
        }
    }
    if let Some(v) = css.opacity {
        if v < 1.0 {
            css_parts.push(format!("opacity: {};", v));
        }
    }
    if let Some(ref bs) = css.box_shadow {
        css_parts.push(format!("box-shadow: {};", bs));
    }
    if let Some(ref ts) = css.text_shadow {
        css_parts.push(format!("text-shadow: {};", ts));
    }
    if let Some(ref bg) = css.background_image {
        css_parts.push(format!("background-image: {};", bg));
    }

    let mut hover_parts = Vec::new();
    if let Some(hcss) = hover_css {
        if let Some(ref bg) = hcss.background_color {
            hover_parts.push(format!("background-color: {};", bg));
        }
        if let Some(ref c) = hcss.color {
            hover_parts.push(format!("color: {};", c));
        }
        if let Some(ref bs) = hcss.box_shadow {
            hover_parts.push(format!("box-shadow: {};", bs));
        }
        if let Some(ref ts) = hcss.text_shadow {
            hover_parts.push(format!("text-shadow: {};", ts));
        }
        if let Some(v) = hcss.opacity {
            hover_parts.push(format!("opacity: {};", v));
        }
        if hcss.has_border() {
            for (side, has, w, st, c) in [
                ("top", hcss.has_border_top(), hcss.border_top_width, hcss.border_top_style, &hcss.border_top_color),
                ("right", hcss.has_border_right(), hcss.border_right_width, hcss.border_right_style, &hcss.border_right_color),
                ("bottom", hcss.has_border_bottom(), hcss.border_bottom_width, hcss.border_bottom_style, &hcss.border_bottom_color),
                ("left", hcss.has_border_left(), hcss.border_left_width, hcss.border_left_style, &hcss.border_left_color),
            ] {
                if has {
                    let wv = w.unwrap_or(1);
                    let sv = CssProperties::border_style_str(st);
                    let cv = c.as_deref().unwrap_or("currentColor");
                    hover_parts.push(format!("border-{}: {}px {} {};", side, wv, sv, cv));
                }
            }
        }
        if let Some(ref br) = hcss.border_radius {
            hover_parts.push(format!("border-radius: {};", br));
        }
    }

    let mut result = String::new();
    if !css_parts.is_empty() {
        result.push_str(&format!("textview {{ {} }}", css_parts.join(" ")));
    }
    if !hover_parts.is_empty() {
        result.push_str(&format!(" textview:hover {{ {} }}", hover_parts.join(" ")));
    }
    result
}

/// Apply CSS properties (background, border, color, font, padding) to a child
/// TextView via a GTK CssProvider. Returns the provider if one was created.
#[allow(deprecated)]
fn apply_child_css_provider_with_hover(child_view: &gtk::TextView, css: &CssProperties, hover_css: Option<&CssProperties>) -> Option<gtk::CssProvider> {
    // Apply padding via widget margins
    if let Some(p) = css.padding_top { child_view.set_top_margin(p); }
    if let Some(p) = css.padding_bottom { child_view.set_bottom_margin(p); }
    if let Some(p) = css.padding_left { child_view.set_left_margin(p); }
    if let Some(p) = css.padding_right { child_view.set_right_margin(p); }
    if css.padding_top.is_some() && css.padding_top == css.padding_bottom
        && css.padding_top == css.padding_left && css.padding_top == css.padding_right
    {
        let p = css.padding_top.unwrap();
        child_view.set_top_margin(p);
        child_view.set_bottom_margin(p);
        child_view.set_left_margin(p);
        child_view.set_right_margin(p);
    }

    // Apply min-width as widget size constraint
    if let Some(ref mw) = css.min_width {
        if let Ok(px) = mw.replace("px", "").trim().parse::<i32>() {
            let (cur_w, cur_h) = child_view.size_request();
            child_view.set_size_request(px.max(cur_w), cur_h);
        }
    }

    // Apply vertical-align as widget valign within container
    if let Some(ref va) = css.vertical_align {
        use crate::css::VerticalAlign;
        match va {
            VerticalAlign::Top | VerticalAlign::Super => child_view.set_valign(gtk::Align::Start),
            VerticalAlign::Middle => child_view.set_valign(gtk::Align::Center),
            VerticalAlign::Bottom | VerticalAlign::Sub => child_view.set_valign(gtk::Align::End),
            VerticalAlign::Length(pango_units) => {
                let px = *pango_units / 1024;
                if px > 0 {
                    child_view.set_valign(gtk::Align::Start);
                    let cur = child_view.top_margin();
                    child_view.set_top_margin(cur + px);
                } else if px < 0 {
                    child_view.set_valign(gtk::Align::End);
                    let cur = child_view.bottom_margin();
                    child_view.set_bottom_margin(cur + px.abs());
                }
            }
            VerticalAlign::Baseline => {}
        }
    }

    // Apply position offsets (top/bottom/left/right) as widget margins
    if let Some(ref t) = css.top {
        if let Some(px) = crate::css::parse_px(t) {
            child_view.set_margin_top(child_view.margin_top() + px);
        }
    }
    if let Some(ref b) = css.bottom_pos {
        if let Some(px) = crate::css::parse_px(b) {
            child_view.set_margin_bottom(child_view.margin_bottom() + px);
        }
    }
    if let Some(ref l) = css.left_pos {
        if let Some(px) = crate::css::parse_px(l) {
            child_view.set_margin_start(child_view.margin_start() + px);
        }
    }
    if let Some(ref r) = css.right_pos {
        if let Some(px) = crate::css::parse_px(r) {
            child_view.set_margin_end(child_view.margin_end() + px);
        }
    }

    let css_str = build_widget_css_string(css, hover_css);
    if !css_str.is_empty() {
        let provider = gtk::CssProvider::new();
        provider.load_from_data(&css_str);
        child_view
            .style_context()
            .add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        Some(provider)
    } else {
        None
    }
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

    // Check for border-style: none in CSS
    if css_props.border_top_style == Some(BorderStyle::None) {
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
                    cell_border_color = cell_css.border_top_color.clone();
                    cell_border_style_str = Some(CssProperties::border_style_str(cell_css.border_top_style).to_string());
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
                // Use a reasonable minimum: at least 60px per column so empty cells are usable
                let min_width = std::cmp::max(60, max_chars_in_cols * 8).min(600);
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
