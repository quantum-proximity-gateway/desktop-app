use std::collections::HashMap;
use ollama_rs::generation::chat::MessageRole;
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse};
use ollama_rs::Ollama;
use serde::{Serialize, Deserialize};
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


#[derive(Debug, Deserialize)]
struct ChatRequest {
    model: String,
    prompt: String,
    chat_id: String,
}

#[derive(Serialize, Deserialize)]
struct ModelResponse { // This is the response the model is trained to give
    message: String,
    command: String
}

fn parse_model_response(json_str: String) -> Result<ModelResponse, serde_json::Error> {
    // Sometimes the model might not respond in the right format, we need to think of a way to handle that.
    let parsed_response: ModelResponse = serde_json::from_str(&json_str)?;
    Ok(parsed_response)
}

#[tauri::command]
async fn generate(request:ChatRequest, g_ollama: State<'_, OllamaInstance>, seen_chats: State<'_, ChatIDs>) -> Result<ChatMessageResponse, String> {
    println!("Generating response for {:?}", request);
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
        Ok(mut res) => {
            println!("Received initial response: {:?}", res);
            let response = res.message.unwrap().content;
            match parse_model_response(response) {
                Ok(parsed_response) => {
                    // execute shell command https://v1.tauri.app/v1/api/js/shell/
                    println!("Command executed: {}", parsed_response.command);
                    res.message = Some(ChatMessage::new(MessageRole::Assistant, parsed_response.message)); 
                    println!("Model Response: {:?}", res);
                    Ok(res)
                }
                Err(e) => {
                    Err(format!("Failed to parse model response: {}", e))
                }
            }
        },
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
