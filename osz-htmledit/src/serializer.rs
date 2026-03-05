use gtk4 as gtk;
use gtk::prelude::*;
use std::collections::HashMap;

/// Serialize a GTK TextBuffer back to HTML.
pub fn serialize_buffer(buffer: &gtk::TextBuffer, css_rules_store: &HashMap<String, String>) -> String {
    serialize_range(buffer, &buffer.start_iter(), &buffer.end_iter(), css_rules_store)
}

/// Serialize a range of a GTK TextBuffer to HTML.
pub fn serialize_range(
    _buffer: &gtk::TextBuffer,
    start: &gtk::TextIter,
    end_bound: &gtk::TextIter,
    css_rules_store: &HashMap<String, String>,
) -> String {
    let mut html = String::new();
    let mut iter = *start;
    let end = *end_bound;

    let mut text_acc = String::new();
    let mut active_tags: Vec<String> = Vec::new();
    let mut active_ids: Vec<String> = Vec::new();

    while iter < end {
        let tags = iter.tags();
        let mut is_marker = false;
        let mut current_pos_tags: Vec<String> = Vec::new();
        let mut current_ids: Vec<String> = Vec::new();

        let mut comment_text: Option<String> = None;
        for tag in tags.iter() {
            if let Some(name) = tag.name() {
                let name_str = name.as_str().to_string();
                if name_str == "list_marker" {
                    is_marker = true;
                } else if let Some(text) = name_str.strip_prefix("comment:") {
                    comment_text = Some(text.to_string());
                } else if let Some(id) = name_str.strip_prefix("editable_id:") {
                    current_ids.push(id.to_string());
                } else if name_str == "_readonly" || name_str == "_editable" {
                    // Internal readonly markers — skip silently
                } else if is_recognized_tag(&name_str) {
                    current_pos_tags.push(name_str);
                }
            }
        }
        // If a link: tag exists, suppress the bare "a" tag (link: handles it with href)
        let has_link = current_pos_tags.iter().any(|t| t.starts_with("link:"));
        if has_link {
            current_pos_tags.retain(|t| t != "a");
        }
        // If an abbr_title: tag exists, suppress abbr_style (abbr_title handles the <abbr> element)
        let has_abbr_title = current_pos_tags.iter().any(|t| t.starts_with("abbr_title:"));
        if has_abbr_title {
            current_pos_tags.retain(|t| t != "abbr_style");
        }
        current_pos_tags.sort();
        current_ids.sort();

        // Determine newly started IDs at this position
        let new_ids: Vec<&String> = current_ids.iter()
            .filter(|id| !active_ids.contains(id))
            .collect();

        if current_pos_tags != active_tags {
            if !text_acc.is_empty() {
                html.push_str(&text_acc);
                text_acc.clear();
            }

            // Close tags that are no longer active (in reverse order)
            for tag in active_tags.iter().rev() {
                if !current_pos_tags.contains(tag) {
                    html.push_str(&format!("</{}>", close_tag_name(tag)));
                }
            }

            // Open new tags, injecting id= into the first new tag if we have new IDs
            let mut id_injected = new_ids.is_empty();
            for tag in current_pos_tags.iter() {
                if !active_tags.contains(tag) {
                    if !id_injected {
                        html.push_str(&open_tag_markup_with_id(tag, css_rules_store, &new_ids));
                        id_injected = true;
                    } else {
                        html.push_str(&open_tag_markup(tag, css_rules_store));
                    }
                }
            }

            active_tags = current_pos_tags.clone();
        } else if !new_ids.is_empty() {
            // Tags didn't change but new IDs started — no new HTML tag to attach to.
            // This shouldn't happen in practice since the id tag should start with the element tag.
        }

        active_ids = current_ids;

        let c = iter.char();

        // Skip list marker characters
        if is_marker {
            iter.forward_char();
            continue;
        }

        // Emit HTML comment and skip the zero-width marker
        if let Some(ref ct) = comment_text {
            if !text_acc.is_empty() {
                html.push_str(&text_acc);
                text_acc.clear();
            }
            html.push_str(&format!("<!--{}-->", ct));
            iter.forward_char();
            continue;
        }

        // Handle embedded widgets (tables, HR)
        if c == '\u{FFFC}' {
            if !text_acc.is_empty() {
                html.push_str(&text_acc);
                text_acc.clear();
            }
            serialize_widget_anchor(&iter, &mut html, css_rules_store);
        } else if c == '\n' {
            if !text_acc.is_empty() {
                html.push_str(&text_acc);
                text_acc.clear();
            }

            // Inside a list container but not in a <li>: just emit newline
            let in_list_but_not_li = active_tags.iter().any(|t| t.starts_with("ul_") || t.starts_with("ol_"))
                && !active_tags.contains(&"li".to_string());
            if !in_list_but_not_li {
                html.push_str("<br>\n");
            } else {
                html.push('\n');
            }
        } else if c != '\0' {
            // HTML-escape special characters
            match c {
                '<' => text_acc.push_str("&lt;"),
                '>' => text_acc.push_str("&gt;"),
                '&' => text_acc.push_str("&amp;"),
                '"' => text_acc.push_str("&quot;"),
                '\u{00A0}' => text_acc.push_str("&nbsp;"),
                _ => text_acc.push(c),
            }
        }

        iter.forward_char();
    }

    // Flush remaining text
    if !text_acc.is_empty() {
        html.push_str(&text_acc);
    }

    // Close all remaining open tags
    for tag in active_tags.iter().rev() {
        html.push_str(&format!("</{}>", close_tag_name(tag)));
    }

    html
}

// ── Tag Recognition ────────────────────────────────────────────────────────

fn is_recognized_tag(name: &str) -> bool {
    // Structural HTML tags
    if matches!(
        name,
        "b" | "strong" | "i" | "em" | "u" | "a"
            | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            | "pre" | "blockquote" | "s" | "li"
            | "align_center" | "align_right" | "align_justify"
            | "sub" | "sup" | "small" | "big" | "mark" | "code"
            | "abbr_style"
    ) {
        return true;
    }
    // Dynamic tags
    if name.starts_with("color: ")
        || name.starts_with("bgcolor:")
        || name.starts_with("ul_")
        || name.starts_with("ol_")
        || name.starts_with("css_")
        || name.starts_with("link:")
        || name.starts_with("blockquote_")
        || name.starts_with("indent_")
        || name.starts_with("comment:")
        || name.starts_with("font:")
        || name.starts_with("size:")
        || name.starts_with("abbr_title:")
    {
        return true;
    }
    false
}

// ── Tag → HTML Mapping ─────────────────────────────────────────────────────

fn open_tag_markup(tag: &str, css_rules_store: &HashMap<String, String>) -> String {
    if tag.starts_with("css_") {
        // Reconstruct inline style from stored rules
        let style = css_rules_store.get(tag).cloned().unwrap_or_default();
        let esc = style.replace('"', "&quot;");
        // Determine if block-level based on tag naming convention
        if tag.starts_with("css_b_") {
            format!("<div style=\"{}\">", esc)
        } else {
            format!("<span style=\"{}\">", esc)
        }
    } else if tag.starts_with("link:") {
        let url = tag.strip_prefix("link:").unwrap_or("");
        let esc = url.replace('"', "&quot;");
        format!("<a href=\"{}\">", esc)
    } else if tag.starts_with("ul_") {
        "<ul>".to_string()
    } else if tag.starts_with("ol_") {
        // Check for start= encoding: ol_{depth}_s{start}
        if let Some(s_pos) = tag.find("_s") {
            let start_val = &tag[s_pos + 2..];
            format!("<ol start=\"{}\">", start_val)
        } else {
            "<ol>".to_string()
        }
    } else if tag.starts_with("color: ") {
        let color = tag.strip_prefix("color: ").unwrap_or("");
        format!("<span style=\"color: {}\">", color)
    } else if tag.starts_with("bgcolor:") {
        let color = tag.strip_prefix("bgcolor:").unwrap_or("");
        format!("<span style=\"background-color: {}\">", color)
    } else if tag.starts_with("font:") {
        let family = tag.strip_prefix("font:").unwrap_or("");
        format!("<span style=\"font-family: {}\">", family)
    } else if tag.starts_with("size:") {
        let size = tag.strip_prefix("size:").unwrap_or("12");
        format!("<span style=\"font-size: {}pt\">", size)
    } else if tag == "align_center" {
        "<div style=\"text-align: center;\">".to_string()
    } else if tag == "align_right" {
        "<div style=\"text-align: right;\">".to_string()
    } else if tag == "align_justify" {
        "<div style=\"text-align: justify;\">".to_string()
    } else if tag.starts_with("blockquote_") {
        "<blockquote>".to_string()
    } else if let Some(level) = tag.strip_prefix("indent_") {
        if let Ok(n) = level.parse::<i32>() {
            format!("<div style=\"margin-left: {}px\">", n * 40)
        } else {
            "<div>".to_string()
        }
    } else if tag == "abbr_style" {
        // No abbr_title: present — emit plain <abbr>
        "<abbr>".to_string()
    } else if tag.starts_with("abbr_title:") {
        let title = tag.strip_prefix("abbr_title:").unwrap_or("");
        let esc = title.replace('"', "&quot;");
        format!("<abbr title=\"{}\">", esc)
    } else {
        format!("<{}>", tag)
    }
}

/// Like `open_tag_markup` but injects `id="..."` into the opening tag.
fn open_tag_markup_with_id(tag: &str, css_rules_store: &HashMap<String, String>, ids: &[&String]) -> String {
    let markup = open_tag_markup(tag, css_rules_store);
    if ids.is_empty() {
        return markup;
    }
    // Use the first ID (elements should only have one id)
    let id = ids[0];
    let mut attrs = format!(" id=\"{}\"", id);
    // Also emit class= if stored
    let class_key = format!("classattr:{}", id);
    if let Some(classes) = css_rules_store.get(&class_key) {
        if !classes.is_empty() {
            attrs.push_str(&format!(" class=\"{}\"", classes));
        }
    }
    // Inject before the first '>'
    if let Some(pos) = markup.find('>') {
        let mut result = String::with_capacity(markup.len() + attrs.len());
        result.push_str(&markup[..pos]);
        result.push_str(&attrs);
        result.push_str(&markup[pos..]);
        result
    } else {
        markup
    }
}

fn close_tag_name(tag: &str) -> String {
    if tag.starts_with("ul_") {
        "ul".to_string()
    } else if tag.starts_with("ol_") {
        "ol".to_string()
    } else if tag.starts_with("color: ") || tag.starts_with("bgcolor:") || tag.starts_with("font:") || tag.starts_with("size:") {
        "span".to_string()
    } else if tag.starts_with("link:") {
        "a".to_string()
    } else if tag.starts_with("align_") || tag.starts_with("css_b_") {
        "div".to_string()
    } else if tag.starts_with("css_") {
        "span".to_string()
    } else if tag.starts_with("blockquote_") {
        "blockquote".to_string()
    } else if tag.starts_with("indent_") {
        "div".to_string()
    } else if tag == "abbr_style" {
        "abbr".to_string()
    } else if tag.starts_with("abbr_title:") {
        "abbr".to_string()
    } else {
        tag.to_string()
    }
}

// ── Widget Anchor Serialization ────────────────────────────────────────────

fn serialize_widget_anchor(iter: &gtk::TextIter, html: &mut String, css_rules_store: &HashMap<String, String>) {
    if let Some(anchor) = iter.child_anchor() {
        let widgets = anchor.widgets();
        for widget in widgets.iter() {
            // Check for Box (link preview or flex container)
            if let Some(gbox) = widget.downcast_ref::<gtk::Box>() {
                let name = gbox.widget_name().to_string();
                if let Some(url) = name.strip_prefix("link_preview:") {
                    html.push_str(&format!(
                        "<a href=\"{}\">{}</a>",
                        url.replace('"', "&quot;"),
                        url.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"),
                    ));
                    return;
                }
                if name.starts_with("flex:") {
                    serialize_flex(gbox, html, css_rules_store);
                    return;
                }
            }
            // Check for FlowBox (flex container, wrap)
            if let Some(fb) = widget.downcast_ref::<gtk::FlowBox>() {
                let name = fb.widget_name().to_string();
                if name.starts_with("flex:") {
                    serialize_flex_flowbox(fb, html, css_rules_store);
                    return;
                }
            }
            // Check for Grid (table or CSS grid)
            if let Some(grid) = widget.downcast_ref::<gtk::Grid>() {
                let name = grid.widget_name().to_string();
                if name.starts_with("cssgrid:") {
                    serialize_css_grid(grid, html, css_rules_store);
                } else {
                    serialize_grid_as_table(grid, html, css_rules_store);
                }
                return;
            }
            // Check for Picture (img or svg)
            if let Some(pic) = widget.downcast_ref::<gtk::Picture>() {
                let name = pic.widget_name().to_string();
                if name.starts_with("svg:") {
                    if let Some(svg_source) = css_rules_store.get(&name) {
                        html.push_str(svg_source);
                    }
                } else if let Some(rest) = name.strip_prefix("img:") {
                    // Parse pipe-delimited parts: img:URL|alt:TEXT|pctw:50%|pcth:auto
                    let parts: Vec<&str> = rest.splitn(2, '|').collect();
                    let src = parts[0];
                    let mut alt: Option<&str> = None;
                    let mut pctw: Option<&str> = None;
                    let mut pcth: Option<&str> = None;
                    if parts.len() > 1 {
                        for segment in parts[1].split('|') {
                            if let Some(v) = segment.strip_prefix("alt:") { alt = Some(v); }
                            else if let Some(v) = segment.strip_prefix("pctw:") { pctw = Some(v); }
                            else if let Some(v) = segment.strip_prefix("pcth:") { pcth = Some(v); }
                        }
                    }
                    html.push_str(&format!("<img src=\"{}\"", src));
                    if let Some(a) = alt {
                        html.push_str(&format!(" alt=\"{}\"", a));
                    }
                    // Use original percentage string if available, otherwise pixel value
                    if let Some(pw) = pctw {
                        html.push_str(&format!(" width=\"{}\"", pw));
                    } else {
                        let w = pic.width_request();
                        if w > 0 { html.push_str(&format!(" width=\"{}\"", w)); }
                    }
                    if let Some(ph) = pcth {
                        html.push_str(&format!(" height=\"{}\"", ph));
                    } else {
                        let h = pic.height_request();
                        if h > 0 { html.push_str(&format!(" height=\"{}\"", h)); }
                    }
                    html.push('>');
                }
                return;
            }
            // Check for Separator (hr)
            if let Some(sep) = widget.downcast_ref::<gtk::Separator>() {
                let name = sep.widget_name().to_string();
                if name == "hr_rule" {
                    // Default: 100% centered
                    html.push_str("<hr width=\"100%\" align=\"center\">\n");
                } else if let Some(params) = name.strip_prefix("hr_rule:") {
                    // Reconstruct attributes from widget_name
                    html.push_str("<hr");
                    let mut has_width = false;
                    let mut has_align = false;
                    for part in params.split(';') {
                        if let Some((k, v)) = part.split_once('=') {
                            html.push_str(&format!(" {}=\"{}\"", k, v));
                            if k == "width" { has_width = true; }
                            if k == "align" { has_align = true; }
                        }
                    }
                    if !has_width { html.push_str(" width=\"100%\""); }
                    if !has_align { html.push_str(" align=\"center\""); }
                    html.push_str(">\n");
                } else {
                    html.push_str("<hr width=\"100%\" align=\"center\">\n");
                }
                return;
            }
        }
    }
}

pub fn serialize_grid_as_table(grid: &gtk::Grid, html: &mut String, css_rules_store: &HashMap<String, String>) {
    // Grid widget_name stores raw HTML attributes verbatim
    let grid_name = grid.widget_name().to_string();
    if grid_name.is_empty() {
        html.push_str("<table>\n");
    } else {
        html.push_str(&format!("<table {}>\n", grid_name));
    }

    let mut serialized_cells: std::collections::HashSet<*const std::ffi::c_void> = std::collections::HashSet::new();

    for row in 0..100 {
        let mut row_has_items = false;

        for col in 0..100 {
            if let Some(w) = grid.child_at(col, row) {
                let ptr = w.as_ptr() as *const std::ffi::c_void;
                if serialized_cells.contains(&ptr) {
                    continue;
                }
                serialized_cells.insert(ptr);

                if !row_has_items {
                    html.push_str("  <tr>\n");
                    row_has_items = true;
                }

                if let Some(cell_view) = w.downcast_ref::<gtk::TextView>() {
                    let name = w.widget_name().to_string();

                    // New format: "tag|raw_attrs" (e.g. "td|colspan=\"2\" bgcolor=\"red\"")
                    // Legacy format: "colspan,rowspan,type"
                    let (cell_tag, cell_attrs) = if let Some((tag, attrs)) = name.split_once('|') {
                        (tag, attrs.to_string())
                    } else if name == "th" || name == "td" {
                        (name.as_str(), String::new())
                    } else if name.contains(',') {
                        // Legacy: colspan,rowspan,type
                        let parts: Vec<&str> = name.split(',').collect();
                        let tag = if parts.len() >= 3 && parts[2] == "th" { "th" } else { "td" };
                        let mut attrs = String::new();
                        if parts.len() >= 2 {
                            if parts[0] != "1" {
                                attrs.push_str(&format!("colspan=\"{}\"", parts[0]));
                            }
                            if parts[1] != "1" {
                                if !attrs.is_empty() { attrs.push(' '); }
                                attrs.push_str(&format!("rowspan=\"{}\"", parts[1]));
                            }
                        }
                        (tag, attrs)
                    } else {
                        ("td", String::new())
                    };

                    if cell_attrs.is_empty() {
                        html.push_str(&format!("    <{}>", cell_tag));
                    } else {
                        html.push_str(&format!("    <{} {}>", cell_tag, cell_attrs));
                    }
                    let cell_html = serialize_buffer(&cell_view.buffer(), css_rules_store);
                    html.push_str(cell_html.trim());
                    html.push_str(&format!("</{}>\n", cell_tag));
                }
            }
        }
        if row_has_items {
            html.push_str("  </tr>\n");
        } else if row > 0 {
            break;
        }
    }
    html.push_str("</table>\n");
}

/// Parse a "prefix:tag|attrs" widget_name into (tag, attrs).
fn parse_layout_widget_name(name: &str, prefix: &str) -> (String, String) {
    let rest = name.strip_prefix(prefix).unwrap_or(name);
    if let Some((tag, attrs)) = rest.split_once('|') {
        (tag.to_string(), attrs.replace("&quot;", "\""))
    } else {
        (rest.to_string(), String::new())
    }
}

fn serialize_flex(gbox: &gtk::Box, html: &mut String, css_rules_store: &HashMap<String, String>) {
    let name = gbox.widget_name().to_string();
    let (tag, attrs) = parse_layout_widget_name(&name, "flex:");

    if attrs.is_empty() {
        html.push_str(&format!("<{}>\n", tag));
    } else {
        html.push_str(&format!("<{} {}>\n", tag, attrs));
    }

    // Iterate over children (gtk::Box children)
    let mut child_opt = gbox.first_child();
    while let Some(child_widget) = child_opt {
        if let Some(child_view) = child_widget.downcast_ref::<gtk::TextView>() {
            let child_name = child_widget.widget_name().to_string();
            let (child_tag, child_attrs) = parse_layout_widget_name(&child_name, "flexchild:");

            if child_attrs.is_empty() {
                html.push_str(&format!("  <{}>", child_tag));
            } else {
                html.push_str(&format!("  <{} {}>", child_tag, child_attrs));
            }
            let child_html = serialize_buffer(&child_view.buffer(), css_rules_store);
            html.push_str(child_html.trim());
            html.push_str(&format!("</{}>\n", child_tag));
        }
        child_opt = child_widget.next_sibling();
    }

    html.push_str(&format!("</{}>\n", tag));
}

fn serialize_flex_flowbox(fb: &gtk::FlowBox, html: &mut String, css_rules_store: &HashMap<String, String>) {
    let name = fb.widget_name().to_string();
    let (tag, attrs) = parse_layout_widget_name(&name, "flex:");

    if attrs.is_empty() {
        html.push_str(&format!("<{}>\n", tag));
    } else {
        html.push_str(&format!("<{} {}>\n", tag, attrs));
    }

    // FlowBox children are wrapped in FlowBoxChild — unwrap to get the TextView
    let mut child_opt = fb.first_child();
    while let Some(fb_child_widget) = child_opt {
        // FlowBoxChild contains our TextView as its child
        if let Some(fb_child) = fb_child_widget.downcast_ref::<gtk::FlowBoxChild>() {
            if let Some(inner) = fb_child.child() {
                if let Some(child_view) = inner.downcast_ref::<gtk::TextView>() {
                    let child_name = inner.widget_name().to_string();
                    let (child_tag, child_attrs) = parse_layout_widget_name(&child_name, "flexchild:");

                    if child_attrs.is_empty() {
                        html.push_str(&format!("  <{}>", child_tag));
                    } else {
                        html.push_str(&format!("  <{} {}>", child_tag, child_attrs));
                    }
                    let child_html = serialize_buffer(&child_view.buffer(), css_rules_store);
                    html.push_str(child_html.trim());
                    html.push_str(&format!("</{}>\n", child_tag));
                }
            }
        }
        child_opt = fb_child_widget.next_sibling();
    }

    html.push_str(&format!("</{}>\n", tag));
}

fn serialize_css_grid(grid: &gtk::Grid, html: &mut String, css_rules_store: &HashMap<String, String>) {
    let name = grid.widget_name().to_string();
    let (tag, attrs) = parse_layout_widget_name(&name, "cssgrid:");

    if attrs.is_empty() {
        html.push_str(&format!("<{}>\n", tag));
    } else {
        html.push_str(&format!("<{} {}>\n", tag, attrs));
    }

    let mut serialized_cells: std::collections::HashSet<*const std::ffi::c_void> = std::collections::HashSet::new();

    for row in 0..100 {
        let mut found_any = false;
        for col in 0..100 {
            if let Some(w) = grid.child_at(col, row) {
                let ptr = w.as_ptr() as *const std::ffi::c_void;
                if serialized_cells.contains(&ptr) {
                    continue;
                }
                serialized_cells.insert(ptr);
                found_any = true;

                if let Some(child_view) = w.downcast_ref::<gtk::TextView>() {
                    let child_name = w.widget_name().to_string();
                    let (child_tag, child_attrs) = parse_layout_widget_name(&child_name, "gridchild:");

                    if child_attrs.is_empty() {
                        html.push_str(&format!("  <{}>", child_tag));
                    } else {
                        html.push_str(&format!("  <{} {}>", child_tag, child_attrs));
                    }
                    let child_html = serialize_buffer(&child_view.buffer(), css_rules_store);
                    html.push_str(child_html.trim());
                    html.push_str(&format!("</{}>\n", child_tag));
                }
            }
        }
        if !found_any && row > 0 {
            break;
        }
    }

    html.push_str(&format!("</{}>\n", tag));
}
