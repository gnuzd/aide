pub mod inference;

use serde::{Serialize, Deserialize};
use crate::system::SystemSpecs;
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{Write, Read};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;

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
    pub filename: String,
    pub template_type: String, // "llama3", "phi3", "chatml"
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub active_model_path: Option<PathBuf>,
    pub active_model_template: Option<String>,
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
                Model {
                    name: "Llama 3 (8B)".to_string(),
                    description: "High-performance general-purpose model.".to_string(),
                    model_type: ModelType::General,
                    min_ram_gb: 8,
                    huggingface_url: "https://huggingface.co/lmstudio-community/Meta-Llama-3-8B-Instruct-GGUF/resolve/main/Meta-Llama-3-8B-Instruct-Q4_K_M.gguf".to_string(),
                    filename: "llama-3-8b.gguf".to_string(),
                    template_type: "llama3".to_string(),
                },
                Model {
                    name: "Phi-3 Mini".to_string(),
                    description: "Extremely lightweight and fast model by Microsoft.".to_string(),
                    model_type: ModelType::General,
                    min_ram_gb: 4,
                    huggingface_url: "https://huggingface.co/microsoft/Phi-3-mini-4k-instruct-gguf/resolve/main/Phi-3-mini-4k-instruct-q4.gguf".to_string(),
                    filename: "phi-3-mini.gguf".to_string(),
                    template_type: "phi3".to_string(),
                },
                Model {
                    name: "DeepSeek Coder (6.7B)".to_string(),
                    description: "Specialized model for coding tasks.".to_string(),
                    model_type: ModelType::Coding,
                    min_ram_gb: 8,
                    huggingface_url: "https://huggingface.co/TheBloke/deepseek-coder-6.7B-instruct-GGUF/resolve/main/deepseek-coder-6.7b-instruct.Q4_K_M.gguf".to_string(),
                    filename: "deepseek-coder-6.7b.gguf".to_string(),
                    template_type: "deepseek".to_string(),
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

    pub fn get_compatible_models(&self, specs: &SystemSpecs) -> Vec<&Model> {
        self.models
            .iter()
            .filter(|m| m.min_ram_gb <= specs.total_memory_gb)
            .collect()
    }

    pub async fn download_model(&self, model: &Model) -> anyhow::Result<PathBuf> {
        let dest_path = self.base_path.join("models").join(&model.filename);
        if dest_path.exists() { return Ok(dest_path); }

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
