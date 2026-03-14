use mail_parser::MessageParser;
use std::fs;
use std::path::Path;

#[allow(dead_code)]
#[path = "../email_theme.rs"]
mod email_theme;

use email_theme::{detect_email_theme, EmailTheme};

fn main() {
    let debugging_dir = Path::new("../debugging");

    let test_cases = [
        ("dark-1.eml", "dark"),
        ("dark-2.eml", "adaptive"),
        ("dark-3.eml", "adaptive"),
        ("dark-4.eml", "adaptive"),
        ("light-1.eml", "light"),
        ("light-2.eml", "light"),
        ("light-3.eml", "light"),
        ("light-4.eml", "adaptive"),
        ("light-5.eml", "light"),
        ("light-6.eml", "light"),
        ("light-7.eml", "adaptive"),
        ("light-8.eml", "light"),
        ("light-9.eml", "light"),
        ("transparent-1.eml", "transparent"),
        ("transparent-2.eml", "transparent"),
        ("transparent-3.eml", "transparent"),
        ("transparent-4.eml", "transparent"),
        ("transparent-5.eml", "transparent"),
        ("text-1.eml", "transparent"),
        ("text-2.eml", "transparent"),
        ("new-light.eml", "light"),
        ("light-10.eml", "light"),
        ("light-11.eml", "light"),
        ("light-12.eml", "adaptive"),
        ("padding.eml", "transparent"),
        ("light-13.eml", "light"),
        ("text-3.eml", "transparent"),
    ];

    println!(
        "{:<20} | {:<12} | {:<12} | Result",
        "File", "Expected", "Detected"
    );
    println!("{}", "-".repeat(60));

    let mut passed = 0;
    let mut failed = 0;

    for (filename, expected) in test_cases {
        let path = debugging_dir.join(filename);
        let result = test_file(&path, expected);
        let status = if result.0 { "✅" } else { "❌" };
        println!(
            "{:<20} | {:<12} | {:<12} | {}",
            filename, expected, result.1, status
        );
        if result.0 {
            passed += 1;
        } else {
            failed += 1;
        }
    }

    println!("{}", "-".repeat(60));
    println!("Passed: {}, Failed: {}", passed, failed);
}

fn test_file(path: &Path, expected: &str) -> (bool, String) {
    let content = match fs::read(path) {
        Ok(c) => c,
        Err(e) => return (false, format!("Error reading: {}", e)),
    };

    let msg = match MessageParser::default().parse(&content) {
        Some(m) => m,
        None => return (false, "Parse failed".to_string()),
    };

    let html = msg.body_html(0).map(|s| s.to_string());

    let detected = match html {
        Some(ref h) if !h.is_empty() => match detect_email_theme(h) {
            Some(EmailTheme::Dark) => "dark",
            Some(EmailTheme::Light) => "light",
            Some(EmailTheme::Transparent) => "transparent",
            Some(EmailTheme::Adaptive) => "adaptive",
            None => "none",
        },
        _ => "none",
    };

    (detected == expected, detected.to_string())
}
