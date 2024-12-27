use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse};
use ollama_rs::Ollama;
use serde::Deserialize;
use tokio::sync::Mutex as TokioMutex;
use tauri::State;


struct OllamaInstance(TokioMutex<Ollama>);

#[tauri::command]
async fn list_models() -> Result<Vec<String>, String> {
    let ollama = Ollama::default();
    match ollama.list_local_models().await {
        Ok(res) => {
            let models: Vec<String> = res.into_iter().map(|model| model.name).collect();
            Ok(models)
        }
        Err(e) => Err(format!("Failed to list models: {}", e)),
    }
}
#[derive(Deserialize)]
struct ChatRequest {
    model: String,
    prompt: String,
    chat_id: String,
}

#[tauri::command]
async fn generate(request:ChatRequest, g_ollama: State<'_, OllamaInstance>) -> Result<ChatMessageResponse, String> {
    let mut ollama = g_ollama.0.lock().await;
    match ollama.send_chat_messages_with_history(
        ChatMessageRequest::new(request.model, vec![ChatMessage::user(request.prompt)]),
        request.chat_id,
    ).await {
        Ok(res) => Ok(res),
        Err(e) => Err(format!("Failed to generate text: {}", e)),
    }
}


// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(OllamaInstance(TokioMutex::new(Ollama::new_default_with_history(30))))
        .invoke_handler(tauri::generate_handler![list_models, generate])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
