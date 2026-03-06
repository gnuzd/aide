use serde::{Deserialize, Serialize};
use crossterm::style::Color;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Theme {
    pub name: String,
    /// Normal text colour — "#rrggbb"
    pub fg: String,
    /// H1 heading
    pub h1: String,
    /// H2-H6 headings
    pub headers: String,
    /// Bold text
    pub bold: String,
    /// Italic text
    pub italic: String,
    /// Inline-code / code-block foreground
    pub code_fg: String,
    /// Code-block background
    pub code_bg: String,
    /// Bullet-point colour
    pub bullet: String,
    /// User chat-bubble background
    pub user_bg: String,
    /// User chat-bubble text
    pub user_fg: String,
    /// syntect theme name for code blocks
    pub syntax_theme: String,
}

impl Theme {
    pub fn fg_color(&self) -> Color {
        let (r, g, b) = parse_hex(&self.fg);
        Color::Rgb { r, g, b }
    }
    pub fn user_bg_color(&self) -> Color {
        let (r, g, b) = parse_hex(&self.user_bg);
        Color::Rgb { r, g, b }
    }
    pub fn user_fg_color(&self) -> Color {
        let (r, g, b) = parse_hex(&self.user_fg);
        Color::Rgb { r, g, b }
    }
    pub fn h1_color(&self) -> Color {
        let (r, g, b) = parse_hex(&self.h1);
        Color::Rgb { r, g, b }
    }
    pub fn headers_color(&self) -> Color {
        let (r, g, b) = parse_hex(&self.headers);
        Color::Rgb { r, g, b }
    }
    pub fn bold_color(&self) -> Color {
        let (r, g, b) = parse_hex(&self.bold);
        Color::Rgb { r, g, b }
    }
    pub fn code_bg_color(&self) -> Color {
        let (r, g, b) = parse_hex(&self.code_bg);
        Color::Rgb { r, g, b }
    }
}

/// Parse "#rrggbb" → (r, g, b).  Falls back to white on any error.
pub fn parse_hex(hex: &str) -> (u8, u8, u8) {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&h[0..2], 16),
            u8::from_str_radix(&h[2..4], 16),
            u8::from_str_radix(&h[4..6], 16),
        ) {
            return (r, g, b);
        }
    }
    (0xff, 0xff, 0xff)
}

pub fn is_valid_hex(hex: &str) -> bool {
    let h = hex.trim_start_matches('#');
    h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())
}

/// All built-in themes in display order.
pub fn builtin_themes() -> Vec<Theme> {
    vec![gruvbox(), nord(), dracula(), tokyonight()]
}

/// Built-in themes + any user-created themes.
pub fn all_themes(custom: &[Theme]) -> Vec<Theme> {
    let mut themes = builtin_themes();
    themes.extend_from_slice(custom);
    themes
}

pub fn get_theme(name: &str, custom: &[Theme]) -> Theme {
    all_themes(custom)
        .into_iter()
        .find(|t| t.name == name)
        .unwrap_or_else(gruvbox)
}

// ── Built-in theme definitions ──────────────────────────────────────────────

pub fn gruvbox() -> Theme {
    Theme {
        name: "gruvbox".to_string(),
        fg: "#ebdbb2".to_string(),
        h1: "#fe8019".to_string(),
        headers: "#fabd2f".to_string(),
        bold: "#fe8019".to_string(),
        italic: "#b8bb26".to_string(),
        code_fg: "#8ec07c".to_string(),
        code_bg: "#3c3836".to_string(),
        bullet: "#fabd2f".to_string(),
        user_bg: "#3c3836".to_string(), // Softer than teal
        user_fg: "#ebdbb2".to_string(),
        syntax_theme: "base16-ocean.dark".to_string(),
    }
}

pub fn nord() -> Theme {
    Theme {
        name: "nord".to_string(),
        fg: "#d8dee9".to_string(),
        h1: "#88c0d0".to_string(),
        headers: "#81a1c1".to_string(),
        bold: "#88c0d0".to_string(),
        italic: "#a3be8c".to_string(),
        code_fg: "#81a1c1".to_string(),
        code_bg: "#3b4252".to_string(),
        bullet: "#88c0d0".to_string(),
        user_bg: "#3b4252".to_string(),
        user_fg: "#eceff4".to_string(),
        syntax_theme: "base16-ocean.dark".to_string(),
    }
}

pub fn dracula() -> Theme {
    Theme {
        name: "dracula".to_string(),
        fg: "#f8f8f2".to_string(),
        h1: "#ff79c6".to_string(),
        headers: "#bd93f9".to_string(),
        bold: "#ff79c6".to_string(),
        italic: "#50fa7b".to_string(),
        code_fg: "#50fa7b".to_string(),
        code_bg: "#282a36".to_string(),
        bullet: "#f1fa8c".to_string(),
        user_bg: "#44475a".to_string(),
        user_fg: "#f8f8f2".to_string(),
        syntax_theme: "base16-eighties.dark".to_string(),
    }
}

pub fn tokyonight() -> Theme {
    Theme {
        name: "tokyonight".to_string(),
        fg: "#a9b1d6".to_string(),
        h1: "#7aa2f7".to_string(),
        headers: "#bb9af7".to_string(),
        bold: "#7aa2f7".to_string(),
        italic: "#9ece6a".to_string(),
        code_fg: "#73daca".to_string(),
        code_bg: "#1a1b2e".to_string(),
        bullet: "#e0af68".to_string(),
        user_bg: "#24283b".to_string(),
        user_fg: "#c0caf5".to_string(),
        syntax_theme: "base16-ocean.dark".to_string(),
    }
}
