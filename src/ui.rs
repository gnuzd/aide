use crate::theme::{Theme, parse_hex};
// termimad bundles crossterm 0.23 internally; use its re-export so Color types match
use termimad::crossterm::style::Color;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use termimad::MadSkin;

fn hex_to_color(hex: &str) -> Color {
    let (r, g, b) = parse_hex(hex);
    Color::Rgb { r, g, b }
}

pub fn theme_to_skin(theme: &Theme) -> MadSkin {
    let mut skin = MadSkin::default();
    skin.paragraph.set_fg(hex_to_color(&theme.fg));
    skin.set_headers_fg(hex_to_color(&theme.headers));
    skin.headers[0].set_fg(hex_to_color(&theme.h1));
    skin.bold.set_fg(hex_to_color(&theme.bold));
    skin.italic.set_fg(hex_to_color(&theme.italic));
    skin.inline_code.set_fg(hex_to_color(&theme.code_fg));
    skin.inline_code.set_bg(hex_to_color(&theme.code_bg));
    skin.code_block.set_fg(hex_to_color(&theme.code_fg));
    skin.code_block.set_bg(hex_to_color(&theme.code_bg));
    skin.bullet.set_fg(hex_to_color(&theme.bullet));
    skin
}

pub struct CodeHighlighter {
    ss: SyntaxSet,
    ts: ThemeSet,
    pub syntax_theme: String,
}

impl CodeHighlighter {
    pub fn with_syntax_theme(name: &str) -> Self {
        Self {
            ss: SyntaxSet::load_defaults_newlines(),
            ts: ThemeSet::load_defaults(),
            syntax_theme: name.to_string(),
        }
    }

    pub fn highlight(&self, code: &str, lang: &str) -> String {
        let syntax = if lang.is_empty() {
            self.ss.find_syntax_plain_text()
        } else {
            self.ss
                .find_syntax_by_token(lang)
                .unwrap_or_else(|| self.ss.find_syntax_plain_text())
        };
        let theme = self
            .ts
            .themes
            .get(&self.syntax_theme)
            .or_else(|| self.ts.themes.get("base16-ocean.dark"))
            .or_else(|| self.ts.themes.values().next())
            .unwrap();
        let mut h = HighlightLines::new(syntax, theme);
        let mut out = String::new();
        for line in LinesWithEndings::from(code) {
            if let Ok(ranges) = h.highlight_line(line, &self.ss) {
                out.push_str(&as_24_bit_terminal_escaped(&ranges, false));
            }
        }
        out.push_str("\x1b[0m");
        out
    }
}

pub enum Part<'a> {
    Text(&'a str),
    Code { lang: &'a str, code: &'a str },
}

pub fn split_code_blocks(text: &str) -> Vec<Part<'_>> {
    let mut parts = Vec::new();
    let mut pos = 0;

    while pos < text.len() {
        match text[pos..].find("```") {
            None => {
                parts.push(Part::Text(&text[pos..]));
                break;
            }
            Some(rel) => {
                let fence = pos + rel;
                if fence > pos {
                    parts.push(Part::Text(&text[pos..fence]));
                }
                let after = fence + 3;
                let nl = text[after..].find('\n').map(|i| after + i).unwrap_or(text.len());
                let lang = text[after..nl].trim();
                let code_start = if nl < text.len() { nl + 1 } else { text.len() };
                match text[code_start..].find("```") {
                    Some(rel_close) => {
                        let close = code_start + rel_close;
                        parts.push(Part::Code { lang, code: &text[code_start..close] });
                        pos = close + 3;
                        if pos < text.len() && text.as_bytes()[pos] == b'\n' {
                            pos += 1;
                        }
                    }
                    None => {
                        parts.push(Part::Code { lang, code: &text[code_start..] });
                        pos = text.len();
                    }
                }
            }
        }
    }

    parts
}

pub fn render_response(text: &str, skin: &MadSkin, hl: &CodeHighlighter) {
    for part in split_code_blocks(text) {
        match part {
            Part::Text(t) => {
                if !t.trim().is_empty() {
                    skin.print_text(t);
                }
            }
            Part::Code { lang, code } => {
                if !code.trim().is_empty() {
                    print!("{}", hl.highlight(code, lang));
                }
            }
        }
    }
}
