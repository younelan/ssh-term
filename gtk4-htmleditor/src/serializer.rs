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

    while iter < end {
        let tags = iter.tags();
        let mut is_marker = false;
        let mut current_pos_tags: Vec<String> = Vec::new();

        let mut comment_text: Option<String> = None;
        for tag in tags.iter() {
            if let Some(name) = tag.name() {
                let name_str = name.as_str().to_string();
                if name_str == "list_marker" {
                    is_marker = true;
                } else if let Some(text) = name_str.strip_prefix("comment:") {
                    comment_text = Some(text.to_string());
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

            // Open new tags
            for tag in current_pos_tags.iter() {
                if !active_tags.contains(tag) {
                    html.push_str(&open_tag_markup(tag, css_rules_store));
                }
            }

            active_tags = current_pos_tags.clone();
        }

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
        || name.starts_with("ul_")
        || name.starts_with("ol_")
        || name.starts_with("css_")
        || name.starts_with("link:")
        || name.starts_with("blockquote_")
        || name.starts_with("indent_")
        || name.starts_with("comment:")
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

fn close_tag_name(tag: &str) -> String {
    if tag.starts_with("ul_") {
        "ul".to_string()
    } else if tag.starts_with("ol_") {
        "ol".to_string()
    } else if tag.starts_with("color: ") {
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
            // Check for Box (flex container, no-wrap)
            if let Some(gbox) = widget.downcast_ref::<gtk::Box>() {
                let name = gbox.widget_name().to_string();
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
            // Check for Picture (img)
            if let Some(pic) = widget.downcast_ref::<gtk::Picture>() {
                let name = pic.widget_name().to_string();
                if let Some(rest) = name.strip_prefix("img:") {
                    // Format: img:src or img:src|alt:text
                    if let Some(pipe_pos) = rest.find("|alt:") {
                        let src = &rest[..pipe_pos];
                        let alt = &rest[pipe_pos + 5..];
                        html.push_str(&format!("<img src=\"{}\" alt=\"{}\">", src, alt));
                    } else {
                        html.push_str(&format!("<img src=\"{}\">", rest));
                    }
                }
                return;
            }
            // Check for Separator (hr)
            if widget.downcast_ref::<gtk::Separator>().is_some() {
                html.push_str("<hr>\n");
                return;
            }
        }
    }
}

fn serialize_grid_as_table(grid: &gtk::Grid, html: &mut String, css_rules_store: &HashMap<String, String>) {
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
