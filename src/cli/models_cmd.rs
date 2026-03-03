use crate::models::ModelRegistry;
use crate::system::SystemSpecs;

pub fn show_system_info() {
    let specs = SystemSpecs::audit();
    println!("{:#?}", specs);
}

pub async fn list_models() -> anyhow::Result<()> {
    let registry = ModelRegistry::new();
    println!("{:<20} | {:<10} | {:<8}", "Name", "Type", "Min RAM");
    println!("{}", "-".repeat(45));
    for model in &registry.models {
        println!(
            "{:<20} | {:<10?} | {:<8} GB",
            model.name, model.model_type, model.min_ram_gb
        );
    }
    Ok(())
}
