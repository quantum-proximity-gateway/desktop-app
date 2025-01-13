use ollama_rs::generation::chat::MessageRole;
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse};
use ollama_rs::Ollama;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;
use tokio::sync::Mutex as TokioMutex;
use tauri_plugin_shell::ShellExt;
use request::Client;

struct OllamaInstance(TokioMutex<Ollama>);
struct ChatIDs(TokioMutex<HashMap<String, bool>>);

#[tauri::command]
async fn list_models() -> Result<Vec<String>, String> {
    let ollama = Ollama::default();
    let default_model_name = "granite-code:3b".to_string();

    let local_models = match ollama.list_local_models().await {
        Ok(res) => res,
        Err(e) => return Err(format!("Failed to list models: {}", e)),
    };

    if local_models.is_empty() { // Download model in the case that it does not exist
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

#[derive(Debug, Deserialize)]
struct ChatRequest {
    model: String,
    prompt: String,
    chat_id: String,
}

#[derive(Serialize, Deserialize)]
struct ModelResponse {
    // This is the response the model is trained to give
    message: String,
    command: String,
}

fn parse_model_response(json_str: String) -> Result<ModelResponse, serde_json::Error> {
    // Sometimes the model might not respond in the right format, we need to think of a way to handle that.
    let parsed_response: ModelResponse = serde_json::from_str(&json_str)?;
    Ok(parsed_response)
}

#[tauri::command]
async fn generate(
    request: ChatRequest,
    g_ollama: State<'_, OllamaInstance>,
    seen_chats: State<'_, ChatIDs>,
    app_handle: tauri::AppHandle
) -> Result<ChatMessageResponse, String> {
    println!("Generating response for {:?}", request);
    let mut ollama = g_ollama.0.lock().await;
    let mut seen_chats = seen_chats.0.lock().await;

    if !seen_chats.contains_key(&request.chat_id) {
        seen_chats.insert(request.chat_id.clone(), true);
        if let Err(e) = ollama.send_chat_messages_with_history(
            ChatMessageRequest::new(request.model.clone(), vec![ChatMessage::system(r#"You're an assistant that only replies in JSON format which contain gsettings command and a message, it is very important that you stick to the following JSON format. 
            Your main job is to act as a computer accessibility coach that will reply to queries with a JSON with the following keys: 'message'(Something you want to say to the user), 
            'command'(a gsettings accessibility command to run)."#.to_string())]),
            request.chat_id.clone()).await {
            return Err(format!("Failed to send initial chat message: {}", e));
        }
    }

    match ollama
        .send_chat_messages_with_history(
            ChatMessageRequest::new(request.model, vec![ChatMessage::user(request.prompt)]),
            request.chat_id,
        )
        .await
    {
        Ok(mut res) => {
            println!("Received initial response: {:?}", res);
            let response = res.message.unwrap().content;
            match parse_model_response(response) {
                Ok(parsed_response) => {
                    // execute shell command https://v2.tauri.app/plugin/shell/
                    let shell = app_handle.shell();
                    match shell.command(parsed_response.command.clone()).output().await // so unsafe we need to whitelist only gsettings
                        {
                            Ok(output) => {
                                if output.status.success() {
                                    println!("Command result: {:?}", String::from_utf8(output.stdout));
                                } else {
                                    println!("Exit with code: {}", output.status.code().unwrap());
                                }
                            }
                            Err(e) => {
                                println!("Failed to execute command: {} with error {}", parsed_response.command.clone(), e);
                            }
                        }
                    // we will need to save new command settings here
                    println!("Command executed: {}", parsed_response.command);
                    res.message = Some(ChatMessage::new(
                        MessageRole::Assistant,
                        parsed_response.message,
                    ));
                    println!("Model Response: {:?}", res);
                    Ok(res)
                }
                Err(e) => Err(format!("Failed to parse model response: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to generate text: {}", e)),
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Preferences {
    ".qf1 2l;kalw mlkwam klm
}

#[tauri::command]
async fn fetch_preferences() -> Result<Preferences, String> {
    let api_url = "https://  {domain}  /devices/  {mac address}  /preferences";
    let client = Client::new();

    match client.get(api_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<Preferences>().await {
                    Ok(preferences) => Ok(preferences),
                    Err(e) => Err(format!("Failed to parse preferences: {}", e)),
                }
            } else {
                Err(format!("Failed to fetch preferences. Status: {}", response.status()))
            }
        }
        Err(e) => Err(format!("HTTP request failed: {}", e)),
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(OllamaInstance(TokioMutex::new(
            Ollama::new_default_with_history(30),
        )))
        .manage(ChatIDs(TokioMutex::new(HashMap::new())))
        .invoke_handler(tauri::generate_handler![list_models, generate, fetch_preferences])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
