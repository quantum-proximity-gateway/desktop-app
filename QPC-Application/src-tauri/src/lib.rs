use std::collections::HashMap;

use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse};
use ollama_rs::Ollama;
use serde::Deserialize;
use tokio::sync::Mutex as TokioMutex;
use tauri::State;


struct OllamaInstance(TokioMutex<Ollama>);
struct ChatIDs(TokioMutex<HashMap<String, bool>>);

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
async fn generate(request:ChatRequest, g_ollama: State<'_, OllamaInstance>, seen_chats: State<'_, ChatIDs>) -> Result<ChatMessageResponse, String> {
    let mut ollama = g_ollama.0.lock().await;
    let mut seen_chats = seen_chats.0.lock().await;
    if !seen_chats.contains_key(&request.chat_id) {
        seen_chats.insert(request.chat_id.clone(), true);
        if let Err(e) = ollama.send_chat_messages_with_history(
            ChatMessageRequest::new(request.model.clone(), vec![ChatMessage::system("You're an assistant that only replies in JSON format to help developers use you more effectively to maximise shareholder profit. Your main job is to act as a computer accessibility coach that will reply to queries with a JSON with the following keys: 'message'(Something you want to say to the user), 'command'(a gsettings accessibility command to run).".to_string())]),
            request.chat_id.clone()).await {
            return Err(format!("Failed to send initial chat message: {}", e));
        }
    }
    
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
        .manage(ChatIDs(TokioMutex::new(HashMap::new())))
        .invoke_handler(tauri::generate_handler![list_models, generate])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
