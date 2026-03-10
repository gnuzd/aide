use aide_core::models::inference::InferenceEngine;
use aide_core::system::SystemSpecs;
use aide_core::Aide;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::Mutex;

struct AppState {
    aide: Mutex<Aide>,
    engine: Mutex<Option<InferenceEngine>>,
    session_id: Mutex<Option<String>>,
}

#[derive(Serialize)]
struct ModelInfo {
    name: String,
    description: String,
    size_gb: f32,
    quality_score: u8,
    filename: String,
    is_downloaded: bool,
    is_active: bool,
}

#[derive(Serialize)]
struct SystemInfo {
    os: String,
    memory_total: f32,
    memory_available: f32,
    cpu: String,
    is_compatible: bool,
    warnings: Vec<String>,
}

#[tauri::command]
async fn get_system_info(state: State<'_, AppState>) -> Result<SystemInfo, String> {
    let _aide = state.aide.lock().await;
    let specs = SystemSpecs::audit();
    let (is_compatible, warnings) = specs.check_compatibility();

    Ok(SystemInfo {
        os: format!("{} {}", specs.os_name, specs.os_version),
        memory_total: specs.total_memory_gb as f32,
        memory_available: specs.available_memory_gb as f32,
        cpu: format!("{} ({} cores)", specs.cpu_brand, specs.cpu_cores),
        is_compatible,
        warnings,
    })
}

#[tauri::command]
async fn get_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let aide = state.aide.lock().await;
    let specs = SystemSpecs::audit();
    let models = aide
        .registry
        .get_compatible_models(&specs, aide_core::models::ModelType::General);
    let models_dir = aide.registry.base_path.join("models");

    let result = models
        .into_iter()
        .map(|m| {
            let model_path = models_dir.join(&m.filename);
            let is_downloaded = model_path.exists();
            let is_active = Some(model_path) == aide.config.active_model_path;
            ModelInfo {
                name: m.name.clone(),
                description: m.description.clone(),
                size_gb: m.size_gb as f32,
                quality_score: m.quality_score,
                filename: m.filename.clone(),
                is_downloaded,
                is_active,
            }
        })
        .collect();

    Ok(result)
}

#[tauri::command]
async fn download_model(state: State<'_, AppState>, filename: String) -> Result<(), String> {
    let model = {
        let aide = state.aide.lock().await;
        aide.registry
            .models
            .iter()
            .find(|m| m.filename == filename)
            .cloned()
            .ok_or_else(|| "Model not found".to_string())?
    };

    // Release lock during long download
    // Wait, download_model is async and aide is not sync.
    // So we must release aide before calling .await on download_model if it was held.
    // But download_model is on ModelRegistry which is inside Aide.

    // We can't call aide.registry.download_model(&model).await while holding the aide lock if Aide is not Sync.

    let path = {
        // Unfortunately download_model needs &self (Aide).
        // Let's see if we can call it differently.
        // Actually, download_model is async.
        // If we want to await it, we must drop the MutexGuard first if the type is not Sync.
        let mut aide = state.aide.lock().await;
        aide.registry
            .download_model(&model)
            .await
            .map_err(|e| e.to_string())?
    };

    let mut aide = state.aide.lock().await;
    aide.config.active_model_path = Some(path);
    aide.config.active_model_template = Some(model.template_type.clone());
    aide.registry
        .save_config(&aide.config)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn init_chat(state: State<'_, AppState>) -> Result<String, String> {
    let (engine, welcome) = {
        let aide = state.aide.lock().await;
        let engine = aide.create_inference_engine().map_err(|e| e.to_string())?;
        let welcome = aide
            .generate_welcome_message(&engine)
            .map_err(|e| e.to_string())?;
        (engine, welcome)
    };

    let mut state_engine = state.engine.lock().await;
    *state_engine = Some(engine);

    let mut state_session = state.session_id.lock().await;
    let aide = state.aide.lock().await;
    *state_session = Some(aide.generate_session_id());

    Ok(welcome)
}

#[tauri::command]
async fn send_message(
    state: State<'_, AppState>,
    message: String,
    window: tauri::Window,
) -> Result<String, String> {
    // 1. Prepare history and prompt (hold aide lock briefly)
    let (stats_turns, chat_history, turn_system_prompt, session_id) = {
        let aide = state.aide.lock().await;
        let session_id = state
            .session_id
            .lock()
            .await
            .as_ref()
            .cloned()
            .ok_or_else(|| "Session not initialized".to_string())?;

        let (stats_turns, _) = aide.memory.conversation_stats().unwrap_or((0, 0));
        let mut history: Vec<String> = Vec::new();
        if let Ok(recent) = aide.memory.load_recent_history(10) {
            for (u, a) in recent {
                history.push(format!("User: {}", u));
                history.push(format!("Assistant: {}", a));
            }
        }

        let mut turn_prompt = aide
            .memory
            .get_profile_summary()
            .unwrap_or_else(|_| "You are Aide, a helpful assistant.".to_string());
        if let Ok(semantic_facts) = aide.memory.search_semantic(&message, 3) {
            if !semantic_facts.is_empty() {
                turn_prompt.push_str("\nRelevant facts: ");
                turn_prompt.push_str(&semantic_facts.join("; "));
            }
        }
        (stats_turns, history, turn_prompt, session_id)
    };

    // 2. Perform inference (hold engine lock, but NOT aide lock)
    let collected = {
        let engine_lock = state.engine.lock().await;
        let engine = engine_lock
            .as_ref()
            .ok_or_else(|| "Engine not initialized".to_string())?;

        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut collected = String::new();
        let win = window.clone();

        engine
            .ask_stream(
                &message,
                &chat_history,
                1024,
                &turn_system_prompt,
                &stop,
                |token| {
                    let _ = win.emit("token", token);
                    collected.push_str(token);
                },
            )
            .map_err(|e| e.to_string())?;
        collected
    };

    // 3. Save to memory (hold aide lock briefly)
    {
        let aide = state.aide.lock().await;
        let turn_number = (stats_turns + 1) as u32;
        let _ = aide
            .memory
            .save_turn(&session_id, turn_number, &message, &collected);
    }

    Ok(collected)
}

#[tauri::command]
async fn get_memory(state: State<'_, AppState>) -> Result<String, String> {
    let aide = state.aide.lock().await;
    aide.memory.get_profile_summary().map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_data(state: State<'_, AppState>, target: String) -> Result<(), String> {
    let mut aide = state.aide.lock().await;
    match target.as_str() {
        "all" => aide.reset().map_err(|e| e.to_string())?,
        "chat" => aide.clear_conversations().map_err(|e| e.to_string())?,
        "profile" => aide.clear_profile().map_err(|e| e.to_string())?,
        "models" => aide.clear_models().map_err(|e| e.to_string())?,
        "config" => aide.clear_config().map_err(|e| e.to_string())?,
        _ => return Err("Unknown target".to_string()),
    }
    Ok(())
}

#[tauri::command]
async fn is_model_loaded(state: State<'_, AppState>) -> Result<bool, String> {
    let aide = state.aide.lock().await;
    Ok(aide.config.active_model_path.is_some())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let aide = Aide::new().expect("failed to initialize aide core");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            aide: Mutex::new(aide),
            engine: Mutex::new(None),
            session_id: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            get_models,
            download_model,
            init_chat,
            send_message,
            get_memory,
            clear_data,
            is_model_loaded
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

