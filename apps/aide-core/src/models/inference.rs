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
        self.format_chat_prompt(&[], user_input, system_prompt)
    }

    pub fn format_chat_prompt(&self, history: &[String], user_input: &str, system_prompt: &str) -> String {
        let mut prompt = String::new();
        
        match self.template_type.as_str() {
            "phi3" => {
                prompt.push_str(&format!("<|system|>\n{}<|end|>\n", system_prompt));
                for msg in history {
                    if msg.starts_with("User: ") {
                        prompt.push_str(&format!("<|user|>\n{}<|end|>\n", &msg[6..]));
                    } else if msg.starts_with("Assistant: ") {
                        prompt.push_str(&format!("<|assistant|>\n{}<|end|>\n", &msg[11..]));
                    }
                }
                prompt.push_str(&format!("<|user|>\n{}<|end|>\n<|assistant|>\n", user_input));
            }
            "deepseek" => {
                prompt.push_str(&format!("### System:\n{}\n", system_prompt));
                for msg in history {
                    if msg.starts_with("User: ") {
                        prompt.push_str(&format!("### Human:\n{}\n", &msg[6..]));
                    } else if msg.starts_with("Assistant: ") {
                        prompt.push_str(&format!("### Assistant:\n{}\n", &msg[11..]));
                    }
                }
                prompt.push_str(&format!("### Human:\n{}\n### Assistant:\n", user_input));
            }
            "chatml" => {
                prompt.push_str(&format!("<|im_start|>system\n{}<|im_end|>\n", system_prompt));
                for msg in history {
                    if msg.starts_with("User: ") {
                        prompt.push_str(&format!("<|im_start|>user\n{}<|im_end|>\n", &msg[6..]));
                    } else if msg.starts_with("Assistant: ") {
                        prompt.push_str(&format!("<|im_start|>assistant\n{}<|im_end|>\n", &msg[11..]));
                    }
                }
                prompt.push_str(&format!("<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", user_input));
            }
            "mistral" => {
                prompt.push_str("<s>");
                for msg in history {
                    if msg.starts_with("User: ") {
                        prompt.push_str(&format!("[INST] {} [/INST]", &msg[6..]));
                    } else if msg.starts_with("Assistant: ") {
                        prompt.push_str(&format!(" {} ", &msg[11..]));
                    }
                }
                prompt.push_str(&format!("[INST] {} [/INST]", user_input));
            }
            "gemma" => {
                prompt.push_str("<bos>");
                prompt.push_str(&format!("<start_of_turn>system\n{}<end_of_turn>\n", system_prompt));
                for msg in history {
                    if msg.starts_with("User: ") {
                        prompt.push_str(&format!("<start_of_turn>user\n{}<end_of_turn>\n", &msg[6..]));
                    } else if msg.starts_with("Assistant: ") {
                        prompt.push_str(&format!("<start_of_turn>model\n{}<end_of_turn>\n", &msg[11..]));
                    }
                }
                prompt.push_str(&format!("<start_of_turn>user\n{}<end_of_turn>\n<start_of_turn>model\n", user_input));
            }
            _ => { // llama3 default
                prompt.push_str("<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n");
                prompt.push_str(system_prompt);
                prompt.push_str("<|eot_id|>");
                for msg in history {
                    if msg.starts_with("User: ") {
                        prompt.push_str(&format!("<|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|>", &msg[6..]));
                    } else if msg.starts_with("Assistant: ") {
                        prompt.push_str(&format!("<|start_header_id|>assistant<|end_header_id|>\n\n{}<|eot_id|>", &msg[11..]));
                    }
                }
                prompt.push_str(&format!("<|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n", user_input));
            }
        }
        prompt
    }

    pub fn ask_stream<F>(
        &self,
        user_input: &str,
        history: &[String],
        max_tokens: i32,
        system_prompt: &str,
        stop: &AtomicBool,
        mut on_token: F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(&str),
    {
        let prompt = self.format_chat_prompt(history, user_input, system_prompt);
        self.generate_raw(&prompt, max_tokens, stop, Some(&mut on_token))
    }

    /// Core generation loop operating on a pre-formatted prompt string.
    fn generate_raw(
        &self,
        prompt: &str,
        max_tokens: i32,
        stop: &AtomicBool,
        mut on_token: Option<&mut dyn FnMut(&str)>,
    ) -> anyhow::Result<String> {
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

        // Quan trọng: Sử dụng AddBos::Never vì template đã có <|begin_of_text|>
        let tokens = self.model.str_to_token(prompt, AddBos::Never)?;

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
            response.push_str(&piece);
            if let Some(cb) = on_token.as_deref_mut() {
                cb(&piece);
            }

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

    pub fn generate_stream<F>(
        &self,
        user_input: &str,
        max_tokens: i32,
        system_prompt: &str,
        stop: &AtomicBool,
        mut on_token: F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(&str),
    {
        let prompt = self.format_prompt(user_input, system_prompt);
        self.generate_raw(&prompt, max_tokens, stop, Some(&mut on_token))
    }
}
