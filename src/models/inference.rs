use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaModel, AddBos};
use llama_cpp_2::token::LlamaToken;
use std::path::Path;
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
        // Cấu hình GPU Metal cho M1 (Llama 3 8B có 33 layers)
        model_params = model_params.with_n_gpu_layers(33); 
        
        let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
            .context("Failed to load model file")?;

        Ok(Self { model, backend, template_type })
    }

    pub fn format_prompt(&self, user_input: &str) -> String {
        match self.template_type.as_str() {
            "phi3" => format!(
                "<|system|>\nYou are Aide, a helpful assistant.<|end|>\n<|user|>\n{}<|end|>\n<|assistant|>\n",
                user_input
            ),
            "deepseek" => format!(
                "### System:\nYou are Aide, a helpful assistant.\n### Human:\n{}\n### Assistant:\n",
                user_input
            ),
            "chatml" => format!(
                "<|im_start|>system\nYou are Aide, a helpful assistant.<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
                user_input
            ),
            // "llama3" and default
            _ => format!(
                "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\nYou are Aide, a helpful assistant.<|eot_id|><|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
                user_input
            ),
        }
    }

    pub fn generate(&self, user_input: &str, max_tokens: i32) -> anyhow::Result<String> {
        let mut ctx_params = LlamaContextParams::default();
        // Đặt n_ctx 2048 là mức an toàn cho M1
        ctx_params = ctx_params.with_n_ctx(Some(NonZeroU32::new(2048).unwrap()));
        ctx_params = ctx_params.with_n_batch(512);
        ctx_params = ctx_params.with_n_threads(8);

        let mut ctx = self.model.new_context(&self.backend, ctx_params)
            .context("Failed to create context")?;

        let prompt = self.format_prompt(user_input);
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

            // Dừng nếu gặp token kết thúc
            if self.model.is_eog_token(token) {
                break;
            }

            let piece = self.model.token_to_piece(token, &mut decoder, false, None)?;
            
            print!("{}", piece);
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
