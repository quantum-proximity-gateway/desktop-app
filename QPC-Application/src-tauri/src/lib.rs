use ollama_rs::{generation::{chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse}, completion::{
        request::GenerationRequest, GenerationContext, GenerationResponse
    }},
    Ollama
    
};
use serde::Deserialize;


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
    chatID: String,
}

#[tauri::command]
async fn generate(request:ChatRequest) -> Result<ChatMessageResponse, String> {
    let ollama = Ollama::default();
    match ollama.send_chat_messages(
        ChatMessageRequest::new(request.model, vec![ChatMessage::user(request.prompt)]),

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
        .invoke_handler(tauri::generate_handler![list_models, generate])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
