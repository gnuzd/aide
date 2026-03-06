pub mod inference;

use crate::system::SystemSpecs;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
    pub size_gb: f64,      // approximate download size in GB
    pub quality_score: u8, // 1–10; used for ranking / recommendation
    pub huggingface_url: String,
    pub filename: String,
    pub template_type: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Config {
    pub active_model_path: Option<PathBuf>,
    pub active_model_template: Option<String>,
    pub active_design_model_path: Option<PathBuf>,
    pub active_theme: Option<String>,
    pub custom_themes: Vec<serde_json::Value>, // Store as Value to avoid circular dependency with aide-cli::theme
}

pub struct ModelRegistry {
    pub models: Vec<Model>,
    pub base_path: PathBuf,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let mut home = dirs::home_dir().expect("Could not find home directory");
        home.push(".aide");
        let base_path = home.clone();
        home.push("models");

        if !home.exists() {
            fs::create_dir_all(&home).expect("Could not create models directory");
        }

        ModelRegistry {
            models: vec![
                // ── General models (sorted by quality asc in source; displayed desc) ──
                Model {
                    name: "Phi-3 Mini (3.8B)".to_string(),
                    description: "Microsoft's compact 3.8B model. Punches above its weight; ideal for 4 GB machines.".to_string(),
                    model_type: ModelType::General,
                    min_ram_gb: 4,
                    size_gb: 2.2,
                    quality_score: 5,
                    huggingface_url: "https://huggingface.co/microsoft/Phi-3-mini-4k-instruct-gguf/resolve/main/Phi-3-mini-4k-instruct-q4.gguf".to_string(),
                    filename: "phi-3-mini.gguf".to_string(),
                    template_type: "phi3".to_string(),
                },
                Model {
                    name: "Llama 3.2 (3B)".to_string(),
                    description: "Meta's lightweight 3B model. Fast and capable; great for low-memory setups.".to_string(),
                    model_type: ModelType::General,
                    min_ram_gb: 4,
                    size_gb: 2.0,
                    quality_score: 6,
                    huggingface_url: "https://huggingface.co/lmstudio-community/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf".to_string(),
                    filename: "llama-3.2-3b.gguf".to_string(),
                    template_type: "llama3".to_string(),
                },
                Model {
                    name: "Mistral 7B v0.3".to_string(),
                    description: "Mistral's flagship 7B model. Excellent instruction following and reasoning.".to_string(),
                    model_type: ModelType::General,
                    min_ram_gb: 8,
                    size_gb: 4.4,
                    quality_score: 7,
                    huggingface_url: "https://huggingface.co/lmstudio-community/Mistral-7B-Instruct-v0.3-GGUF/resolve/main/Mistral-7B-Instruct-v0.3-Q4_K_M.gguf".to_string(),
                    filename: "mistral-7b-v0.3.gguf".to_string(),
                    template_type: "mistral".to_string(),
                },
                Model {
                    name: "Llama 3 (8B)".to_string(),
                    description: "Meta's flagship 8B model. Best general-purpose performance under 8 GB RAM.".to_string(),
                    model_type: ModelType::General,
                    min_ram_gb: 8,
                    size_gb: 4.7,
                    quality_score: 8,
                    huggingface_url: "https://huggingface.co/lmstudio-community/Meta-Llama-3-8B-Instruct-GGUF/resolve/main/Meta-Llama-3-8B-Instruct-Q4_K_M.gguf".to_string(),
                    filename: "llama-3-8b.gguf".to_string(),
                    template_type: "llama3".to_string(),
                },
                Model {
                    name: "Gemma 2 (9B)".to_string(),
                    description: "Google's 9B model. Outperforms most 7-8B models. Best-in-class for 12+ GB.".to_string(),
                    model_type: ModelType::General,
                    min_ram_gb: 12,
                    size_gb: 5.5,
                    quality_score: 9,
                    huggingface_url: "https://huggingface.co/bartowski/gemma-2-9b-it-GGUF/resolve/main/gemma-2-9b-it-Q4_K_M.gguf".to_string(),
                    filename: "gemma-2-9b.gguf".to_string(),
                    template_type: "gemma".to_string(),
                },
                // ── Coding models ──
                Model {
                    name: "DeepSeek Coder 1.3B".to_string(),
                    description: "Ultra-compact coding model. Surprisingly capable for simple tasks under 4 GB.".to_string(),
                    model_type: ModelType::Coding,
                    min_ram_gb: 4,
                    size_gb: 0.8,
                    quality_score: 4,
                    huggingface_url: "https://huggingface.co/TheBloke/deepseek-coder-1.3b-instruct-GGUF/resolve/main/deepseek-coder-1.3b-instruct.Q4_K_M.gguf".to_string(),
                    filename: "deepseek-coder-1.3b.gguf".to_string(),
                    template_type: "deepseek".to_string(),
                },
                Model {
                    name: "DeepSeek Coder 6.7B".to_string(),
                    description: "Solid coding model for 8 GB machines. Strong across multiple languages.".to_string(),
                    model_type: ModelType::Coding,
                    min_ram_gb: 8,
                    size_gb: 3.8,
                    quality_score: 7,
                    huggingface_url: "https://huggingface.co/TheBloke/deepseek-coder-6.7B-instruct-GGUF/resolve/main/deepseek-coder-6.7b-instruct.Q4_K_M.gguf".to_string(),
                    filename: "deepseek-coder-6.7b.gguf".to_string(),
                    template_type: "deepseek".to_string(),
                },
                Model {
                    name: "Qwen2.5 Coder 7B".to_string(),
                    description: "State-of-the-art 7B coding model. Beats much larger models on benchmarks.".to_string(),
                    model_type: ModelType::Coding,
                    min_ram_gb: 8,
                    size_gb: 4.4,
                    quality_score: 9,
                    huggingface_url: "https://huggingface.co/bartowski/Qwen2.5-Coder-7B-Instruct-GGUF/resolve/main/Qwen2.5-Coder-7B-Instruct-Q4_K_M.gguf".to_string(),
                    filename: "qwen2.5-coder-7b.gguf".to_string(),
                    template_type: "chatml".to_string(),
                },
                Model {
                    name: "DeepSeek Coder 33B".to_string(),
                    description: "DeepSeek's flagship 33B coding model. Near-GPT-4 coding quality.".to_string(),
                    model_type: ModelType::Coding,
                    min_ram_gb: 32,
                    size_gb: 18.9,
                    quality_score: 10,
                    huggingface_url: "https://huggingface.co/TheBloke/deepseek-coder-33B-instruct-GGUF/resolve/main/deepseek-coder-33b-instruct.Q4_K_M.gguf".to_string(),
                    filename: "deepseek-coder-33b.gguf".to_string(),
                    template_type: "deepseek".to_string(),
                },
                // ── Design models ──
                Model {
                    name: "Stable Diffusion v1.5 (GGUF)".to_string(),
                    description: "Classic image generation model. Fast and reliable; ideal for most machines.".to_string(),
                    model_type: ModelType::Design,
                    min_ram_gb: 8,
                    size_gb: 2.1,
                    quality_score: 7,
                    huggingface_url: "https://huggingface.co/lmstudio-community/sd1.5-v-q4_0.gguf/resolve/main/sd1.5-v-q4_0.gguf".to_string(),
                    filename: "sd-v1.5-q4_0.gguf".to_string(),
                    template_type: "sd".to_string(),
                },

            ],
            base_path,
        }
    }

    pub fn get_config_path(&self) -> PathBuf {
        self.base_path.join("config.json")
    }

    pub fn save_config(&self, config: &Config) -> anyhow::Result<()> {
        let path = self.get_config_path();
        let json = serde_json::to_string_pretty(config)?;
        let mut file = fs::File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn load_config(&self) -> anyhow::Result<Config> {
        let path = self.get_config_path();
        if !path.exists() {
            return Ok(Config::default());
        }
        let mut file = fs::File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let config = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn get_compatible_models(&self, specs: &SystemSpecs, model_type: ModelType) -> Vec<&Model> {
        self.models
            .iter()
            .filter(|m| m.model_type == model_type && m.min_ram_gb <= specs.total_memory_gb)
            .collect()
    }

    pub async fn download_model(&self, model: &Model) -> anyhow::Result<PathBuf> {
        let dest_path = self.base_path.join("models").join(&model.filename);
        if dest_path.exists() {
            return Ok(dest_path);
        }

        println!("Downloading model: {}", model.name);
        let client = Client::new();
        let res = client.get(&model.huggingface_url).send().await?;
        let total_size = res.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
            .progress_chars("#>-"));

        let mut file = fs::File::create(&dest_path)?;
        let mut downloaded: u64 = 0;
        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            let chunk = item?;
            file.write_all(&chunk)?;
            downloaded = std::cmp::min(downloaded + (chunk.len() as u64), total_size);
            pb.set_position(downloaded);
        }
        pb.finish_with_message("Download complete");
        Ok(dest_path)
    }
}
