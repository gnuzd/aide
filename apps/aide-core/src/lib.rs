pub mod models;
pub mod system;
pub mod memory;

use crate::models::{ModelRegistry, Config, ModelType};
use crate::system::SystemSpecs;
use crate::models::inference::InferenceEngine;
use crate::memory::{MemoryStore, generate_session_id};
use std::io::Write;

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

    pub fn reset(&mut self) -> anyhow::Result<()> {
        self.clear_conversations()?;
        self.clear_profile()?;
        self.clear_models()?;
        self.clear_config()?;
        Ok(())
    }

    pub fn clear_conversations(&mut self) -> anyhow::Result<()> {
        self.memory.clear_conversations()
    }

    pub fn clear_profile(&mut self) -> anyhow::Result<()> {
        self.memory.clear_profile()
    }

    pub fn clear_models(&mut self) -> anyhow::Result<()> {
        let models_dir = self.registry.base_path.join("models");
        if models_dir.exists() {
            std::fs::remove_dir_all(&models_dir)?;
            std::fs::create_dir_all(&models_dir)?;
        }
        self.config.active_model_path = None;
        self.config.active_model_template = None;
        self.registry.save_config(&self.config)?;
        Ok(())
    }

    pub fn clear_config(&mut self) -> anyhow::Result<()> {
        self.config = Config::default();
        let config_path = self.registry.get_config_path();
        if config_path.exists() {
            std::fs::remove_file(config_path)?;
        }
        Ok(())
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        let mut model_needed = true;

        if let Some(ref path) = self.config.active_model_path {
            if path.exists() {
                // Try a "test load" to ensure it's not corrupted/incompatible
                let template = self.config.active_model_template.clone().unwrap_or_default();
                match InferenceEngine::new(path, template) {
                    Ok(_) => {
                        model_needed = false;
                    }
                    Err(e) => {
                        println!("Failed to load existing model: {}. It might be corrupted or incompatible.", e);
                        self.config.active_model_path = None;
                        let _ = self.registry.save_config(&self.config);
                    }
                }
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
                        println!("Found existing model file: {}. Verifying...", model.name);
                        if InferenceEngine::new(&p, model.template_type.clone()).is_ok() {
                            println!("Model verified.");
                            self.config.active_model_path = Some(p);
                            self.config.active_model_template = Some(model.template_type.clone());
                            self.registry.save_config(&self.config)?;
                            return Ok(());
                        } else {
                            println!("Existing model file {} is invalid or incompatible. Skipping.", model.name);
                        }
                    }
                }
            }

            // List compatible models
            let models = self.registry.get_compatible_models(&specs, ModelType::General);
            if models.is_empty() {
                return Err(anyhow::anyhow!("No compatible models found in registry."));
            }

            println!("\nAvailable models for your system:");
            for (i, model) in models.iter().enumerate() {
                println!("{}. {} ({})", i + 1, model.name, model.description);
                println!("   Size: {} GB, Quality: {}/10", model.size_gb, model.quality_score);
            }

            let selected_model = loop {
                print!("\nSelect a model to download (1-{}): ", models.len());
                std::io::stdout().flush()?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if let Ok(choice) = input.trim().parse::<usize>() {
                    if choice > 0 && choice <= models.len() {
                        break models[choice - 1];
                    }
                }
                println!("Invalid selection, please try again.");
            };

            println!("\nSelected: {}", selected_model.name);
            
            let path = self.registry.download_model(selected_model).await?;
            self.config.active_model_path = Some(path);
            self.config.active_model_template = Some(selected_model.template_type.clone());
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
