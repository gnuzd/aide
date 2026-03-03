use crate::cli::run_chat_loop;
use crate::system::SystemSpecs;
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};

pub async fn run_setup() -> anyhow::Result<()> {
    println!("{}", "=== Aide Setup Wizard ===".bold().cyan());

    let specs = SystemSpecs::audit();
    println!("\n{}", "1. Auditing Hardware...".bold());
    println!("- OS: {} {}", specs.os_name, specs.os_version);
    println!(
        "- CPU: {} ({} cores / {} threads)",
        specs.cpu_brand, specs.cpu_cores, specs.cpu_threads
    );
    println!(
        "- RAM: {} GB total ({} GB available)",
        specs.total_memory_gb, specs.available_memory_gb
    );

    let (compatible, warnings) = specs.check_compatibility();
    if !compatible {
        println!(
            "\n{}",
            "⚠️  Minimum hardware requirements not met!".red().bold()
        );
    }

    for warning in warnings {
        println!("  - {}", warning.yellow());
    }

    println!("\n{}", "2. Select Main Model...".bold());
    let registry = crate::models::ModelRegistry::new();
    let compatible_llms = registry.get_compatible_models(&specs, crate::models::ModelType::General);
    let mut config = registry.load_config()?;

    if compatible_llms.is_empty() {
        println!("{}", "No suitable local models found for your hardware.".red());
    } else {
        let items: Vec<String> = compatible_llms.iter().map(|m| format!("{} - {} ({} GB RAM min)", m.name, m.description, m.min_ram_gb)).collect();
        let selection = Select::with_theme(&ColorfulTheme::default()).with_prompt("Pick an LLM model to download").items(&items).default(0).interact().unwrap();
        let selected_model = compatible_llms[selection];
        println!("\nYou selected: {}", selected_model.name.green().bold());
        let model_path = registry.download_model(selected_model).await?;
        config.active_model_path = Some(model_path);
        config.active_model_template = Some(selected_model.template_type.clone());
        registry.save_config(&config)?;
    }

    println!("\n{}", "3. Select Design Model (Optional)...".bold());
    let compatible_design = registry.get_compatible_models(&specs, crate::models::ModelType::Design);
    if !compatible_design.is_empty() {
        let mut design_items: Vec<String> = compatible_design.iter().map(|m| format!("{} - {} ({} GB RAM min)", m.name, m.description, m.min_ram_gb)).collect();
        design_items.push("Skip for now".to_string());

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Pick a Stable Diffusion model for image generation")
            .items(&design_items)
            .default(0)
            .interact()
            .unwrap();

        if selection < compatible_design.len() {
            let selected_design = compatible_design[selection];
            println!("\nYou selected: {}", selected_design.name.green().bold());
            let design_path = registry.download_model(selected_design).await?;
            config.active_design_model_path = Some(design_path);
            registry.save_config(&config)?;
        }
    }

    println!("\n{}", "Setup complete! Starting chat mode...".green());
    let _ = run_chat_loop();
    Ok(())
}
