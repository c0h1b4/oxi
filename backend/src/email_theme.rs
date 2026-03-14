use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

static HEX_COLOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"#[0-9a-fA-F]{3,8}\b"#).unwrap());

static RGB_COLOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"rgb\s*\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)"#).unwrap());

static RGBA_COLOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"rgba\s*\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)"#).unwrap());

static STYLE_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?is)<style[^>]*>(.*?)</style>"#).unwrap());

static STYLE_ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)style\s*=\s*(?:"([^"]+)"|'([^']+)')"#).unwrap());

static BGCOLOR_ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)bgcolor\s*=\s*["']([^"']+)["']"#).unwrap());

static DARK_MEDIA_QUERY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)@media[^{]*prefers-color-scheme\s*:\s*dark[^{]*\{(.+?\})\s*\}"#).unwrap()
});

static BODY_CSS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)body\s*\{([^}]+)\}"#).unwrap());

static CLASS_CSS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\.[a-zA-Z][a-zA-Z0-9_-]*\s*\{([^}]+)\}"#).unwrap());

static ELEM_CSS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(body|table|td|div)\s*(?:,\s*#\w+)?\s*\{([^}]+)\}"#).unwrap()
});

static BG_PROP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)background(?:-color)?\s*:\s*([^;}"']+)"#).unwrap());

/// Named CSS colors mapped to RGB values.
type NamedColorList = Vec<(&'static str, (u8, u8, u8))>;
static NAMED_COLORS: LazyLock<NamedColorList> = LazyLock::new(|| {
    vec![
        ("black", (0, 0, 0)),
        ("white", (255, 255, 255)),
        ("red", (255, 0, 0)),
        ("green", (0, 128, 0)),
        ("blue", (0, 0, 255)),
        ("yellow", (255, 255, 0)),
        ("navy", (0, 0, 128)),
        ("gray", (128, 128, 128)),
        ("grey", (128, 128, 128)),
        ("silver", (192, 192, 192)),
        ("maroon", (128, 0, 0)),
        ("purple", (128, 0, 128)),
        ("teal", (0, 128, 128)),
        ("olive", (128, 128, 0)),
        ("aqua", (0, 255, 255)),
        ("fuchsia", (255, 0, 255)),
        ("lime", (0, 255, 0)),
        ("darkgray", (169, 169, 169)),
        ("darkgrey", (169, 169, 169)),
        ("lightgray", (211, 211, 211)),
        ("lightgrey", (211, 211, 211)),
        ("whitesmoke", (245, 245, 245)),
        ("ghostwhite", (248, 248, 255)),
        ("floralwhite", (255, 250, 240)),
        ("linen", (250, 240, 230)),
        ("cornsilk", (255, 248, 220)),
        ("gainsboro", (220, 220, 220)),
    ]
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailTheme {
    Light = 0,
    Dark = 1,
    Transparent = 2,
    Adaptive = 3,
}

impl EmailTheme {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Detect whether an email's HTML is light, dark, transparent, or adaptive.
///
/// Detection priority:
/// 1. If the email has `@media(prefers-color-scheme:dark)` rules with background
///    overrides, classify as Adaptive (it handles dark mode natively).
/// 2. Extract background colors from body, table, td, div elements (inline styles,
///    style tags, bgcolor attrs). Group by luminance (light vs dark). If one group
///    dominates (>= 50% of total weight), return that group's classification.
/// 3. If no backgrounds found, return Transparent.
pub fn detect_email_theme(html: &str) -> Option<EmailTheme> {
    if html.trim().is_empty() {
        return None;
    }

    let decoded = decode_quoted_printable(html);

    // Check for @media(prefers-color-scheme:dark) with background overrides.
    // These emails handle their own dark adaptation — classify as Adaptive
    // so the frontend sets color-scheme and lets the email CSS handle it.
    if has_dark_mode_media_query(&decoded) {
        return Some(EmailTheme::Adaptive);
    }

    let backgrounds = extract_background_colors(&decoded);

    if backgrounds.is_empty() {
        return Some(EmailTheme::Transparent);
    }

    classify_by_luminance_groups(&backgrounds)
}

/// Check if the HTML has a @media(prefers-color-scheme:dark) block that sets
/// background colors, indicating the email handles dark mode natively.
fn has_dark_mode_media_query(html: &str) -> bool {
    let lower = html.to_lowercase();
    for cap in DARK_MEDIA_QUERY_RE.captures_iter(&lower) {
        let block = &cap[1];
        if block.contains("background") {
            return true;
        }
    }
    false
}

/// Group all extracted colors into "light" and "dark" buckets by their
/// luminance, then check which bucket dominates.
fn classify_by_luminance_groups(colors: &[BackgroundColor]) -> Option<EmailTheme> {
    let mut light_weight: f32 = 0.0;
    let mut dark_weight: f32 = 0.0;
    let mut total_weight: f32 = 0.0;
    let mut has_body_background = false;

    for bg in colors {
        let luminance = calculate_luminance(&bg.color);
        total_weight += bg.element_weight;
        if bg.element_weight >= 1.0 {
            has_body_background = true;
        }
        if luminance < 0.5 {
            dark_weight += bg.element_weight;
        } else {
            light_weight += bg.element_weight;
        }
    }

    if total_weight == 0.0 {
        return Some(EmailTheme::Transparent);
    }

    let light_coverage = light_weight / total_weight;
    let dark_coverage = dark_weight / total_weight;

    // Require at least 50% of weighted colors to agree
    if light_coverage >= 0.5 {
        // If the only signal is from a wrapper table/td with white background
        // and there's no body-level background, treat as transparent.
        // A lone "white" on a wrapper element is usually just a reset, not
        // intentional theming.
        if !has_body_background && is_only_white_wrapper(colors) {
            return Some(EmailTheme::Transparent);
        }
        Some(EmailTheme::Light)
    } else if dark_coverage >= 0.5 {
        // Guard against tiny accent elements being the only signal:
        // if total weight is very low (only 1-2 low-weight elements like td),
        // that's likely an accent, not a real background.
        if total_weight <= 1.0 && colors.len() <= 2 && !has_body_background {
            Some(EmailTheme::Transparent)
        } else {
            Some(EmailTheme::Dark)
        }
    } else {
        Some(EmailTheme::Transparent)
    }
}

/// Check if all color signals are just white (#ffffff or named "white") on
/// wrapper elements (table or td) without any body-level background. This
/// usually indicates a boilerplate reset rather than intentional light theming.
fn is_only_white_wrapper(colors: &[BackgroundColor]) -> bool {
    if colors.is_empty() {
        return false;
    }
    for bg in colors {
        // If there's a body-level background, it's intentional
        if bg.element_weight >= 1.0 {
            return false;
        }
        // Only consider pure white as a wrapper reset
        let c = bg.color.trim().to_lowercase();
        if c != "#ffffff" && c != "white" {
            return false;
        }
    }
    true
}

fn decode_quoted_printable(html: &str) -> String {
    let mut result = html.to_string();
    result = result.replace("=3D", "=");
    result = result.replace("=3d", "=");
    result = result.replace("=20", " ");
    result = result.replace("=\r\n", "");
    result = result.replace("=\n", "");
    result
}

#[derive(Debug, Clone)]
struct BackgroundColor {
    color: String,
    element_weight: f32,
}

fn extract_background_colors(html: &str) -> Vec<BackgroundColor> {
    let mut colors = Vec::new();

    extract_inline_style_colors(html, &mut colors);
    extract_style_tag_colors(html, &mut colors);
    extract_bgcolor_attrs(html, &mut colors);

    // Deduplicate: if the same color appears on the same element from both
    // bgcolor and style attributes, keep only the one with higher weight.
    deduplicate_colors(&mut colors);

    colors
}

/// Remove duplicate color entries that likely come from the same element
/// having both bgcolor="X" and style="background-color:X".
fn deduplicate_colors(colors: &mut Vec<BackgroundColor>) {
    if colors.len() <= 1 {
        return;
    }

    // Track seen (normalized_color, weight) pairs and remove exact duplicates
    let mut seen: HashSet<String> = HashSet::new();
    colors.retain(|bg| {
        let key = format!("{}:{:.1}", bg.color, bg.element_weight);
        seen.insert(key)
    });
}

fn extract_inline_style_colors(html: &str, colors: &mut Vec<BackgroundColor>) {
    let element_weights: &[(&str, f32)] =
        &[("body", 1.0), ("table", 0.8), ("td", 0.6), ("div", 0.4)];

    for (tag, weight) in element_weights {
        let tag_pattern = format!(r#"<{tag}\b[^>]*>"#);
        if let Ok(tag_re) = Regex::new(&tag_pattern) {
            for m in tag_re.find_iter(html) {
                let tag_content = m.as_str();
                let extracted = extract_colors_from_style(tag_content);
                for color in extracted {
                    if !is_transparent(&color) {
                        let normalized = normalize_color(&color);
                        if !normalized.is_empty() {
                            colors.push(BackgroundColor {
                                color: normalized,
                                element_weight: *weight,
                            });
                        }
                    }
                }
            }
        }
    }
}

fn extract_style_tag_colors(html: &str, colors: &mut Vec<BackgroundColor>) {
    for cap in STYLE_TAG_RE.captures_iter(html) {
        let style_content = &cap[1];

        let lower = style_content.to_lowercase();
        if !lower.contains("background") {
            continue;
        }

        // Extract from body{...} rules
        if let Some(body_cap) = BODY_CSS_RE.captures(style_content) {
            let body_styles = &body_cap[1];
            if body_styles.to_lowercase().contains("background") {
                extract_all_colors_from_css(body_styles, 1.0, colors);
            }
        }

        // Extract from class rules that mention background
        for class_cap in CLASS_CSS_RE.captures_iter(style_content) {
            let class_styles = &class_cap[1];
            if class_styles.to_lowercase().contains("background") {
                extract_all_colors_from_css(class_styles, 0.3, colors);
            }
        }

        // Extract from element-level rules (td{...}, table{...}, etc.)
        for elem_cap in ELEM_CSS_RE.captures_iter(style_content) {
            let tag = elem_cap[1].to_lowercase();
            let elem_styles = &elem_cap[2];
            if elem_styles.to_lowercase().contains("background") {
                let weight = match tag.as_str() {
                    "body" => 1.0,
                    "table" => 0.8,
                    "td" => 0.6,
                    "div" => 0.4,
                    _ => 0.3,
                };
                extract_all_colors_from_css(elem_styles, weight, colors);
            }
        }
    }
}

/// Extract color values only from background/background-color properties in a CSS rule body.
fn extract_all_colors_from_css(css: &str, weight: f32, colors: &mut Vec<BackgroundColor>) {
    let lower = css.to_lowercase();

    // Only extract colors from background/background-color property values,
    // not from the entire CSS rule (which may contain color, border-color, etc.)
    for bg_cap in BG_PROP_RE.captures_iter(&lower) {
        let bg_value = bg_cap[1].trim();

        for hex_cap in HEX_COLOR_RE.captures_iter(bg_value) {
            let color = normalize_color(&hex_cap[0]);
            if !is_transparent(&color) && !color.is_empty() {
                colors.push(BackgroundColor {
                    color,
                    element_weight: weight,
                });
            }
        }

        for rgb_cap in RGB_COLOR_RE.captures_iter(bg_value) {
            let color = format!("rgb({}, {}, {})", &rgb_cap[1], &rgb_cap[2], &rgb_cap[3]);
            if !is_transparent(&color) {
                colors.push(BackgroundColor {
                    color,
                    element_weight: weight,
                });
            }
        }

        for rgba_cap in RGBA_COLOR_RE.captures_iter(bg_value) {
            let color = format!("rgb({}, {}, {})", &rgba_cap[1], &rgba_cap[2], &rgba_cap[3]);
            if !is_transparent(&color) {
                colors.push(BackgroundColor {
                    color,
                    element_weight: weight,
                });
            }
        }

        // Check for named colors
        for (name, _rgb) in NAMED_COLORS.iter() {
            if bg_value.contains(name) {
                colors.push(BackgroundColor {
                    color: name.to_string(),
                    element_weight: weight,
                });
                break;
            }
        }
    }
}

fn extract_bgcolor_attrs(html: &str, colors: &mut Vec<BackgroundColor>) {
    let element_weights: &[(&str, f32)] = &[("body", 1.0), ("table", 0.8), ("td", 0.6)];

    for (tag, weight) in element_weights {
        let tag_pattern = format!(r#"<{tag}\b[^>]*>"#);
        if let Ok(tag_re) = Regex::new(&tag_pattern) {
            for m in tag_re.find_iter(html) {
                let tag_content = m.as_str();
                if let Some(cap) = BGCOLOR_ATTR_RE.captures(tag_content) {
                    let color = cap[1].to_string();
                    if !is_transparent(&color) {
                        let normalized = normalize_color(&color);
                        if !normalized.is_empty() {
                            colors.push(BackgroundColor {
                                color: normalized,
                                element_weight: *weight,
                            });
                        }
                    }
                }
            }
        }
    }
}

fn extract_colors_from_style(tag_content: &str) -> Vec<String> {
    let mut colors = Vec::new();

    if let Some(cap) = STYLE_ATTR_RE.captures(tag_content) {
        let style_value = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map(|m| m.as_str().to_lowercase())
            .unwrap_or_default();

        // Only extract colors from background/background-color property values,
        // not from the entire style string (which may contain border-color, color, etc.)
        for bg_cap in BG_PROP_RE.captures_iter(&style_value) {
            let bg_value = bg_cap[1].trim();

            for hex_cap in HEX_COLOR_RE.captures_iter(bg_value) {
                colors.push(hex_cap[0].to_string());
            }

            for rgb_cap in RGB_COLOR_RE.captures_iter(bg_value) {
                colors.push(format!(
                    "rgb({}, {}, {})",
                    &rgb_cap[1], &rgb_cap[2], &rgb_cap[3]
                ));
            }

            for rgba_cap in RGBA_COLOR_RE.captures_iter(bg_value) {
                colors.push(format!(
                    "rgb({}, {}, {})",
                    &rgba_cap[1], &rgba_cap[2], &rgba_cap[3]
                ));
            }

            // Check for named colors
            for (name, _rgb) in NAMED_COLORS.iter() {
                if bg_value.contains(name) {
                    colors.push(name.to_string());
                    break;
                }
            }
        }
    }

    colors
}

fn is_transparent(color: &str) -> bool {
    let c = color.to_lowercase();
    c.contains("transparent")
        || c.contains("rgba(0, 0, 0, 0)")
        || c.contains("rgba(0,0,0,0)")
        || c == "initial"
        || c == "inherit"
        || c == "none"
}

fn normalize_color(color: &str) -> String {
    let c = color.trim().to_lowercase();
    if let Some(hex) = c.strip_prefix('#') {
        match hex.len() {
            3 => {
                let mut chars = hex.chars();
                let r = chars.next().unwrap_or('0');
                let g = chars.next().unwrap_or('0');
                let b = chars.next().unwrap_or('0');
                format!("#{r}{r}{g}{g}{b}{b}")
            }
            6 => c,
            8 => {
                // 8-digit hex: strip alpha channel
                format!("#{}", &hex[0..6])
            }
            _ => c,
        }
    } else {
        c
    }
}

fn calculate_luminance(color: &str) -> f64 {
    let (r, g, b) = parse_color(color);
    (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0
}

fn parse_color(color: &str) -> (u8, u8, u8) {
    let c = color.trim().to_lowercase();

    // Check named colors first
    for (name, rgb) in NAMED_COLORS.iter() {
        if c == *name {
            return *rgb;
        }
    }

    if let Some(hex) = c.strip_prefix('#') {
        let hex = hex.trim();
        if hex.len() >= 6 {
            return (
                u8::from_str_radix(&hex[0..2], 16).unwrap_or(255),
                u8::from_str_radix(&hex[2..4], 16).unwrap_or(255),
                u8::from_str_radix(&hex[4..6], 16).unwrap_or(255),
            );
        }
    }

    if let Some(cap) = RGB_COLOR_RE.captures(&c) {
        return (
            cap[1].parse().unwrap_or(255),
            cap[2].parse().unwrap_or(255),
            cap[3].parse().unwrap_or(255),
        );
    }

    if let Some(cap) = RGBA_COLOR_RE.captures(&c) {
        return (
            cap[1].parse().unwrap_or(255),
            cap[2].parse().unwrap_or(255),
            cap[3].parse().unwrap_or(255),
        );
    }

    // Unknown color - default to white (high luminance = light, safe fallback)
    (255, 255, 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_dark_email_by_background() {
        let html = r#"<html><body style="background-color: #101519">Dark content</body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Dark));
    }

    #[test]
    fn test_detect_light_email_by_background() {
        let html = r#"<html><body style="background-color: #ffffff">Light content</body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Light));
    }

    #[test]
    fn test_detect_light_email_with_table() {
        let html = r#"<html><body><table style="background-color: #f5f8fa"><tr><td>Light</td></tr></table></body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Light));
    }

    #[test]
    fn test_detect_transparent_email() {
        let html = r#"<html><body>Just text, no background</body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Transparent));
    }

    #[test]
    fn test_empty_html_returns_none() {
        assert_eq!(detect_email_theme(""), None);
    }

    #[test]
    fn test_luminance_calculation() {
        assert!(calculate_luminance("#000000") < 0.1);
        assert!(calculate_luminance("#ffffff") > 0.9);
        assert!(calculate_luminance("#101519") < 0.5);
    }

    #[test]
    fn test_adaptive_media_query_detection() {
        let html = r#"<html><head><style>
            body { background: #F0EEE6; }
            @media(prefers-color-scheme:dark) {
                body { background: #1f1f1f !important; }
            }
        </style></head><body>Content</body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Adaptive));
    }

    #[test]
    fn test_luminance_grouping_mixed_light_colors() {
        // Multiple light colors should still classify as Light
        let html = r#"<html>
            <body style="background-color: #ffffff">
                <table style="background-color: #FAF9F5">
                    <tr><td style="background-color: #F0EEE6">Content</td></tr>
                </table>
                <div style="background-color: #E4E4E4">Footer</div>
            </body>
        </html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Light));
    }

    #[test]
    fn test_accent_td_not_treated_as_background() {
        // A single td with a dark accent color shouldn't classify the email as dark
        let html = r##"<html><body><table><tr><td bgcolor="#2672ec">Click here</td></tr></table></body></html>"##;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Transparent));
    }

    #[test]
    fn test_deduplicate_bgcolor_and_style() {
        // Same color from both bgcolor and style on same td should be deduplicated
        let html = r##"<html><body><table><tr><td bgcolor="#2672ec" style="background-color:#2672ec">Click</td></tr></table></body></html>"##;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Transparent));
    }

    #[test]
    fn test_lone_white_wrapper_table_is_transparent() {
        // A wrapper table with background-color:white but no body background
        // should be treated as transparent (boilerplate reset)
        let html = r#"<html><body><table style="background-color:white"><tr><td>Content</td></tr></table></body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Transparent));
    }

    #[test]
    fn test_named_color_white() {
        // Body with white background is intentional
        let html = r#"<html><body style="background-color: white">Content</body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Light));
    }

    #[test]
    fn test_named_color_black() {
        let html = r#"<html><body style="background-color: black">Content</body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Dark));
    }

    #[test]
    fn test_rgb_color() {
        let html = r#"<html><body><table style="background-color: rgb(214, 214, 214)"><tr><td>Content</td></tr></table></body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Light));
    }

    #[test]
    fn test_3_char_hex() {
        let html = r#"<html><body style="background-color: #fff">Content</body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Light));
    }

    #[test]
    fn test_quoted_printable_decoding() {
        let encoded = r#"<body style=3D"background-color: #101519">"#;
        let decoded = decode_quoted_printable(encoded);
        assert!(decoded.contains("style=\"background-color:"));
    }

    #[test]
    fn test_detect_dark_with_style_tag() {
        let html = r#"<html><head><style>body { background-color: #101519; }</style></head><body>Dark</body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Dark));
    }
}
