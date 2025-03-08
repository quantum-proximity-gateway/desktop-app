use ollama_rs::generation::chat::MessageRole;
use tauri::async_runtime::block_on;
use url::Url;
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse};
use ollama_rs::Ollama;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::Value;
use std::collections::HashMap;
use tauri::State;
use tokio::sync::Mutex as TokioMutex;
use tauri_plugin_shell::ShellExt;
use reqwest::Client;

mod encryption;
use encryption::{DecryptData, EncryptionClient};


const OLLAMA_BASE_URL: &str = "http://localhost:11434";
const SERVER_URL: &str = "https://2f3f-5-151-28-149.ngrok-free.app";

struct OllamaInstance(TokioMutex<Ollama>);
struct EncryptionClientInstance(TokioMutex<EncryptionClient>);
struct ChatIDs(TokioMutex<HashMap<String, bool>>);

#[tauri::command]
async fn list_models() -> Result<Vec<String>, String> {
    let ollama = Ollama::new_with_history_from_url(
        Url::parse(OLLAMA_BASE_URL).unwrap(),
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

#[derive(Serialize)]
struct GenerateResult {
    ollama_response: ChatMessageResponse,
    command: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PreferencesAPIResponse {
    preferences: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct UpdateJSONPreferencesRequest {
    username: String,
    preferences: AppConfig,
}

#[cfg(target_os = "linux")]
fn get_linux_gui() -> Option<String> {
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
	if desktop.to_lowercase().contains("gnome") {
	    return Some("gnome".to_string());
	}
	return Some(desktop);
    }

    if let Ok(session) = std::env::var("DESKTOP_SESSION") {
	if session.to_lowercase().contains("gnome") {
	    return Some("gnome".to_string());
	}
	return Some(session);
    }

    None
}

#[tauri::command]
fn get_platform_info() -> String {
    #[cfg(target_os = "macos")] {
	return "macos".into();
    }

    #[cfg(target_os = "windows")] {
	return "windows".into();
    }

    #[cfg(target_os = "linux")] {
	let frontend_env = get_linux_gui();

	if let Some(env) = frontend_env {
	    if env.to_lowercase().contains("gnome") {
		return "gnome".into();
	    } else {
		return format!("linux-{env}").into();
	    }
	} else {
	    return "linux-unknown".into();
	}
    }
}

#[tauri::command]
async fn get_username(app_handle: tauri::AppHandle) -> Result<String, String> {
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

fn filter_json_by_env(json_str: &str, env: &str) -> Result<String, serde_json::Error> {
    let mut data: Value = serde_json::from_str(json_str)?;

    if let Value::Object(ref mut categories) = data {
	for (_, value) in categories.iter_mut() {
	    if let Value::Object(ref mut settings) = value {
		if let Some(Value::Object(commands)) = settings.get_mut("commands") {
		    *commands = commands
			.iter()
			.filter(|(key, _)| key.as_str() == env)
			.map(|(k, v)| (k.clone(), v.clone()))
			.collect();
		}
	    }
	}
    }

    serde_json::to_string_pretty(&data)
}

#[tauri::command]
async fn generate(
    request: ChatRequest,
    encryption_instance: State<'_, EncryptionClientInstance>,
    g_ollama: State<'_, OllamaInstance>,
    seen_chats: State<'_, ChatIDs>,
    app_handle: tauri::AppHandle
) -> Result<GenerateResult, String> {

    
    println!("Generating response for {:?}", request);

    // Fetch username using whomai to fetch user's preferences
    let username = match get_username(app_handle.clone()).await {
        Ok(username) => username,
        Err(e) => {
            println!("Warning: failed to get username from whoami: {}", e);
	    return Err(format!("Failed to fetch username."));
        }
    };

    println!("past username {:?}", username);

    let platform_info = get_platform_info();
    println!("Platform: {}", platform_info);
    
    // Fetch preferences
    let encryption_client = encryption_instance.0.lock().await;
    let client = Client::new();
    let mut url = Url::parse(&format!("{}/preferences/{}", SERVER_URL, username)).unwrap();
    url.query_pairs_mut().append_pair("client_id", &encryption_client.client_id);
    let response = client
	.get(url)
	.send()
	.await
	.map_err(|e| format!("Failed to fetch preferences: {}", e))?;
    
    println!("past response {:?}", response.status());

    if !response.status().is_success() {
        return Err(format!("Failed to fetch preferences: {}", response.status()));
    }

    println!("Decrypting");
    let response_body: String = response.text().await.map_err(|e| format!("Failed to read response body: {}", e))?;
    println!("after response body: {:?}", response_body);
    let encrypted_body: DecryptData = serde_json::from_str(&response_body).map_err(|e| format!("Failed to parse JSON: {}", e))?;
    println!("after encrypted body");
    let decrypted_body: String = encryption_client.decrypt_data(encrypted_body)?;
    println!("Decrypted body: {}", decrypted_body);

    // Some issue here maybe reuse fetch_username
    let json_example = match filter_json_by_env(&decrypted_body, &platform_info) {
	Ok(filtered) => filtered,
	Err(e) => return Err(format!("Failed to filter JSON: {}", e)),
    };
    println!("Filtered JSON for {}: {}", platform_info, json_example);
    
    let sys_prompt = format!(
	r#""You're an assistant that only replies in JSON format with keys "message" and "command".
It is very important that you stick to the following JSON format.

Your main job is to act as a computer accessibility coach that will reply to queries with a JSON
that has the following keys:
- "message": Something you want to say to the user
- "command": A gsettings accessibility command to run

Below is a reference JSON that shows possible accessibility commands for GNOME:

{}

The "current" field is the current value on the computer, while the "lower_bound",
"upper_bound", and "default" fields represent the ranges/values in gsettings.
Use this reference to inform your responses if needed. However, always reply with just
the final JSON object, like:

{{
  "message": "...",
  "command": "..."
}}"#, json_example);
    
    // Prompt the model
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
	    let response = res.message.as_ref().map(|m| m.content.clone()).unwrap_or_default();
            match parse_model_response(response) {
                Ok(parsed_response) => {
                    res.message = Some(ChatMessage::new(
			MessageRole::Assistant,
			parsed_response.message,
		    ));

		    Ok(GenerateResult {
			ollama_response: res,
			command: Some(parsed_response.command),
		    })
                }
                Err(e) => Err(format!("Failed to parse model response: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to generate text: {}", e)),
    }
}

async fn update_json_current_value(
    username: &str,
    base_command: &str,
    new_value_str: &str,
    encryption_instance: State<'_, EncryptionClientInstance>,
) -> Result<(), String> {
   
    let encryption_client = encryption_instance.0.lock().await;
    let client = Client::new();
    let mut url = Url::parse(&format!("{}/preferences/{}", SERVER_URL, username)).unwrap();
    url.query_pairs_mut().append_pair("client_id", &encryption_client.client_id);
    let response = client
	.get(url)
	.send()
	.await
	.map_err(|e| format!("Failed to fetch preferences: {}", e))?;

    let prefs_resp = response
        .json::<PreferencesAPIResponse>()
        .await
        .map_err(|e| format!("Failed to parse preferences JSON: {}", e))?;

    let mut config: AppConfig = serde_json::from_value(prefs_resp.preferences)
        .map_err(|e| format!("Could not parse preferences into AppConfig: {}", e))?;

    let mut found_match = false;
    for (key, setting) in config.iter_mut() {
        if setting.commands.gnome.trim() == base_command.trim() {
            let new_val: DefaultValue = parse_new_value(new_value_str, &setting.default);
            setting.current = Some(new_val);

            found_match = true;
            println!("Updated '{}': current is now '{}'", key, new_value_str);
            break;
        }
    }

    if !found_match { // no match in json for command
        println!("No command exists for: {}", base_command);
        return Ok(());
    }

    let update_payload = UpdateJSONPreferencesRequest {
        username: username.to_string(),
        preferences: config,
    };

    let json_payload = serde_json::to_string(&update_payload)
        .map_err(|e| format!("Failed to serialize payload: {}", e))?;
    let encrypted_payload = encryption_client.encrypt_data(&json_payload);
 
    let post_url = format!("{}/preferences/update", SERVER_URL);
    let update_resp = client
        .post(&post_url)
        .json(&encrypted_payload)
        .send()
        .await
        .map_err(|e| format!("Failed to send update to server: {}", e))?;

    if !update_resp.status().is_success() {
        return Err(format!(
            "Server failed to update preferences. Status: {}",
            update_resp.status()
        ));
    }

    println!("Successfully updated preferences on the server.");
    Ok(())
}

fn parse_new_value(new_value_str: &str, default_val: &DefaultValue) -> DefaultValue {
    match default_val {
        DefaultValue::Bool(_) => {
            if let Ok(b) = new_value_str.parse::<bool>() {
                return DefaultValue::Bool(b);
            }
            DefaultValue::String(new_value_str.to_string())
        }
        DefaultValue::Float(_) => {
            if let Ok(f) = new_value_str.parse::<f32>() {
                return DefaultValue::Float(f);
            }
            DefaultValue::String(new_value_str.to_string())
        }
        DefaultValue::String(_) => {
            DefaultValue::String(new_value_str.to_string())
        }
    }
}

#[tauri::command]
async fn execute_command(
    command: String,
    app_handle: tauri::AppHandle,
    encryption_instance: State<'_, EncryptionClientInstance>
) -> Result<(), String> {
    // execute shell command https://v2.tauri.app/plugin/shell/
    let shell = app_handle.shell();
    
    let username = match get_username(app_handle.clone()).await {
        Ok(username) => username,
        Err(e) => {
            println!("Warning: failed to get username from whoami: {}", e);
        return Err(format!("Failed to fetch username."));
        }
    };

    println!("Attempting to run shell command: {}", command);

    let command_parts: Vec<&str> = command.split_whitespace().collect();
    if let Some((cmd, args)) = command_parts.split_first() {
    match shell.command(cmd).args(args).output().await { // so unsafe we need to whitelist only gsettings
            Ok(output) => {
                if output.status.success() {
            let stdout_str = String::from_utf8(output.stdout).unwrap_or_else(|_| "".to_string());
                    println!("Command result: {:?}", stdout_str);

                    if !args.is_empty() {
                        let new_value_str = args.last().unwrap().to_string();
                        let base_command_str = {
                            let without_last = &args[..args.len() - 1];
                            format!("{} {}", cmd, without_last.join(" "))
                        };

                        update_json_current_value(
                            &username,
                            &base_command_str,
                            &new_value_str,
                            encryption_instance.clone()
                        )
                        .await?;
                    }
                } else {
                    println!("Exit with code: {}", output.status.code().unwrap());
                }
            }
            Err(e) => {
                println!("Failed to execute command: {} with error {}", command, e);
            }
        }
    } else {
	match shell.command(command.clone()).output().await { // so unsafe we need to whitelist only gsettings
            Ok(output) => {
                if output.status.success() {
                    println!("Command result: {:?}", String::from_utf8(output.stdout));
                } else {
                    println!("Exit with code: {}", output.status.code().unwrap());
                }
            }
            Err(e) => {
                println!("Failed to execute command: {} with error {}", command, e);
            }
        }
    }
    
    // we will need to save new command settings here
    println!("Command executed: {}", command);

    Ok(())
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
pub struct Setting {
    #[serde(default)]
    lower_bound: Option<f32>,
    #[serde(default)]
    upper_bound: Option<f32>,
    default: DefaultValue,
    #[serde(default)]
    current: Option<DefaultValue>,
    commands: Commands,
}

pub type AppConfig = HashMap<String, Setting>;

#[tauri::command]
async fn fetch_preferences(app_handle: tauri::AppHandle, encryption_instance: State<'_, EncryptionClientInstance>,) -> Result<PreferencesAPIResponse, String> {
    use std::fs;
    use serde_json;

    let encryption_client = encryption_instance.0.lock().await;

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

    let username = match get_username(app_handle.clone()).await {
        Ok(username) => username,
        Err(e) => {
            println!("Warning: failed to get username from whoami: {}", e);
	    return Err(format!("Failed to fetch username."));
        }
    };

    let mut url = Url::parse(&format!("{}/preferences/{}", SERVER_URL, username)).unwrap();
    url.query_pairs_mut().append_pair("client_id", &encryption_client.client_id);
    let client = Client::new();

    match client.get(url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                let response_body: String = response.text().await.map_err(|e| format!("Failed to read response body: {}", e))?;
                let encrypted_body: DecryptData = serde_json::from_str(&response_body).map_err(|e| format!("Failed to parse JSON: {}", e))?;
                let decrypted_body: String = encryption_client.decrypt_data(encrypted_body)?;
                let preferences: PreferencesAPIResponse = serde_json::from_str(&decrypted_body).map_err(|e| format!("Failed to parse JSON: {}", e))?;
                Ok(preferences)
                
            } else {
                println!("Failed to fetch preferences. Status: {}", response.status());
                Ok(PreferencesAPIResponse {
                    preferences: serde_json::to_value(default_commands).unwrap_or_default(),
                })
            }
        }
        Err(e) => {
            println!("HTTP request failed: {}", e);
            Ok(PreferencesAPIResponse {
                preferences: serde_json::to_value(default_commands).unwrap_or_default(),
            })
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

                let _ = app.handle().plugin(tauri_plugin_autostart::init(
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
        .manage(EncryptionClientInstance(TokioMutex::new(
            block_on(async {
                match EncryptionClient::new(SERVER_URL).await {
                    Ok(client) => {
                        println!("EncryptionClient created successfully!");
                        println!("Client ID: {}", client.client_id);
                        println!("Shared Secret {:?}", client.shared_secret);
                        client
                    }
                    Err(e) => {
                        eprintln!("Failed to create EncryptionClient: {}", e);
                        panic!("Failed to create EncryptionClient");
                    }
                }
            })
        )))
        .manage(OllamaInstance(TokioMutex::new(
	    Ollama::new_with_history_from_url(
	        Url::parse(OLLAMA_BASE_URL).unwrap(),
                50,
            )
        )))
        .manage(ChatIDs(TokioMutex::new(HashMap::new())))
        .invoke_handler(tauri::generate_handler![list_models, generate, fetch_preferences, execute_command, get_username])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
