use serde::{Serialize, Deserialize};
use crate::system::SystemSpecs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ModelType {
    General,
    Coding,
    Design,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Model {
    pub name: String,
    pub description: String,
    pub model_type: ModelType,
    pub min_ram_gb: u64,
    pub huggingface_url: String,
}

pub struct ModelRegistry {
    pub models: Vec<Model>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        ModelRegistry {
            models: vec![
                Model {
                    name: "Llama 3 (8B)".to_string(),
                    description: "High-performance general-purpose model.".to_string(),
                    model_type: ModelType::General,
                    min_ram_gb: 8,
                    huggingface_url: "https://huggingface.co/lmstudio-community/Meta-Llama-3-8B-Instruct-GGUF".to_string(),
                },
                Model {
                    name: "Phi-3 Mini".to_string(),
                    description: "Extremely lightweight and fast model by Microsoft.".to_string(),
                    model_type: ModelType::General,
                    min_ram_gb: 4,
                    huggingface_url: "https://huggingface.co/microsoft/Phi-3-mini-4k-instruct".to_string(),
                },
                Model {
                    name: "DeepSeek Coder (6.7B)".to_string(),
                    description: "Specialized model for coding and refactoring tasks.".to_string(),
                    model_type: ModelType::Coding,
                    min_ram_gb: 8,
                    huggingface_url: "https://huggingface.co/deepseek-ai/deepseek-coder-6.7b-instruct".to_string(),
                },
            ],
        }
    }

    /// Get all models that meet the system RAM requirements
    pub fn get_compatible_models(&self, specs: &SystemSpecs) -> Vec<&Model> {
        self.models
            .iter()
            .filter(|m| m.min_ram_gb <= specs.total_memory_gb)
            .collect()
    }

    /// Recommend the best main model based on hardware specs
    pub fn recommend_main_model(&self, specs: &SystemSpecs) -> Option<&Model> {
        // Find general models that fit in RAM
        self.models
            .iter()
            .filter(|m| matches!(m.model_type, ModelType::General))
            .filter(|m| m.min_ram_gb <= specs.total_memory_gb)
            .max_by_key(|m| m.min_ram_gb) // Pick the largest capable model
    }
}
