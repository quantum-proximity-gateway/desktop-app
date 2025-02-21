use ollama_rs::generation::chat::MessageRole;
use url::Url;
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse};
use ollama_rs::Ollama;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use tauri::State;
use tokio::sync::Mutex as TokioMutex;
use tauri_plugin_shell::ShellExt;
use reqwest::Client;

struct OllamaInstance(TokioMutex<Ollama>);
struct ChatIDs(TokioMutex<HashMap<String, bool>>);

#[tauri::command]
async fn list_models() -> Result<Vec<String>, String> {
    let ollama = Ollama::new_with_history_from_url(
        Url::parse("https://0b53-31-205-125-243.ngrok-free.app").unwrap(),
        50,
    );
    let default_model_name = "granite3-dense:8b".to_string();

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

    let json_example = fs::read_to_string("src/json_example.json").unwrap_or_else(|_| "{}".to_string());

    let sys_prompt = format!(
	r#""You're an assistant that only replies in JSON format with keys "message" and "command".
It is very important that you stick to the following JSON format.

Your main job is to act as a computer accessibility coach that will reply to queries with a JSON
that has the following keys:
- "message": Something you want to say to the user
- "command": A gsettings accessibility command to run

Below is a reference JSON that shows possible accessibility commands for GNOME:

{}

Use this reference to inform your responses if needed. However, always reply
with just the final JSON object, like:

{{
  "message": "...",
  "command": "..."
}}"#, json_example);
    
    let mut ollama = g_ollama.0.lock().await;
    let mut seen_chats = seen_chats.0.lock().await;
    
    if !seen_chats.contains_key(&request.chat_id) {
        seen_chats.insert(request.chat_id.clone(), true);
        if let Err(e) = ollama.send_chat_messages_with_history(
            ChatMessageRequest::new(request.model.clone(), vec![ChatMessage::system(sys_prompt)]),
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
            // let response = res.message.unwrap().content;
	    let response = res.message.as_ref().map(|m| m.content.clone()).unwrap_or_default();
            match parse_model_response(response) {
                Ok(parsed_response) => {
                    // execute shell command https://v2.tauri.app/plugin/shell/
                    let shell = app_handle.shell();

		    let command_parts: Vec<&str> = parsed_response.command.split_whitespace().collect();
		    if let Some((command, args)) = command_parts.split_first() {
		        match shell.command(command).args(args).output().await { // so unsafe we need to whitelist only gsettings
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
		    } else {
		        match shell.command(parsed_response.command.clone()).output().await { // so unsafe we need to whitelist only gsettings
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

#[derive(Debug, Serialize, Deserialize)]
struct Commands {
    windows: String,
    macos: String,
    gnome: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum DefaultValue {
    Float(f32),
    Bool(bool),
    String(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct Setting {
    #[serde(default)]
    lower_bound: Option<f32>,
    #[serde(default)]
    upper_bound: Option<f32>,
    default: DefaultValue,
    commands: Commands,
}

pub type AppConfig = HashMap<String, Setting>;

#[tauri::command]
async fn fetch_preferences() -> Result<AppConfig, String> {
    use std::fs;
    use serde_json;

    fn load_default_app_config(path: &str) -> Result<AppConfig, Box<dyn std::error::Error>> {
        let file_contents = fs::read_to_string(path)?;
        let config: AppConfig = serde_json::from_str(&file_contents)?;
        Ok(config)
    }

    let default_commands = match load_default_app_config("src/json_example.json") {
        Ok(config) => config,
        Err(e) => {
            println!("Failed to load defaults: {}", e);
            HashMap::new()
        }
    };

    let api_url = "https://localhost:8000/devices/{username}/preferences"; // Replace placeholders with actual values
    let client = Client::new();

    match client.get(api_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<AppConfig>().await {
                    Ok(preferences) => Ok(preferences),
                    Err(e) => {
                        println!("Failed to parse preferences: {}", e);
                        Ok(default_commands) // Placeholder data
                    }
                }
            } else {
                println!("Failed to fetch preferences. Status: {}", response.status());
                Ok(default_commands) // Placeholder data
            }
        }
        Err(e) => {
            println!("HTTP request failed: {}", e);
            Ok(default_commands) // Placeholder data
        }
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(desktop)]
            {
                use tauri_plugin_autostart::MacosLauncher;
                use tauri_plugin_autostart::ManagerExt;

                app.handle().plugin(tauri_plugin_autostart::init(
                    MacosLauncher::LaunchAgent,
                    Some(vec!["--flag1", "--flag2"]),
                ));

                // Get the autostart manager
                let autostart_manager = app.autolaunch();
                // Enable autostart
                let _ = autostart_manager.enable();
                // Check enable state
                println!("registered for autostart? {}", autostart_manager.is_enabled().unwrap());
                // Disable autostart
                let _ = autostart_manager.disable();
            }
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(OllamaInstance(TokioMutex::new(
	    Ollama::new_with_history_from_url(
	        Url::parse("https://0b53-31-205-125-243.ngrok-free.app").unwrap(),
                50,
            )
        )))
        .manage(ChatIDs(TokioMutex::new(HashMap::new())))
        .invoke_handler(tauri::generate_handler![list_models, generate, fetch_preferences])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
