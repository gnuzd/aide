pub mod models;
pub mod system;
pub mod memory;

use crate::models::{ModelRegistry, Config, ModelType};
use crate::system::SystemSpecs;
use crate::models::inference::InferenceEngine;
use crate::memory::{MemoryStore, generate_session_id};

pub struct Aide {
    pub registry: ModelRegistry,
    pub config: Config,
    pub memory: MemoryStore,
}

impl Aide {
    pub fn new() -> anyhow::Result<Self> {
        let registry = ModelRegistry::new();
        let config = registry.load_config().unwrap_or_default();
        let memory = MemoryStore::init_db(&registry.base_path)?;
        Ok(Self { registry, config, memory })
    }

    pub fn generate_session_id(&self) -> String {
        generate_session_id()
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        let mut model_needed = true;

        if let Some(ref path) = self.config.active_model_path {
            if path.exists() {
                model_needed = false;
            } else {
                println!("Configured model path does not exist: {:?}", path);
            }
        }

        if model_needed {
            println!("No active model found. Setting up your AI assistant...");
            
            let specs = SystemSpecs::audit();
            println!("System Audit:");
            println!("- OS: {} {}", specs.os_name, specs.os_version);
            println!("- Memory: {} GB", specs.total_memory_gb);
            println!("- CPU: {} ({} cores)", specs.cpu_brand, specs.cpu_cores);

            let (compatible, warnings) = specs.check_compatibility();
            for warning in warnings {
                println!("Warning: {}", warning);
            }

            if !compatible {
                println!("Error: Your system does not meet the minimum requirements (4GB RAM) for local AI.");
                return Err(anyhow::anyhow!("System incompatible"));
            }

            // Check if there are any models already downloaded that we could use
            let models_dir = self.registry.base_path.join("models");
            if models_dir.exists() {
                for model in &self.registry.models {
                    let p = models_dir.join(&model.filename);
                    if p.exists() {
                        println!("Found existing model: {}", model.name);
                        self.config.active_model_path = Some(p);
                        self.config.active_model_template = Some(model.template_type.clone());
                        self.registry.save_config(&self.config)?;
                        return Ok(());
                    }
                }
            }

            // Recommend a model
            let models = self.registry.get_compatible_models(&specs, ModelType::General);
            if models.is_empty() {
                return Err(anyhow::anyhow!("No compatible models found in registry."));
            }

            // Pick the best quality model that fits in RAM
            let recommended = models.iter()
                .max_by_key(|m| m.quality_score)
                .ok_or_else(|| anyhow::anyhow!("At least one model should be available"))?;

            println!("\nRecommended model for your system: {}", recommended.name);
            println!("Description: {}", recommended.description);
            
            let path = self.registry.download_model(recommended).await?;
            self.config.active_model_path = Some(path);
            self.config.active_model_template = Some(recommended.template_type.clone());
            self.registry.save_config(&self.config)?;
            
            println!("Model setup complete!");
        }

        Ok(())
    }

    pub fn create_inference_engine(&self) -> anyhow::Result<InferenceEngine> {
        let path = self.config.active_model_path.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active model path configured. Please run init first."))?;
        let template = self.config.active_model_template.clone()
            .unwrap_or_else(|| "llama3".to_string());
            
        InferenceEngine::new(path, template)
    }
}
