use serde::{Deserialize, Serialize};
use crossterm::style::Color;
use termimad::{MadSkin, StyledChar, crossterm::style::Attribute as MadAttribute};

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

    pub fn to_mad_skin(&self) -> MadSkin {
        let mut skin = MadSkin::default();
        let (fg_r, fg_g, fg_b) = parse_hex(&self.fg);
        skin.set_fg(termimad::rgb(fg_r, fg_g, fg_b));

        let (h1_r, h1_g, h1_b) = parse_hex(&self.h1);
        skin.headers[0].set_fg(termimad::rgb(h1_r, h1_g, h1_b));
        skin.headers[0].compound_style.add_attr(MadAttribute::Bold);

        let (h_r, h_g, h_b) = parse_hex(&self.headers);
        for i in 1..=5 {
            skin.headers[i].set_fg(termimad::rgb(h_r, h_g, h_b));
        }

        let (b_r, b_g, b_b) = parse_hex(&self.bold);
        skin.bold.set_fg(termimad::rgb(b_r, b_g, b_b));

        let (i_r, i_g, i_b) = parse_hex(&self.italic);
        skin.italic.set_fg(termimad::rgb(i_r, i_g, i_b));

        let (c_fg_r, c_fg_g, c_fg_b) = parse_hex(&self.code_fg);
        let (c_bg_r, c_bg_g, c_bg_b) = parse_hex(&self.code_bg);
        skin.inline_code.set_fg(termimad::rgb(c_fg_r, c_fg_g, c_fg_b));
        skin.inline_code.set_bg(termimad::rgb(c_bg_r, c_bg_g, c_bg_b));
        skin.code_block.set_fg(termimad::rgb(c_fg_r, c_fg_g, c_fg_b));
        skin.code_block.set_bg(termimad::rgb(c_bg_r, c_bg_g, c_bg_b));

        let (bul_r, bul_g, bul_b) = parse_hex(&self.bullet);
        skin.bullet = StyledChar::from_fg_char(termimad::rgb(bul_r, bul_g, bul_b), '•');

        skin
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
/// All built-in themes in display order.
pub fn builtin_themes() -> Vec<Theme> {
    vec![
        gruvbox(), 
        tokyonight(), 
        catppuccin(), 
        rose_pine(), 
        kanagawa(),
        everforest(),
        nord(), 
        dracula()
    ]
}

pub fn kanagawa() -> Theme {
    Theme {
        name: "kanagawa".to_string(),
        fg: "#dcd7ba".to_string(),
        h1: "#938aa9".to_string(),
        headers: "#7e9cd8".to_string(),
        bold: "#e82424".to_string(),
        italic: "#98bb6c".to_string(),
        code_fg: "#dca561".to_string(),
        code_bg: "#1f1f28".to_string(),
        bullet: "#7e9cd8".to_string(),
        user_bg: "#2a2a37".to_string(),
        user_fg: "#dcd7ba".to_string(),
        syntax_theme: "base16-ocean.dark".to_string(),
    }
}

pub fn everforest() -> Theme {
    Theme {
        name: "everforest".to_string(),
        fg: "#d3c6aa".to_string(),
        h1: "#a7c080".to_string(),
        headers: "#dbbc7f".to_string(),
        bold: "#e67e80".to_string(),
        italic: "#7fbbb3".to_string(),
        code_fg: "#83c092".to_string(),
        code_bg: "#2d353b".to_string(),
        bullet: "#dbbc7f".to_string(),
        user_bg: "#343f44".to_string(),
        user_fg: "#d3c6aa".to_string(),
        syntax_theme: "base16-ocean.dark".to_string(),
    }
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
        user_bg: "#3c3836".to_string(),
        user_fg: "#ebdbb2".to_string(),
        syntax_theme: "base16-ocean.dark".to_string(),
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

pub fn catppuccin() -> Theme {
    Theme {
        name: "catppuccin".to_string(),
        fg: "#cad3f5".to_string(),
        h1: "#8aadf4".to_string(),
        headers: "#c6a0f6".to_string(),
        bold: "#ee99a0".to_string(),
        italic: "#a6da95".to_string(),
        code_fg: "#8bd5ca".to_string(),
        code_bg: "#1e2030".to_string(),
        bullet: "#f5bde6".to_string(),
        user_bg: "#363a4f".to_string(),
        user_fg: "#cad3f5".to_string(),
        syntax_theme: "base16-ocean.dark".to_string(),
    }
}

pub fn rose_pine() -> Theme {
    Theme {
        name: "rose-pine".to_string(),
        fg: "#e0def4".to_string(),
        h1: "#ebbcba".to_string(),
        headers: "#c4a7e7".to_string(),
        bold: "#eb6f92".to_string(),
        italic: "#9ccfd8".to_string(),
        code_fg: "#f6c177".to_string(),
        code_bg: "#191724".to_string(),
        bullet: "#31748f".to_string(),
        user_bg: "#26233a".to_string(),
        user_fg: "#e0def4".to_string(),
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
