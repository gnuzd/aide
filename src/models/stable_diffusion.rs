use diffusion_rs::api::gen_img;
use diffusion_rs::preset::{Preset, PresetBuilder};
use std::path::{Path, PathBuf};
use anyhow::Result;
use colored::*;

pub struct StableDiffusionEngine {
    pub model_path: String,
}

impl StableDiffusionEngine {
    pub fn new(model_path: &Path) -> Self {
        Self {
            model_path: model_path.to_string_lossy().to_string(),
        }
    }

    pub fn generate(&self, prompt: &str, output_path: &Path) -> Result<()> {
        let m_path = PathBuf::from(&self.model_path);
        let abs_m_path = if m_path.is_absolute() { m_path } else { std::env::current_dir()?.join(m_path) };
        println!("{} {:?}", "DEBUG: Using absolute model path:".dimmed(), abs_m_path);
        
        if !abs_m_path.exists() {
            return Err(anyhow::anyhow!("Model file not found at {:?}", abs_m_path));
        }

        let clean_prompt = prompt.replace('\n', " ").replace('\r', " ").trim().to_string();
        println!("{} \"{}\"", "DEBUG: Building config for prompt:".dimmed(), clean_prompt);

        // Ensure output_path is absolute for the engine
        let abs_output = if output_path.is_absolute() { 
            output_path.to_path_buf() 
        } else { 
            std::env::current_dir()?.join(output_path) 
        };

        let abs_output_for_engine = abs_output.clone();
        let (config, mut model_config) = PresetBuilder::default()
            .preset(Preset::StableDiffusion1_5)
            .prompt(clean_prompt)
            .with_modifier(move |(mut c_b, mut m_b)| {
                 m_b.model(abs_m_path);
                 m_b.n_threads(4); // Safer thread count for M1 stability
                 m_b.offload_params_to_cpu(true); 
                 m_b.vae_tiling(false);
                 m_b.enable_mmap(true);
                 m_b.flash_attention(false); // Explicitly disable
                 m_b.diffusion_flash_attention(false);
                 
                 c_b.output(abs_output_for_engine);
                 c_b.width(512); 
                 c_b.height(512);
                 c_b.steps(4); 
                 
                 Ok((c_b, m_b))
            })
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build SD config: {}", e))?;


        println!("{} {:?}", "DEBUG: Engine is configured to write to:".dimmed(), abs_output);
        println!("{}", "DEBUG: Starting image generation (FFI call)...".dimmed());
        
        // FFI call
        gen_img(&config, &mut model_config).map_err(|e| anyhow::anyhow!("Generation failed: {}", e))?;
        println!("{}", "DEBUG: Generation successful.".dimmed());

        if abs_output.exists() {
            println!("{} {}", "DEBUG: Output exists at:".dimmed(), abs_output.display());
        } else {
            println!("{}", "DEBUG: Output file was not created by the engine.".yellow());
        }

        Ok(())
    }
}
