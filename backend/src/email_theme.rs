use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailTheme {
    Light = 0,
    Dark = 1,
    Transparent = 2,
}

impl EmailTheme {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

pub fn detect_email_theme(html: &str) -> Option<EmailTheme> {
    if html.trim().is_empty() {
        return None;
    }

    let html_lower = html.to_lowercase();

    if let Some(theme) = check_color_scheme_meta(&html_lower) {
        return Some(theme);
    }

    let backgrounds = extract_background_colors(html);

    if backgrounds.is_empty() {
        return Some(EmailTheme::Transparent);
    }

    let dominant = find_dominant_background(&backgrounds, html.len());

    match dominant {
        Some((color, coverage)) if coverage > 0.7 => {
            let luminance = calculate_luminance(&color);
            if luminance < 0.179 {
                Some(EmailTheme::Dark)
            } else {
                Some(EmailTheme::Light)
            }
        }
        _ => Some(EmailTheme::Transparent),
    }
}

fn check_color_scheme_meta(html_lower: &str) -> Option<EmailTheme> {
    let re = Regex::new(r#"<meta\s+[^>]*name\s*=\s*["'](?:color-scheme|supported-color-schemes)["'][^>]*content\s*=\s*["']([^"']+)["']"#).ok()?;
    let re2 = Regex::new(r#"<meta\s+[^>]*content\s*=\s*["']([^"']+)["'][^>]*name\s*=\s*["'](?:color-scheme|supported-color-schemes)["']"#).ok()?;

    for cap in re.captures_iter(html_lower) {
        let content = cap.get(1)?.as_str();
        if content.contains("dark") && !content.contains("light") {
            return Some(EmailTheme::Dark);
        }
        if content.contains("light") && !content.contains("dark") {
            return Some(EmailTheme::Light);
        }
    }

    for cap in re2.captures_iter(html_lower) {
        let content = cap.get(1)?.as_str();
        if content.contains("dark") && !content.contains("light") {
            return Some(EmailTheme::Dark);
        }
        if content.contains("light") && !content.contains("dark") {
            return Some(EmailTheme::Light);
        }
    }

    None
}

#[derive(Debug, Clone)]
struct BackgroundColor {
    color: String,
    element_weight: f32,
}

fn extract_background_colors(html: &str) -> Vec<BackgroundColor> {
    let mut colors = Vec::new();

    let bg_style_re = Regex::new(r#"background(?:-color)?:\s*([^;}"'\s]+)"#).unwrap();
    let bg_attr_re = Regex::new(r#"bgcolor\s*=\s*["']([^"']+)["']"#).unwrap();

    let element_weights: &[(&str, f32)] =
        &[("body", 1.0), ("table", 0.8), ("td", 0.6), ("div", 0.4)];

    for (tag, weight) in element_weights {
        let tag_pattern = format!(r#"<{tag}\b[^>]*>"#);
        if let Ok(tag_re) = Regex::new(&tag_pattern) {
            for m in tag_re.find_iter(html) {
                let tag_content = m.as_str();

                if let Some(cap) = bg_style_re.captures(tag_content) {
                    let color = cap[1].trim().to_string();
                    if !is_transparent(&color) {
                        colors.push(BackgroundColor {
                            color: normalize_color(&color),
                            element_weight: *weight,
                        });
                    }
                }

                if let Some(cap) = bg_attr_re.captures(tag_content) {
                    let color = cap[1].trim().to_string();
                    if !is_transparent(&color) {
                        colors.push(BackgroundColor {
                            color: normalize_color(&color),
                            element_weight: *weight,
                        });
                    }
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
}

fn normalize_color(color: &str) -> String {
    let c = color.trim().to_lowercase();
    if c.starts_with('#') && c.len() == 4 {
        let r = c.chars().nth(1).unwrap_or('0');
        let g = c.chars().nth(2).unwrap_or('0');
        let b = c.chars().nth(3).unwrap_or('0');
        format!("#{r}{r}{g}{g}{b}{b}")
    } else {
        c
    }
}

fn find_dominant_background(colors: &[BackgroundColor], _html_len: usize) -> Option<(String, f32)> {
    if colors.is_empty() {
        return None;
    }

    let mut color_scores: std::collections::HashMap<String, f32> = std::collections::HashMap::new();

    for bg in colors {
        *color_scores.entry(bg.color.clone()).or_insert(0.0) += bg.element_weight;
    }

    let total_weight: f32 = colors.iter().map(|c| c.element_weight).sum();

    color_scores
        .into_iter()
        .map(|(color, score)| (color, score / total_weight))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
}

fn calculate_luminance(color: &str) -> f64 {
    let (r, g, b) = parse_color(color);
    (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0
}

fn parse_color(color: &str) -> (u8, u8, u8) {
    let c = color.trim().to_lowercase();

    if let Some(hex) = c.strip_prefix('#') {
        let hex = hex.trim();
        if hex.len() == 6 {
            return (
                u8::from_str_radix(&hex[0..2], 16).unwrap_or(255),
                u8::from_str_radix(&hex[2..4], 16).unwrap_or(255),
                u8::from_str_radix(&hex[4..6], 16).unwrap_or(255),
            );
        }
    }

    let rgb_re = Regex::new(r#"rgb\s*\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)"#).unwrap();
    if let Some(cap) = rgb_re.captures(&c) {
        return (
            cap[1].parse().unwrap_or(255),
            cap[2].parse().unwrap_or(255),
            cap[3].parse().unwrap_or(255),
        );
    }

    let rgba_re = Regex::new(r#"rgba\s*\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)"#).unwrap();
    if let Some(cap) = rgba_re.captures(&c) {
        return (
            cap[1].parse().unwrap_or(255),
            cap[2].parse().unwrap_or(255),
            cap[3].parse().unwrap_or(255),
        );
    }

    (255, 255, 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_dark_email_with_meta() {
        let html = r#"<html><head><meta name="color-scheme" content="dark"></head><body>Dark</body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Dark));
    }

    #[test]
    fn test_detect_light_email_with_meta() {
        let html = r#"<html><head><meta name="color-scheme" content="light"></head><body>Light</body></html>"#;
        assert_eq!(detect_email_theme(html), Some(EmailTheme::Light));
    }

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
        assert!(calculate_luminance("#101519") < 0.2);
    }
}
