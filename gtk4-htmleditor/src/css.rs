use gtk4 as gtk;
use gtk::prelude::*;
use std::collections::HashMap;

// ── Enums ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextTransform {
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerticalAlign {
    Baseline,
    Super,
    Sub,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WhiteSpaceMode {
    Normal,
    Pre,
    PreWrap,
    Nowrap,
    PreLine,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ListStyleType {
    Disc,
    Square,
    Circle,
    None,
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorderStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDirection {
    Ltr,
    Rtl,
}

// ── CssProperties ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct CssProperties {
    // Character-level
    pub color: Option<String>,
    pub background_color: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f64>,
    pub font_weight: Option<i32>,
    pub font_style: Option<FontStyle>,
    pub text_decoration_underline: Option<bool>,
    pub text_decoration_line_through: Option<bool>,
    pub text_transform: Option<TextTransform>,
    pub letter_spacing: Option<i32>,
    pub word_spacing: Option<i32>,
    pub vertical_align: Option<VerticalAlign>,
    pub font_variant_small_caps: Option<bool>,

    // Paragraph/block-level
    pub text_align: Option<gtk::Justification>,
    pub margin_left: Option<i32>,
    pub margin_right: Option<i32>,
    pub margin_top: Option<i32>,
    pub margin_bottom: Option<i32>,
    pub padding_left: Option<i32>,
    pub padding_right: Option<i32>,
    pub padding_top: Option<i32>,
    pub padding_bottom: Option<i32>,
    pub text_indent: Option<i32>,
    pub line_height: Option<f64>,
    pub direction: Option<TextDirection>,
    pub white_space: Option<WhiteSpaceMode>,
    pub paragraph_background: Option<String>,

    // Block control
    pub display_none: bool,

    // List
    pub list_style_type: Option<ListStyleType>,

    // Border per side: [top, right, bottom, left]
    pub border_top_width: Option<i32>,
    pub border_right_width: Option<i32>,
    pub border_bottom_width: Option<i32>,
    pub border_left_width: Option<i32>,
    pub border_color: Option<String>,
    pub border_style: Option<BorderStyle>,

    // Dimensions (stored as original strings for round-trip, e.g. "50%", "200px")
    pub width: Option<String>,
    pub max_width: Option<String>,
    pub min_width: Option<String>,

    // Block control: visibility
    pub visibility_hidden: bool,
    pub opacity: Option<f64>,

    // Height (stored as original string for round-trip, e.g. "100px", "auto")
    pub height: Option<String>,
    pub min_height: Option<String>,
    pub max_height: Option<String>,

    // Background image (round-trip only — GTK TextTag cannot render)
    pub background_image: Option<String>,

    // Round-trip only (not rendered in TextBuffer, preserved for serialization)
    pub float: Option<String>,
    pub clear: Option<String>,
    pub position: Option<String>,
    pub border_radius: Option<String>,
    pub overflow: Option<String>,
    pub top: Option<String>,
    pub right_pos: Option<String>,
    pub bottom_pos: Option<String>,
    pub left_pos: Option<String>,
    pub z_index: Option<String>,
    pub display: Option<String>,

    // Flex/Grid layout (rendered via embedded widgets)
    pub flex_direction: Option<String>,
    pub flex_wrap: Option<String>,
    pub justify_content: Option<String>,
    pub align_items: Option<String>,
    pub gap: Option<i32>,
    pub grid_template_columns: Option<String>,
    pub grid_column: Option<String>,
    pub grid_row: Option<String>,
}

impl CssProperties {
    /// Merge `other` into `self`. Non-None fields from `other` override `self`.
    pub fn merge(&mut self, other: &CssProperties) {
        macro_rules! merge_field {
            ($field:ident) => {
                if other.$field.is_some() {
                    self.$field = other.$field.clone();
                }
            };
        }
        merge_field!(color);
        merge_field!(background_color);
        merge_field!(font_family);
        merge_field!(font_size);
        merge_field!(font_weight);
        merge_field!(font_style);
        merge_field!(text_decoration_underline);
        merge_field!(text_decoration_line_through);
        merge_field!(text_transform);
        merge_field!(letter_spacing);
        merge_field!(word_spacing);
        merge_field!(vertical_align);
        merge_field!(font_variant_small_caps);
        merge_field!(text_align);
        merge_field!(margin_left);
        merge_field!(margin_right);
        merge_field!(margin_top);
        merge_field!(margin_bottom);
        merge_field!(padding_left);
        merge_field!(padding_right);
        merge_field!(padding_top);
        merge_field!(padding_bottom);
        merge_field!(text_indent);
        merge_field!(line_height);
        merge_field!(direction);
        merge_field!(white_space);
        merge_field!(paragraph_background);
        merge_field!(list_style_type);
        merge_field!(border_top_width);
        merge_field!(border_right_width);
        merge_field!(border_bottom_width);
        merge_field!(border_left_width);
        merge_field!(border_color);
        merge_field!(border_style);
        merge_field!(width);
        merge_field!(max_width);
        merge_field!(min_width);
        merge_field!(height);
        merge_field!(min_height);
        merge_field!(max_height);
        merge_field!(background_image);
        merge_field!(opacity);
        merge_field!(float);
        merge_field!(clear);
        merge_field!(position);
        merge_field!(border_radius);
        merge_field!(overflow);
        merge_field!(top);
        merge_field!(right_pos);
        merge_field!(bottom_pos);
        merge_field!(left_pos);
        merge_field!(z_index);
        merge_field!(display);
        merge_field!(flex_direction);
        merge_field!(flex_wrap);
        merge_field!(justify_content);
        merge_field!(align_items);
        merge_field!(gap);
        merge_field!(grid_template_columns);
        merge_field!(grid_column);
        merge_field!(grid_row);
        if other.display_none {
            self.display_none = true;
        }
        if other.visibility_hidden {
            self.visibility_hidden = true;
        }
    }

    /// Effective left margin (margin + padding).
    pub fn effective_left_margin(&self) -> Option<i32> {
        match (self.margin_left, self.padding_left) {
            (Some(m), Some(p)) => Some(m + p),
            (Some(m), None) => Some(m),
            (None, Some(p)) => Some(p),
            (None, None) => None,
        }
    }

    /// Effective right margin (margin + padding).
    pub fn effective_right_margin(&self) -> Option<i32> {
        match (self.margin_right, self.padding_right) {
            (Some(m), Some(p)) => Some(m + p),
            (Some(m), None) => Some(m),
            (None, Some(p)) => Some(p),
            (None, None) => None,
        }
    }

    /// Whether this has any paragraph-level property set.
    pub fn has_paragraph_properties(&self) -> bool {
        self.text_align.is_some()
            || self.margin_left.is_some()
            || self.margin_right.is_some()
            || self.margin_top.is_some()
            || self.margin_bottom.is_some()
            || self.padding_left.is_some()
            || self.padding_right.is_some()
            || self.padding_top.is_some()
            || self.padding_bottom.is_some()
            || self.text_indent.is_some()
            || self.paragraph_background.is_some()
            || self.direction.is_some()
    }

    /// Whether this has border properties set.
    pub fn has_border(&self) -> bool {
        let has_width = self.border_top_width.unwrap_or(0) > 0
            || self.border_right_width.unwrap_or(0) > 0
            || self.border_bottom_width.unwrap_or(0) > 0
            || self.border_left_width.unwrap_or(0) > 0;
        has_width && self.border_style != Some(BorderStyle::None)
    }

    /// Serialize back to CSS declaration string for round-trip.
    pub fn to_css_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref v) = self.color { parts.push(format!("color: {}", v)); }
        if let Some(ref v) = self.background_color { parts.push(format!("background-color: {}", v)); }
        if let Some(ref v) = self.font_family { parts.push(format!("font-family: {}", v)); }
        if let Some(v) = self.font_size { parts.push(format!("font-size: {}pt", v)); }
        if let Some(v) = self.font_weight {
            if v >= 700 { parts.push("font-weight: bold".to_string()); }
            else if v != 400 { parts.push(format!("font-weight: {}", v)); }
        }
        if let Some(FontStyle::Italic) = self.font_style { parts.push("font-style: italic".to_string()); }
        if let Some(FontStyle::Oblique) = self.font_style { parts.push("font-style: oblique".to_string()); }
        if let Some(true) = self.text_decoration_underline { parts.push("text-decoration: underline".to_string()); }
        if let Some(true) = self.text_decoration_line_through { parts.push("text-decoration: line-through".to_string()); }
        if let Some(ref align) = self.text_align {
            let s = match align {
                gtk::Justification::Center => "center",
                gtk::Justification::Right => "right",
                gtk::Justification::Fill => "justify",
                _ => "left",
            };
            parts.push(format!("text-align: {}", s));
        }
        if let Some(v) = self.margin_top { parts.push(format!("margin-top: {}px", v)); }
        if let Some(v) = self.margin_bottom { parts.push(format!("margin-bottom: {}px", v)); }
        if let Some(v) = self.margin_left { parts.push(format!("margin-left: {}px", v)); }
        if let Some(v) = self.margin_right { parts.push(format!("margin-right: {}px", v)); }
        if let Some(v) = self.padding_top { parts.push(format!("padding-top: {}px", v)); }
        if let Some(v) = self.padding_bottom { parts.push(format!("padding-bottom: {}px", v)); }
        if let Some(v) = self.padding_left { parts.push(format!("padding-left: {}px", v)); }
        if let Some(v) = self.padding_right { parts.push(format!("padding-right: {}px", v)); }
        if let Some(v) = self.text_indent { parts.push(format!("text-indent: {}px", v)); }
        if let Some(v) = self.line_height { if v > 0.0 { parts.push(format!("line-height: {}px", v as i32)); } }
        if let Some(ref v) = self.paragraph_background { parts.push(format!("background-color: {}", v)); }
        if let Some(TextDirection::Rtl) = self.direction { parts.push("direction: rtl".to_string()); }
        if let Some(v) = self.letter_spacing { parts.push(format!("letter-spacing: {}px", v / 1024)); }
        if let Some(ref tt) = self.text_transform {
            let s = match tt {
                TextTransform::Uppercase => "uppercase",
                TextTransform::Lowercase => "lowercase",
                TextTransform::Capitalize => "capitalize",
                TextTransform::None => "none",
            };
            parts.push(format!("text-transform: {}", s));
        }
        if let Some(true) = self.font_variant_small_caps { parts.push("font-variant: small-caps".to_string()); }
        if self.has_border() {
            let w = self.border_top_width.unwrap_or(1);
            let s = match self.border_style {
                Some(BorderStyle::Dashed) => "dashed",
                Some(BorderStyle::Dotted) => "dotted",
                Some(BorderStyle::Double) => "double",
                _ => "solid",
            };
            let c = self.border_color.as_deref().unwrap_or("black");
            parts.push(format!("border: {}px {} {}", w, s, c));
        }
        if let Some(ref v) = self.width { parts.push(format!("width: {}", v)); }
        if let Some(ref v) = self.max_width { parts.push(format!("max-width: {}", v)); }
        if let Some(ref v) = self.min_width { parts.push(format!("min-width: {}", v)); }
        if let Some(ref v) = self.height { parts.push(format!("height: {}", v)); }
        if let Some(ref v) = self.min_height { parts.push(format!("min-height: {}", v)); }
        if let Some(ref v) = self.max_height { parts.push(format!("max-height: {}", v)); }
        if let Some(ref v) = self.background_image { parts.push(format!("background-image: {}", v)); }
        if self.visibility_hidden { parts.push("visibility: hidden".to_string()); }
        if let Some(v) = self.opacity { if v < 1.0 { parts.push(format!("opacity: {}", v)); } }
        // Round-trip properties (not rendered but preserved)
        if let Some(ref v) = self.float { parts.push(format!("float: {}", v)); }
        if let Some(ref v) = self.clear { parts.push(format!("clear: {}", v)); }
        if let Some(ref v) = self.position { parts.push(format!("position: {}", v)); }
        if let Some(ref v) = self.border_radius { parts.push(format!("border-radius: {}", v)); }
        if let Some(ref v) = self.overflow { parts.push(format!("overflow: {}", v)); }
        if let Some(ref v) = self.top { parts.push(format!("top: {}", v)); }
        if let Some(ref v) = self.right_pos { parts.push(format!("right: {}", v)); }
        if let Some(ref v) = self.bottom_pos { parts.push(format!("bottom: {}", v)); }
        if let Some(ref v) = self.left_pos { parts.push(format!("left: {}", v)); }
        if let Some(ref v) = self.z_index { parts.push(format!("z-index: {}", v)); }
        if let Some(ref v) = self.display { parts.push(format!("display: {}", v)); }
        if let Some(ref v) = self.flex_direction { parts.push(format!("flex-direction: {}", v)); }
        if let Some(ref v) = self.flex_wrap { parts.push(format!("flex-wrap: {}", v)); }
        if let Some(ref v) = self.justify_content { parts.push(format!("justify-content: {}", v)); }
        if let Some(ref v) = self.align_items { parts.push(format!("align-items: {}", v)); }
        if let Some(v) = self.gap { parts.push(format!("gap: {}px", v)); }
        if let Some(ref v) = self.grid_template_columns { parts.push(format!("grid-template-columns: {}", v)); }
        if let Some(ref v) = self.grid_column { parts.push(format!("grid-column: {}", v)); }
        if let Some(ref v) = self.grid_row { parts.push(format!("grid-row: {}", v)); }
        parts.join("; ")
    }
}

// ── CSS Value Parsing ──────────────────────────────────────────────────────

/// Parse a CSS numeric value with units. Returns pixels (approximate).
/// Supports: px, pt, em, %, bare numbers.
pub fn parse_css_length(val: &str) -> Option<f64> {
    let val = val.trim();
    if val.is_empty() || val == "auto" || val == "inherit" || val == "initial" {
        return None;
    }
    // Extract numeric prefix
    let mut num_end = 0;
    for (i, ch) in val.char_indices() {
        if ch.is_ascii_digit() || ch == '.' || ch == '-' || (ch == '+' && i == 0) {
            num_end = i + ch.len_utf8();
        } else if num_end > 0 {
            break;
        }
    }
    if num_end == 0 {
        return None;
    }
    let num: f64 = val[..num_end].parse().ok()?;
    let unit = val[num_end..].trim().to_lowercase();
    match unit.as_str() {
        "px" | "" => Some(num),
        "pt" => Some(num * 1.333), // 1pt ≈ 1.333px at 96dpi
        "em" | "rem" => Some(num * 16.0), // assume 1em = 16px base
        "%" => Some(num * 0.16), // rough: 100% ≈ 16px base
        _ => Some(num), // fallback: treat as px
    }
}

/// Parse a CSS length and return as integer pixels.
pub fn parse_px(val: &str) -> Option<i32> {
    parse_css_length(val).map(|v| v.round() as i32)
}

// ── Shorthand Expansion ────────────────────────────────────────────────────

/// Expand a 1-4 value margin/padding shorthand into [top, right, bottom, left].
pub fn expand_box_shorthand(val: &str) -> [Option<i32>; 4] {
    let parts: Vec<&str> = val.split_whitespace().collect();
    let parsed: Vec<Option<i32>> = parts.iter().map(|p| parse_px(p)).collect();
    match parsed.len() {
        1 => [parsed[0], parsed[0], parsed[0], parsed[0]],
        2 => [parsed[0], parsed[1], parsed[0], parsed[1]],
        3 => [parsed[0], parsed[1], parsed[2], parsed[1]],
        4 => [parsed[0], parsed[1], parsed[2], parsed[3]],
        _ => [None, None, None, None],
    }
}

/// Parse a border shorthand value like "1px solid #000".
/// Returns (width, style, color).
pub fn parse_border_shorthand(val: &str) -> (Option<i32>, Option<BorderStyle>, Option<String>) {
    let mut width = None;
    let mut style = None;
    let mut color = None;
    for token in val.split_whitespace() {
        let lower = token.to_lowercase();
        match lower.as_str() {
            "none" | "hidden" => { style = Some(BorderStyle::None); width = Some(0); }
            "solid" => style = Some(BorderStyle::Solid),
            "dashed" => style = Some(BorderStyle::Dashed),
            "dotted" => style = Some(BorderStyle::Dotted),
            "double" => style = Some(BorderStyle::Double),
            "groove" | "ridge" | "inset" | "outset" => style = Some(BorderStyle::Solid), // approximate
            "thin" => width = Some(1),
            "medium" => width = Some(3),
            "thick" => width = Some(5),
            _ => {
                if let Some(px) = parse_px(token) {
                    width = Some(px);
                } else {
                    // Assume it's a color
                    color = Some(token.to_string());
                }
            }
        }
    }
    (width, style, color)
}

/// Parse CSS `font` shorthand.
/// Format: [style] [variant] [weight] size[/line-height] family
pub fn parse_font_shorthand(val: &str, props: &mut CssProperties) {
    let tokens: Vec<&str> = val.split_whitespace().collect();
    let mut found_size = false;
    let mut family_parts = Vec::new();

    for token in &tokens {
        let lower = token.to_lowercase();
        if found_size {
            family_parts.push(*token);
            continue;
        }
        match lower.as_str() {
            "italic" => props.font_style = Some(FontStyle::Italic),
            "oblique" => props.font_style = Some(FontStyle::Oblique),
            "small-caps" => props.font_variant_small_caps = Some(true),
            "bold" => props.font_weight = Some(700),
            "bolder" => props.font_weight = Some(700),
            "lighter" => props.font_weight = Some(300),
            "normal" => {} // skip
            _ => {
                // Check if it's a numeric weight
                if let Ok(w) = lower.parse::<i32>()
                    && (100..=900).contains(&w) {
                        props.font_weight = Some(w);
                        continue;
                    }
                // Try parsing as size (possibly with /line-height)
                let (size_part, lh_part) = if let Some(slash) = lower.find('/') {
                    (&lower[..slash], Some(&lower[slash + 1..]))
                } else {
                    (lower.as_str(), None)
                };
                if let Some(sz) = parse_css_length(size_part) {
                    props.font_size = Some(sz / 1.333); // px to pt approximation
                    if let Some(lh) = lh_part
                        && let Some(lh_val) = parse_css_length(lh) {
                            props.line_height = Some(lh_val);
                        }
                    found_size = true;
                }
            }
        }
    }
    if !family_parts.is_empty() {
        let family = family_parts.join(" ");
        // Strip quotes, take first family before comma
        let family = family.trim_matches(|c| c == '\'' || c == '"');
        let family = family.split(',').next().unwrap_or(family).trim();
        let family = family.trim_matches(|c| c == '\'' || c == '"');
        if !family.is_empty() {
            props.font_family = Some(family.to_string());
        }
    }
}

// ── Declaration Parsing ────────────────────────────────────────────────────

/// Parse a CSS declaration block (semicolon-separated property:value pairs).
pub fn parse_declarations(decls: &str) -> CssProperties {
    let mut props = CssProperties::default();
    for rule in decls.split(';') {
        let rule = rule.trim();
        if rule.is_empty() { continue; }
        let colon = match rule.find(':') {
            Some(i) => i,
            None => continue,
        };
        let key = rule[..colon].trim().to_lowercase();
        let val = rule[colon + 1..].trim();
        if val.is_empty() { continue; }

        match key.as_str() {
            // ── Character-level ──
            "color" => props.color = Some(val.to_string()),
            "background-color" => {
                props.background_color = Some(val.to_string());
                props.paragraph_background = Some(val.to_string());
            }
            "background" => {
                // Extract url() for round-trip preservation
                if let Some(url_start) = val.find("url(") {
                    let rest = &val[url_start..];
                    if let Some(close) = rest.find(')') {
                        props.background_image = Some(rest[..=close].to_string());
                    }
                }
                // Extract color (first token that isn't url() or gradient)
                for token in val.split_whitespace() {
                    if !token.starts_with("url(") && !token.contains("gradient")
                        && !token.starts_with("no-repeat") && !token.starts_with("repeat")
                        && !token.starts_with("center") && !token.starts_with("top")
                        && !token.starts_with("bottom") && !token.starts_with("left")
                        && !token.starts_with("right") && !token.starts_with("cover")
                        && !token.starts_with("contain")
                    {
                        // Likely a color value
                        if token.starts_with('#') || token.starts_with("rgb")
                            || token.starts_with("hsl") || token.chars().next().map_or(false, |c| c.is_alphabetic())
                        {
                            props.background_color = Some(token.to_string());
                            props.paragraph_background = Some(token.to_string());
                            break;
                        }
                    }
                }
            }
            "font-family" => {
                let fam = val.trim_matches(|c| c == '\'' || c == '"');
                let fam = fam.split(',').next().unwrap_or(fam).trim();
                let fam = fam.trim_matches(|c| c == '\'' || c == '"');
                props.font_family = Some(fam.to_string());
            }
            "font-size" => {
                if let Some(sz) = parse_css_length(val) {
                    // Convert px to pt (approximate)
                    props.font_size = Some(sz / 1.333);
                }
            }
            "font-weight" => {
                let lower = val.to_lowercase();
                match lower.as_str() {
                    "bold" | "bolder" => props.font_weight = Some(700),
                    "normal" => props.font_weight = Some(400),
                    "lighter" => props.font_weight = Some(300),
                    _ => {
                        if let Ok(w) = lower.parse::<i32>() {
                            props.font_weight = Some(w);
                        }
                    }
                }
            }
            "font-style" => {
                match val.to_lowercase().as_str() {
                    "italic" => props.font_style = Some(FontStyle::Italic),
                    "oblique" => props.font_style = Some(FontStyle::Oblique),
                    "normal" => props.font_style = Some(FontStyle::Normal),
                    _ => {}
                }
            }
            "font-variant" => {
                if val.to_lowercase().contains("small-caps") {
                    props.font_variant_small_caps = Some(true);
                }
            }
            "font" => {
                parse_font_shorthand(val, &mut props);
            }
            "text-decoration" | "text-decoration-line" => {
                let lower = val.to_lowercase();
                if lower == "none" {
                    props.text_decoration_underline = Some(false);
                    props.text_decoration_line_through = Some(false);
                } else {
                    if lower.contains("underline") {
                        props.text_decoration_underline = Some(true);
                    }
                    if lower.contains("line-through") {
                        props.text_decoration_line_through = Some(true);
                    }
                }
            }
            "text-transform" => {
                match val.to_lowercase().as_str() {
                    "uppercase" => props.text_transform = Some(TextTransform::Uppercase),
                    "lowercase" => props.text_transform = Some(TextTransform::Lowercase),
                    "capitalize" => props.text_transform = Some(TextTransform::Capitalize),
                    _ => props.text_transform = Some(TextTransform::None),
                }
            }
            "letter-spacing" => {
                if let Some(px) = parse_px(val) {
                    // Pango letter-spacing is in 1/1024 of a point
                    props.letter_spacing = Some(px.saturating_mul(1024));
                }
            }
            "word-spacing" => {
                props.word_spacing = parse_px(val);
            }
            "vertical-align" => {
                match val.to_lowercase().as_str() {
                    "super" | "text-top" => props.vertical_align = Some(VerticalAlign::Super),
                    "sub" | "text-bottom" => props.vertical_align = Some(VerticalAlign::Sub),
                    _ => props.vertical_align = Some(VerticalAlign::Baseline),
                }
            }

            // ── Paragraph/block-level ──
            "text-align" => {
                // Support both LTR and RTL alignment keywords
                match val.to_lowercase().as_str() {
                    "center" => props.text_align = Some(gtk::Justification::Center),
                    "right" => props.text_align = Some(gtk::Justification::Right),
                    "left" => props.text_align = Some(gtk::Justification::Left),
                    "justify" | "fill" => props.text_align = Some(gtk::Justification::Fill),
                    // CSS logical properties for BiDi
                    "start" => props.text_align = Some(gtk::Justification::Left),
                    "end" => props.text_align = Some(gtk::Justification::Right),
                    _ => {}
                }
            }
            "text-indent" => { props.text_indent = parse_px(val); }
            "direction" => {
                match val.to_lowercase().as_str() {
                    "rtl" => props.direction = Some(TextDirection::Rtl),
                    "ltr" => props.direction = Some(TextDirection::Ltr),
                    _ => {}
                }
            }
            "unicode-bidi" => {
                // Acknowledge but GTK handles BiDi via Pango automatically
            }
            "line-height" => {
                let lower = val.to_lowercase();
                if lower == "normal" {
                    props.line_height = Some(0.0);
                } else if lower.ends_with('%') {
                    if let Ok(pct) = lower.trim_end_matches('%').parse::<f64>() {
                        props.line_height = Some(pct / 100.0 * 16.0); // approx
                    }
                } else if lower.ends_with("em") {
                    if let Ok(em) = lower.trim_end_matches("em").parse::<f64>() {
                        props.line_height = Some(em * 16.0); // approx
                    }
                } else if let Some(px) = parse_css_length(val) {
                    props.line_height = Some(px);
                } else if let Ok(mult) = lower.parse::<f64>() {
                    // Unitless multiplier
                    props.line_height = Some(mult * 16.0);
                }
            }
            "white-space" => {
                match val.to_lowercase().as_str() {
                    "pre" => props.white_space = Some(WhiteSpaceMode::Pre),
                    "pre-wrap" => props.white_space = Some(WhiteSpaceMode::PreWrap),
                    "nowrap" => props.white_space = Some(WhiteSpaceMode::Nowrap),
                    "pre-line" => props.white_space = Some(WhiteSpaceMode::PreLine),
                    _ => props.white_space = Some(WhiteSpaceMode::Normal),
                }
            }

            // ── Margins ──
            "margin" | "padding" => {
                let [top, right, bottom, left] = expand_box_shorthand(val);
                if key == "margin" {
                    if top.is_some() { props.margin_top = top; }
                    if right.is_some() { props.margin_right = right; }
                    if bottom.is_some() { props.margin_bottom = bottom; }
                    if left.is_some() { props.margin_left = left; }
                } else {
                    if top.is_some() { props.padding_top = top; }
                    if right.is_some() { props.padding_right = right; }
                    if bottom.is_some() { props.padding_bottom = bottom; }
                    if left.is_some() { props.padding_left = left; }
                }
            }
            "margin-left" | "margin-inline-start" => { props.margin_left = parse_px(val); }
            "margin-right" | "margin-inline-end" => { props.margin_right = parse_px(val); }
            "margin-top" | "margin-block-start" => { props.margin_top = parse_px(val); }
            "margin-bottom" | "margin-block-end" => { props.margin_bottom = parse_px(val); }
            "padding-left" | "padding-inline-start" => { props.padding_left = parse_px(val); }
            "padding-right" | "padding-inline-end" => { props.padding_right = parse_px(val); }
            "padding-top" | "padding-block-start" => { props.padding_top = parse_px(val); }
            "padding-bottom" | "padding-block-end" => { props.padding_bottom = parse_px(val); }

            // ── Display ──
            "display" => {
                let v = val.to_lowercase();
                if v == "none" {
                    props.display_none = true;
                } else {
                    props.display = Some(v);
                }
            }

            // ── Flex / Grid layout ──
            "flex-direction" => { props.flex_direction = Some(val.to_lowercase()); }
            "flex-wrap" => { props.flex_wrap = Some(val.to_lowercase()); }
            "justify-content" => { props.justify_content = Some(val.to_lowercase()); }
            "align-items" => { props.align_items = Some(val.to_lowercase()); }
            "gap" | "grid-gap" => { props.gap = parse_px(val); }
            "grid-template-columns" => { props.grid_template_columns = Some(val.to_string()); }
            "grid-column" => { props.grid_column = Some(val.to_string()); }
            "grid-row" => { props.grid_row = Some(val.to_string()); }

            // ── List ──
            "list-style-type" => {
                props.list_style_type = parse_list_style_type(val);
            }
            "list-style" => {
                // Shorthand: take first keyword
                let first = val.split_whitespace().next().unwrap_or(val);
                props.list_style_type = parse_list_style_type(first);
            }

            // ── Border ──
            "border" => {
                let (w, s, c) = parse_border_shorthand(val);
                if let Some(w) = w {
                    props.border_top_width = Some(w);
                    props.border_right_width = Some(w);
                    props.border_bottom_width = Some(w);
                    props.border_left_width = Some(w);
                }
                if let Some(s) = s { props.border_style = Some(s); }
                if let Some(c) = c { props.border_color = Some(c); }
            }
            "border-top" => {
                let (w, s, c) = parse_border_shorthand(val);
                if let Some(w) = w { props.border_top_width = Some(w); }
                if let Some(s) = s { props.border_style = Some(s); }
                if let Some(c) = c { props.border_color = Some(c); }
            }
            "border-bottom" => {
                let (w, s, c) = parse_border_shorthand(val);
                if let Some(w) = w { props.border_bottom_width = Some(w); }
                if let Some(s) = s { props.border_style = Some(s); }
                if let Some(c) = c { props.border_color = Some(c); }
            }
            "border-left" | "border-inline-start" => {
                let (w, s, c) = parse_border_shorthand(val);
                if let Some(w) = w { props.border_left_width = Some(w); }
                if let Some(s) = s { props.border_style = Some(s); }
                if let Some(c) = c { props.border_color = Some(c); }
            }
            "border-right" | "border-inline-end" => {
                let (w, s, c) = parse_border_shorthand(val);
                if let Some(w) = w { props.border_right_width = Some(w); }
                if let Some(s) = s { props.border_style = Some(s); }
                if let Some(c) = c { props.border_color = Some(c); }
            }
            "border-width" => {
                let [top, right, bottom, left] = expand_box_shorthand(val);
                if let Some(v) = top { props.border_top_width = Some(v); }
                if let Some(v) = right { props.border_right_width = Some(v); }
                if let Some(v) = bottom { props.border_bottom_width = Some(v); }
                if let Some(v) = left { props.border_left_width = Some(v); }
            }
            "border-color" => { props.border_color = Some(val.to_string()); }
            "border-style" => {
                match val.to_lowercase().as_str() {
                    "solid" => props.border_style = Some(BorderStyle::Solid),
                    "dashed" => props.border_style = Some(BorderStyle::Dashed),
                    "dotted" => props.border_style = Some(BorderStyle::Dotted),
                    "double" => props.border_style = Some(BorderStyle::Double),
                    "none" | "hidden" => props.border_style = Some(BorderStyle::None),
                    _ => props.border_style = Some(BorderStyle::Solid),
                }
            }
            "border-collapse" => {
                // Handled at table level, not here
            }

            // ── Dimensions ──
            "width" => { props.width = Some(val.to_string()); }
            "max-width" => { props.max_width = Some(val.to_string()); }
            "min-width" => { props.min_width = Some(val.to_string()); }
            "height" => { props.height = Some(val.to_string()); }
            "min-height" => { props.min_height = Some(val.to_string()); }
            "max-height" => { props.max_height = Some(val.to_string()); }
            "top" => { props.top = Some(val.to_string()); }
            "bottom" => { props.bottom_pos = Some(val.to_string()); }
            "z-index" => { props.z_index = Some(val.to_string()); }

            // ── Visibility ──
            "visibility" => { props.visibility_hidden = val == "hidden"; }
            "opacity" => { props.opacity = val.parse().ok(); }

            // ── Background image (round-trip only) ──
            "background-image" => { props.background_image = Some(val.to_string()); }

            // ── Round-trip only (not rendered in TextBuffer) ──
            "float" => { props.float = Some(val.to_string()); }
            "clear" => { props.clear = Some(val.to_string()); }
            "position" => { props.position = Some(val.to_string()); }
            "border-radius" => { props.border_radius = Some(val.to_string()); }
            "overflow" | "overflow-x" | "overflow-y" => { props.overflow = Some(val.to_string()); }
            "right" => { props.right_pos = Some(val.to_string()); }
            "left" => { props.left_pos = Some(val.to_string()); }

            _ => {} // Ignore unknown properties
        }
    }
    props
}

fn parse_list_style_type(val: &str) -> Option<ListStyleType> {
    match val.to_lowercase().as_str() {
        "disc" => Some(ListStyleType::Disc),
        "square" => Some(ListStyleType::Square),
        "circle" => Some(ListStyleType::Circle),
        "none" => Some(ListStyleType::None),
        "decimal" => Some(ListStyleType::Decimal),
        "lower-alpha" | "lower-latin" => Some(ListStyleType::LowerAlpha),
        "upper-alpha" | "upper-latin" => Some(ListStyleType::UpperAlpha),
        "lower-roman" => Some(ListStyleType::LowerRoman),
        "upper-roman" => Some(ListStyleType::UpperRoman),
        _ => None,
    }
}

// ── CSS Cascade ────────────────────────────────────────────────────────────

/// Collect CSS rules from `<style>` blocks in the DOM.
/// Normal rules go into `rules`, `:hover` pseudo-class rules go into `hover_rules`.
pub fn collect_style_rules(
    node: &markup5ever_rcdom::Handle,
    rules: &mut HashMap<String, String>,
    hover_rules: &mut HashMap<String, String>,
) {
    if let markup5ever_rcdom::NodeData::Element { ref name, .. } = node.data {
        let tag = name.local.to_string();
        if tag == "style" {
            let mut css_block = String::new();
            for child in node.children.borrow().iter() {
                if let markup5ever_rcdom::NodeData::Text { ref contents } = child.data {
                    css_block.push_str(&contents.borrow());
                }
            }
            // Strip /* */ comments
            let mut cleaned = String::new();
            let bytes = css_block.as_bytes();
            let mut i = 0usize;
            while i < bytes.len() {
                if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                    i += 2;
                    while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                        i += 1;
                    }
                    if i + 1 < bytes.len() { i += 2; }
                } else {
                    cleaned.push(bytes[i] as char);
                    i += 1;
                }
            }
            // Parse rules
            let mut pos = 0usize;
            while pos < cleaned.len() {
                if let Some(open) = cleaned[pos..].find('{') {
                    let open_idx = pos + open;
                    if let Some(close_rel) = cleaned[open_idx..].find('}') {
                        let close_idx = open_idx + close_rel;
                        let selectors = cleaned[pos..open_idx].trim();
                        let decls = cleaned[open_idx + 1..close_idx].trim();
                        for sel in selectors.split(',') {
                            let mut sel_s = sel.trim().to_lowercase();
                            if sel_s.starts_with('@')
                                || sel_s.contains('>')
                                || sel_s.contains('+')
                                || sel_s.contains('~')
                                || sel_s.contains('[')
                            {
                                continue;
                            }
                            // Handle :hover pseudo-class — store separately
                            if sel_s.contains(':') {
                                if let Some(base) = sel_s.strip_suffix(":hover") {
                                    let base = if let Some(space) = base.rfind(' ') {
                                        &base[space + 1..]
                                    } else {
                                        base
                                    };
                                    if !base.is_empty() {
                                        hover_rules.entry(base.to_string())
                                            .and_modify(|e| { e.push(';'); e.push_str(decls); })
                                            .or_insert_with(|| decls.to_string());
                                    }
                                }
                                continue; // Skip other pseudo-classes
                            }
                            // For descendant selectors, take last part
                            if let Some(space) = sel_s.rfind(' ') {
                                sel_s = sel_s[space + 1..].to_string();
                            }
                            if sel_s.is_empty() { continue; }
                            rules.entry(sel_s)
                                .and_modify(|e| { e.push(';'); e.push_str(decls); })
                                .or_insert_with(|| decls.to_string());
                        }
                        pos = close_idx + 1;
                        continue;
                    } else { break; }
                } else { break; }
            }
        }
    }
    for child in node.children.borrow().iter() {
        collect_style_rules(child, rules, hover_rules);
    }
}

/// Resolve CSS cascade for a given element:
/// tag rules < .class rules < tag.class rules < #id rules < inline style.
pub fn apply_css_cascade(
    tag_name: &str,
    class_attr: Option<&str>,
    id_attr: Option<&str>,
    inline_style: Option<&str>,
    css_rules: &HashMap<String, String>,
) -> CssProperties {
    let mut combined = String::new();

    // 1. Tag selector
    if let Some(decls) = css_rules.get(tag_name) {
        combined.push_str(decls);
        combined.push(';');
    }

    // 2. Class selectors (can have multiple classes)
    if let Some(classes) = class_attr {
        for class in classes.split_whitespace() {
            let class = class.trim();
            if class.is_empty() { continue; }
            // .class
            let class_sel = format!(".{}", class);
            if let Some(decls) = css_rules.get(&class_sel) {
                combined.push_str(decls);
                combined.push(';');
            }
            // tag.class
            let tag_class_sel = format!("{}.{}", tag_name, class);
            if let Some(decls) = css_rules.get(&tag_class_sel) {
                combined.push_str(decls);
                combined.push(';');
            }
        }
    }

    // 3. ID selector
    if let Some(id) = id_attr {
        let id = id.trim();
        if !id.is_empty() {
            let id_sel = format!("#{}", id);
            if let Some(decls) = css_rules.get(&id_sel) {
                combined.push_str(decls);
                combined.push(';');
            }
        }
    }

    // 4. Inline style (highest specificity)
    if let Some(style) = inline_style {
        combined.push_str(style);
        combined.push(';');
    }

    parse_declarations(&combined)
}

// ── HTML Attribute Helpers ──────────────────────────────────────────────────

/// Convert HTML `<font size="N">` (1-7) to point size.
pub fn html_font_size_to_points(size: i32) -> f64 {
    match size {
        1 => 8.0,
        2 => 10.0,
        3 => 12.0,
        4 => 14.0,
        5 => 18.0,
        6 => 24.0,
        7 => 36.0,
        _ => 12.0,
    }
}

/// Convert heading level (1-6) to point size.
pub fn heading_size_to_points(level: i32) -> f64 {
    match level {
        1 => 32.0,
        2 => 24.0,
        3 => 18.72,
        4 => 16.0,
        5 => 13.28,
        6 => 10.72,
        _ => 16.0,
    }
}

/// Apply resolved CssProperties to a GTK TextTag.
pub fn apply_to_text_tag(props: &CssProperties, tag: &gtk::TextTag, is_block: bool) {
    if let Some(ref v) = props.color {
        tag.set_foreground(Some(v));
    }
    if let Some(ref v) = props.background_color {
        if is_block {
            tag.set_paragraph_background(Some(v));
        } else {
            tag.set_background(Some(v));
        }
    }
    if let Some(ref v) = props.font_family {
        tag.set_family(Some(v));
    }
    if let Some(v) = props.font_size {
        tag.set_size_points(v);
    }
    if let Some(v) = props.font_weight {
        tag.set_weight(v);
    }
    if let Some(ref v) = props.font_style {
        match v {
            FontStyle::Italic => tag.set_style(gtk::pango::Style::Italic),
            FontStyle::Oblique => tag.set_style(gtk::pango::Style::Oblique),
            FontStyle::Normal => tag.set_style(gtk::pango::Style::Normal),
        }
    }
    if let Some(true) = props.text_decoration_underline {
        tag.set_underline(gtk::pango::Underline::Single);
    }
    if let Some(false) = props.text_decoration_underline {
        tag.set_underline(gtk::pango::Underline::None);
    }
    if let Some(v) = props.text_decoration_line_through {
        tag.set_strikethrough(v);
    }
    if let Some(ref v) = props.text_align {
        tag.set_justification(*v);
    }
    if let Some(v) = props.effective_left_margin() {
        tag.set_left_margin(v);
    }
    if let Some(v) = props.effective_right_margin() {
        tag.set_right_margin(v);
    }
    if let Some(v) = props.margin_top {
        tag.set_pixels_above_lines(v);
    }
    if let Some(v) = props.margin_bottom {
        tag.set_pixels_below_lines(v);
    }
    if let Some(v) = props.text_indent {
        tag.set_indent(v);
    }
    if let Some(v) = props.line_height
        && v > 0.0 {
            // line-height translates to spacing between lines:
            // distribute half above and half below, plus inside wrap
            let extra = (v - 16.0).max(0.0) as i32; // excess over ~default line height
            let half = extra / 2;
            if half > 0 {
                tag.set_pixels_above_lines(half);
                tag.set_pixels_below_lines(half);
            }
            tag.set_pixels_inside_wrap(v as i32);
        }
    if is_block
        && let Some(ref v) = props.paragraph_background {
            tag.set_paragraph_background(Some(v));
        }
    if let Some(ref dir) = props.direction {
        // Set base direction for BiDi text
        // Note: Pango's BiDi algorithm handles mixed LTR/RTL automatically,
        // but we set the base direction to help with paragraph alignment
        // and neutral character ordering.
        match dir {
            TextDirection::Rtl => {
                // For RTL, we also want right-to-left text alignment unless overridden
                // GTK/Pango will handle the actual glyph reordering
            }
            TextDirection::Ltr => {}
        }
    }
    if let Some(v) = props.letter_spacing {
        tag.set_letter_spacing(v);
    }
    // vertical-align: super/sub
    if let Some(ref va) = props.vertical_align {
        match va {
            VerticalAlign::Super => {
                tag.set_rise(5000);
                tag.set_scale(0.75);
            }
            VerticalAlign::Sub => {
                tag.set_rise(-3000);
                tag.set_scale(0.75);
            }
            VerticalAlign::Baseline => {}
        }
    }
}

// ── List Marker Formatting ─────────────────────────────────────────────────

/// Format a list marker for the given style type and index.
pub fn format_list_marker(style: ListStyleType, index: i32) -> String {
    match style {
        ListStyleType::Disc => "\u{2022} ".to_string(),     // •
        ListStyleType::Square => "\u{25AA} ".to_string(),    // ▪
        ListStyleType::Circle => "\u{25CB} ".to_string(),    // ○
        ListStyleType::None => String::new(),
        ListStyleType::Decimal => format!("{}. ", index),
        ListStyleType::LowerAlpha => {
            let ch = (b'a' + ((index - 1).max(0) as u8) % 26) as char;
            format!("{}. ", ch)
        }
        ListStyleType::UpperAlpha => {
            let ch = (b'A' + ((index - 1).max(0) as u8) % 26) as char;
            format!("{}. ", ch)
        }
        ListStyleType::LowerRoman => format!("{}. ", to_roman(index, false)),
        ListStyleType::UpperRoman => format!("{}. ", to_roman(index, true)),
    }
}

fn to_roman(mut n: i32, upper: bool) -> String {
    if n <= 0 || n > 3999 { return n.to_string(); }
    let vals = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1];
    let syms = ["m", "cm", "d", "cd", "c", "xc", "l", "xl", "x", "ix", "v", "iv", "i"];
    let mut result = String::new();
    for (i, &val) in vals.iter().enumerate() {
        while n >= val {
            result.push_str(syms[i]);
            n -= val;
        }
    }
    if upper { result.to_uppercase() } else { result }
}

/// Apply text-transform to a string.
pub fn apply_text_transform(text: &str, transform: TextTransform) -> String {
    match transform {
        TextTransform::Uppercase => text.to_uppercase(),
        TextTransform::Lowercase => text.to_lowercase(),
        TextTransform::Capitalize => {
            let mut result = String::with_capacity(text.len());
            let mut capitalize_next = true;
            for ch in text.chars() {
                if ch.is_whitespace() {
                    capitalize_next = true;
                    result.push(ch);
                } else if capitalize_next {
                    for uc in ch.to_uppercase() {
                        result.push(uc);
                    }
                    capitalize_next = false;
                } else {
                    result.push(ch);
                }
            }
            result
        }
        TextTransform::None => text.to_string(),
    }
}
