use crate::models::ModelRegistry;
use colored::*;
use dialoguer::{Input, Select, theme::ColorfulTheme};

pub fn run_theme() -> anyhow::Result<()> {
    println!("{}", "=== Aide: Themes ===".bold().cyan());

    let registry = ModelRegistry::new();
    let config = registry.load_config()?;
    let active = config
        .active_theme
        .clone()
        .unwrap_or_else(|| "gruvbox".to_string());
    let all = crate::theme::all_themes(&config.custom_themes);

    // ── List all themes ──────────────────────────────────────────────────────
    println!();
    let builtin_names: Vec<_> = crate::theme::builtin_themes()
        .into_iter()
        .map(|t| t.name)
        .collect();
    for t in &all {
        let kind = if builtin_names.contains(&t.name) { "built-in" } else { "custom" };
        let marker = if t.name == active { " ← active".green().bold().to_string() } else { String::new() };
        println!("  {:<14}  [{}]{}", t.name.bold(), kind, marker);
    }
    println!();

    // ── Top-level menu ───────────────────────────────────────────────────────
    let menu = &["Switch theme", "Create new theme", "Cancel"];
    let choice = match Select::with_theme(&ColorfulTheme::default())
        .with_prompt("What would you like to do?")
        .items(menu)
        .default(0)
        .interact_opt()?
    {
        Some(i) => i,
        None => return Ok(()),
    };

    match choice {
        0 => theme_switch(&registry, &config, &all, &active)?,
        1 => theme_create(&registry, &config)?,
        _ => {}
    }
    Ok(())
}

fn theme_switch(
    registry: &ModelRegistry,
    config: &crate::models::Config,
    all: &[crate::theme::Theme],
    active: &str,
) -> anyhow::Result<()> {
    let builtin_names: Vec<_> = crate::theme::builtin_themes()
        .into_iter()
        .map(|t| t.name)
        .collect();
    let items: Vec<String> = all
        .iter()
        .map(|t| {
            let kind = if builtin_names.contains(&t.name) { "built-in" } else { "custom" };
            let marker = if t.name == active { "  ← active" } else { "" };
            format!("{:<14}  [{}]{}", t.name, kind, marker)
        })
        .collect();

    let default_idx = all.iter().position(|t| t.name == active).unwrap_or(0);
    let idx = match Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select theme")
        .items(&items)
        .default(default_idx)
        .interact_opt()?
    {
        Some(i) => i,
        None => return Ok(()),
    };

    let selected = &all[idx];
    let mut new_config = config.clone();
    new_config.active_theme = Some(selected.name.clone());
    registry.save_config(&new_config)?;
    println!("\nSwitched to {} theme.", selected.name.green().bold());
    Ok(())
}

fn theme_create(
    registry: &ModelRegistry,
    config: &crate::models::Config,
) -> anyhow::Result<()> {
    println!("{}", "\nCreate New Theme".bold());
    println!("Enter hex colors in {} format. Press Enter to keep the default.\n", "#rrggbb".cyan());

    let validate_hex = |s: &String| -> Result<(), String> {
        if crate::theme::is_valid_hex(s) {
            Ok(())
        } else {
            Err(format!("Expected #rrggbb format, got '{}'", s))
        }
    };

    let builtin_names: Vec<_> = crate::theme::builtin_themes()
        .into_iter()
        .map(|t| t.name)
        .collect();

    let name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Theme name")
        .validate_with(|s: &String| -> Result<(), String> {
            if s.trim().is_empty() {
                return Err("Name cannot be empty.".to_string());
            }
            if builtin_names.iter().any(|n| n == s.trim()) {
                return Err(format!("'{}' is a built-in theme and cannot be overridden.", s.trim()));
            }
            Ok(())
        })
        .interact_text()?;

    println!("\n{}", "── Text ──".dimmed());
    let fg: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Text color (fg)")
        .default("#ebdbb2".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;

    println!("\n{}", "── Headings ──".dimmed());
    let h1: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("H1 color")
        .default("#fe8019".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;
    let headers: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("H2–H6 color")
        .default("#fabd2f".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;

    println!("\n{}", "── Emphasis ──".dimmed());
    let bold: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Bold color")
        .default("#fe8019".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;
    let italic: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Italic color")
        .default("#b8bb26".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;

    println!("\n{}", "── Code ──".dimmed());
    let code_fg: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Code text color")
        .default("#8ec07c".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;
    let code_bg: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Code background")
        .default("#3c3836".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;
    let bullet: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Bullet color")
        .default("#fabd2f".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;

    println!("\n{}", "── Chat bubble ──".dimmed());
    let user_bg: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Your message background")
        .default("#076678".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;
    let user_fg: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Your message text color")
        .default("#ebdbb2".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;

    println!("\n{}", "── Syntax highlighting ──".dimmed());
    let syntax_options = [
        "base16-ocean.dark",
        "base16-eighties.dark",
        "base16-mocha.dark",
        "Solarized (dark)",
        "InspiredGitHub",
    ];
    let syntax_idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Code syntax theme")
        .items(&syntax_options)
        .default(0)
        .interact()?;

    let theme = crate::theme::Theme {
        name: name.trim().to_string(),
        fg,
        h1,
        headers,
        bold,
        italic,
        code_fg,
        code_bg,
        bullet,
        user_bg,
        user_fg,
        syntax_theme: syntax_options[syntax_idx].to_string(),
    };

    let mut new_config = config.clone();
    // Replace if a custom theme with the same name already exists
    new_config.custom_themes.retain(|t| t.name != theme.name);
    new_config.custom_themes.push(theme.clone());
    new_config.active_theme = Some(theme.name.clone());
    registry.save_config(&new_config)?;

    println!("\nTheme '{}' created and activated!", theme.name.green().bold());
    Ok(())
}
