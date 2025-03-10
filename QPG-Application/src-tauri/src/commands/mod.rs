pub mod startup;
pub mod generation;

pub use startup::init_startup_commands;

use tauri::State;
use crate::state::{EncryptionClientInstance, OllamaInstance, ChatIDs, GenerateState};
use crate::preferences;
use crate::models::{ChatRequest, GenerateResult};
use tauri_plugin_shell::ShellExt;

#[tauri::command]
pub async fn list_models() -> Result<Vec<String>, String> {
    use ollama_rs::Ollama;
    let ollama = Ollama::new_with_history_from_url(
        url::Url::parse(preferences::OLLAMA_BASE_URL).unwrap(),
        50,
    );
    let default_model_name = "granite3-dense:8b".to_string();

    let local_models = match ollama.list_local_models().await {
        Ok(res) => res,
        Err(e) => return Err(format!("Failed to list models: {}", e)),
    };

    if local_models.is_empty() {
        println!("No local models found. Pulling {}...", default_model_name);
        if let Err(e) = ollama.pull_model(default_model_name.into(), false).await {
            return Err(format!("Failed to pull model: {}", e));
        }
    }

    let updated_models = match ollama.list_local_models().await {
        Ok(res) => res,
        Err(e) => return Err(format!("Failed to list models: {}", e)),
    };

    let models: Vec<String> = updated_models.into_iter().map(|model| model.name).collect();
    Ok(models)
}

#[tauri::command]
pub async fn get_username(app_handle: tauri::AppHandle) -> Result<String, String> {
    let shell = app_handle.shell();

    let output = shell.command("whoami").output().await
        .map_err(|e| format!("Failed to run whoami: {}", e))?;

    if output.status.success() {
        let username = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(username)
    } else {
        Err(format!("Command failed with exit code: {:?}", output.status.code()))
    }
}

#[tauri::command]
pub async fn fetch_preferences(
    username: &str,
    encryption_instance: State<'_, EncryptionClientInstance>,
    env: &str,
    state: State<'_, GenerateState>,
) -> Result<String, String> {
    preferences::fetch_preferences_impl(&username, &encryption_instance, &env, &state).await
}

#[tauri::command]
pub async fn execute_command(
    command: String,
    update: bool,
    app_handle: tauri::AppHandle,
    encryption_instance: State<'_, EncryptionClientInstance>,
    state: State<'_, GenerateState>,
) -> Result<(), String> {
    generation::execute_command_impl(
        command,
        update,
        app_handle,
        encryption_instance,
        state
    ).await
}

#[tauri::command]
pub async fn generate(
    request: ChatRequest,
    encryption_instance: State<'_, EncryptionClientInstance>,
    g_ollama: State<'_, OllamaInstance>,
    seen_chats: State<'_, ChatIDs>,
    app_handle: tauri::AppHandle,
    gen_state: State<'_, GenerateState>
) -> Result<GenerateResult, String> {
    generation::generate_impl(
        request,
        encryption_instance,
        g_ollama,
        seen_chats,
        app_handle,
        gen_state
    ).await
}
