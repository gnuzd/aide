use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_sys_2;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaModel, AddBos};
use llama_cpp_2::token::LlamaToken;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Context;
use std::num::NonZeroU32;
use encoding_rs;

pub struct InferenceEngine {
    pub model: LlamaModel,
    pub backend: LlamaBackend,
    pub template_type: String,
}

impl InferenceEngine {
    pub fn new(model_path: &Path, template_type: String) -> anyhow::Result<Self> {
        let mut backend = LlamaBackend::init()?;
        backend.void_logs(); 

        let mut model_params = LlamaModelParams::default();
        // i32::MAX để đảm bảo toàn bộ layers đều offload lên Metal GPU
        model_params = model_params.with_n_gpu_layers(u32::MAX);
        
        let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
            .context("Failed to load model file")?;

        Ok(Self { model, backend, template_type })
    }

    pub fn format_prompt(&self, user_input: &str, system_prompt: &str) -> String {
        match self.template_type.as_str() {
            "phi3" => format!(
                "<|system|>\n{}<|end|>\n<|user|>\n{}<|end|>\n<|assistant|>\n",
                system_prompt, user_input
            ),
            "deepseek" => format!(
                "### System:\n{}\n### Human:\n{}\n### Assistant:\n",
                system_prompt, user_input
            ),
            "chatml" => format!(
                "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
                system_prompt, user_input
            ),
            // "llama3" and default
            _ => format!(
                "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
                system_prompt, user_input
            ),
        }
    }

    pub fn generate(&self, user_input: &str, max_tokens: i32, system_prompt: &str, stop: &AtomicBool) -> anyhow::Result<String> {
        let mut ctx_params = LlamaContextParams::default();
        ctx_params = ctx_params.with_n_ctx(Some(NonZeroU32::new(2048).unwrap()));
        ctx_params = ctx_params.with_n_batch(2048);
        ctx_params = ctx_params.with_n_threads(8);
        ctx_params = ctx_params.with_n_threads_batch(8);
        // Flash Attention tăng tốc đáng kể trên Apple Silicon Metal
        ctx_params = ctx_params.with_flash_attention_policy(
            llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_ENABLED
        );

        let mut ctx = self.model.new_context(&self.backend, ctx_params)
            .context("Failed to create context")?;

        let prompt = self.format_prompt(user_input, system_prompt);
        // Quan trọng: Sử dụng AddBos::Never vì template đã có <|begin_of_text|>
        let tokens = self.model.str_to_token(&prompt, AddBos::Never)?;

        let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(2048, 1);
        for (i, token) in tokens.iter().enumerate() {
            let _ = batch.add(*token, i as i32, &[0], i == tokens.len() - 1);
        }

        ctx.decode(&mut batch).map_err(|e| anyhow::anyhow!("Decode prompt failed: {}", e))?;
        
        let mut response = String::new();
        let mut n_cur = batch.n_tokens();
        let mut decoder = encoding_rs::UTF_8.new_decoder();

        for _ in 0..max_tokens {
            let logits = ctx.get_logits_ith(batch.n_tokens() - 1);
            let token = LlamaToken::new(
                logits.iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.total_cmp(b))
                    .map(|(i, _)| i as i32)
                    .unwrap_or(0),
            );

            // Stop on end-of-generation token or ESC signal
            if self.model.is_eog_token(token) || stop.load(Ordering::Relaxed) {
                break;
            }

            let piece = self.model.token_to_piece(token, &mut decoder, false, None)?;

            // Use \r\n so newlines render correctly in raw mode (active during generation)
            print!("{}", piece.replace('\n', "\r\n"));
            std::io::Write::flush(&mut std::io::stdout())?;
            response.push_str(&piece);

            batch.clear();
            let _ = batch.add(token, n_cur, &[0], true);
            ctx.decode(&mut batch).map_err(|e| anyhow::anyhow!("Decode token failed: {}", e))?;
            n_cur += 1;

            if n_cur >= 2048 { break; }
        }

        if response.is_empty() {
            return Ok("The model did not generate any text. Please check the model files.".to_string());
        }

        Ok(response)
    }
}
