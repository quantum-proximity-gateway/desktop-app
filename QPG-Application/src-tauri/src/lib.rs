use ollama_rs::generation::chat::MessageRole;
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse};
use ollama_rs::Ollama;
use tauri::async_runtime::block_on;
use tauri::State;
use tauri_plugin_shell::ShellExt;
use url::Url;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::OnceCell;
use reqwest::Client;
mod encryption;
use encryption::{DecryptData, EncryptionClient};
use rust_stemmers::{Algorithm, Stemmer};

const OLLAMA_BASE_URL: &str = "https://6ad3-31-205-125-238.ngrok-free.app";
const SERVER_URL: &str = "https://11f6-5-151-28-149.ngrok-free.app";

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
async fn get_platform_info() -> String {
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
fn preprocess_text(text: &str) -> HashSet<String> {
    let stopwords: HashSet<&str> = [
        "the", "is", "to", "a", "and", "for", "on", "in", "of", "with", "set", "enable", "disable"
    ]
    .iter()
    .cloned()
    .collect();

    let stemmer = Stemmer::create(Algorithm::English);

    text.to_lowercase()
        .split_whitespace()
        .filter(|word| !stopwords.contains(*word))
        .map(|word| stemmer.stem(word).to_string())
        .collect()
}

#[tauri::command]
fn cosine_similarity(set1: &HashSet<String>, set2: &HashSet<String>) -> f64 {
    let intersection = set1.intersection(set2).count() as f64;
    let norm1 = set1.len() as f64;
    let norm2 = set2.len() as f64;

    if norm1 == 0.0 || norm2 == 0.0 {
        return 0.0;
    }

    intersection / (norm1.sqrt() * norm2.sqrt())
}

#[tauri::command]
fn find_best_match(prompt: &str, json_str: &str) -> Option<String> {
    let parsed_json: Value = serde_json::from_str(json_str).ok()?;
    let mut best_match = None;
    let mut highest_score = 0.0;

    let prompt_tokens = preprocess_text(prompt);

    if let Value::Object(settings) = parsed_json {
        for (key, value) in settings.iter() {

            if let Value::Object(commands) = value.get("commands")? {
                if !commands.is_empty() {
                    let key_tokens = preprocess_text(key);
                    let cosine_sim = cosine_similarity(&prompt_tokens, &key_tokens);

                    println!(
                        "Similarity of '{}' w/ '{}': {:.3}",
                        prompt, key, cosine_sim
                    );

                    if cosine_sim > highest_score {
                        highest_score = cosine_sim;
                        best_match = Some(key.clone());
                    }
                }
            }
	    
        }
    }

    best_match
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

#[derive(Default)]
pub struct GenerateState {
    username: OnceCell<String>,
    platform_info: OnceCell<String>,
    json_example: OnceCell<String>,
}

impl GenerateState {
    pub async fn get_username(&self, app_handle: &tauri::AppHandle) -> String {
	self.username.get_or_init(|| async {
	    get_username(app_handle.clone()).await.unwrap_or_else(|_| "unknown_user".to_string())
	}).await.clone()
    }

    pub async fn get_platform_info(&self) -> String {
	self.platform_info.get_or_init(|| async {
	    get_platform_info().await
	}).await.clone()
    }

    pub(crate) async fn get_json_example(&self, username: &str, encryption_instance: State<'_, EncryptionClientInstance>, platform_info: &str) -> String {
        self.json_example.get_or_init(|| async {
            fetch_preferences(username, encryption_instance.clone(), platform_info).await.unwrap_or_else(|_| "{}".to_string())
        }).await.clone()
    }
}

#[tauri::command]
async fn generate(
    request: ChatRequest,
    encryption_instance: State<'_, EncryptionClientInstance>,
    g_ollama: State<'_, OllamaInstance>,
    seen_chats: State<'_, ChatIDs>,
    app_handle: tauri::AppHandle,
    state: State<'_, GenerateState>
) -> Result<GenerateResult, String> {
    let username = state.get_username(&app_handle).await;
    println!("Username: {}", username);

    let platform_info = state.get_platform_info().await;
    println!("Platform: {}", platform_info);

    let json_example = state.get_json_example(&username, encryption_instance.clone(), &platform_info).await;
    println!("Filtered JSON: {}", json_example);
    
    let mut ollama = g_ollama.0.lock().await;
    let mut seen_chats = seen_chats.0.lock().await;
    
    if !seen_chats.contains_key(&request.chat_id) {
        seen_chats.insert(request.chat_id.clone(), true);

	let sys_prompt = format!(
		r#""You're an assistant that only replies in JSON format with keys "message" and "command".
It is very important that you stick to the following JSON format.

Your main job is to act as a computer accessibility coach that will reply to queries with a JSON
that has the following keys:
- "message": Something you want to say to the user
- "command": A gsettings accessibility command to run

Below is a reference JSON that shows possible accessibility commands for the current environment ({}):

{}

The "current" field is the current value on the computer, while the "lower_bound",
"upper_bound", and "default" fields represent the ranges/values in gsettings.
Use this reference to inform your responses if needed. The prompt will always begin
with a snippet of the reference JSON that is the most likely command the user is
referring to, but this may not always be accurate. You will need to add a value
to the end of the command based on the current and default fields in the JSON.
Refer to the user's prompt to decide how to choose this value. Remember, always
reply with just the final JSON object, like:

{{
  "message": "...",
  "command": "..."
}}"#, platform_info, json_example);
	println!("System Prompt Initialized");
	
        if let Err(e) = ollama.send_chat_messages_with_history(
            ChatMessageRequest::new(request.model.clone(), vec![ChatMessage::system(sys_prompt)]),
            request.chat_id.clone()).await {
            return Err(format!("Failed to send initial chat message: {}", e));
        }
    }

    let best_match = find_best_match(&request.prompt, &json_example);
    println!("Best match for prompt '{}': {:?}", request.prompt, best_match);

    let best_match_json = match best_match.as_ref() {
        Some(key) => {
            let parsed_json: Value = serde_json::from_str(&json_example).unwrap_or(Value::Null);
            if let Value::Object(mut settings) = parsed_json {
                if let Some(matching_value) = settings.remove(key) {
                    let mut new_obj = serde_json::Map::new();
                    new_obj.insert(key.clone(), matching_value);
                    serde_json::to_string_pretty(&Value::Object(new_obj)).unwrap_or_else(|_| json_example.clone())
                } else {
                    json_example.clone()
                }
            } else {
                json_example.clone()
            }
        }
        None => json_example.clone(),
    };
    println!("Filtered JSON for best match: {}", best_match_json);

    let user_prompt = format!("{}\n\n {}", best_match_json, request.prompt);
    match ollama
        .send_chat_messages_with_history(
            ChatMessageRequest::new(request.model, vec![ChatMessage::user(user_prompt)]),
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
async fn fetch_preferences(
    username: &str,
    encryption_instance: State<'_, EncryptionClientInstance>,
    env: &str,
) -> Result<String, String> {
    use std::fs;
    use serde_json;

    println!("[fetch_preferences] Fetching preferences for user: {}", username);
    println!("[fetch_preferences] Platform environment: {}", env);

    let encryption_client = encryption_instance.0.lock().await;

    // backup
    fn load_default_app_config(path: &str) -> Result<AppConfig, Box<dyn std::error::Error>> {
        let file_contents = fs::read_to_string(path)?;
        let config: AppConfig = serde_json::from_str(&file_contents)?;
        Ok(config)
    }

    let default_commands = match load_default_app_config("src/json_example.json") {
        Ok(config) => {
            println!("[fetch_preferences] Successfully loaded default JSON.");
            serde_json::to_value(config).unwrap_or_default()
        }
        Err(e) => {
            println!("[fetch_preferences] Failed to load default JSON: {}", e);
            serde_json::Value::Null
        }
    };

    let mut url = Url::parse(&format!("{}/preferences/{}", SERVER_URL, username)).unwrap();
    url.query_pairs_mut().append_pair("client_id", &encryption_client.client_id);
    let client = Client::new();

    println!("[fetch_preferences] Sending request to: {}", url);

    let preferences_json = match client.get(url).send().await {
        Ok(response) => {
            println!("[fetch_preferences] Server responded with status: {}", response.status());

            if response.status().is_success() {
                let response_body: String = response.text().await.map_err(|e| {
                    println!("[fetch_preferences] Failed to read response body: {}", e);
                    format!("Failed to read response body: {}", e)
                })?;

                println!("[fetch_preferences] Successfully received encrypted response: {}", response_body);

                let encrypted_body: DecryptData = serde_json::from_str(&response_body).map_err(|e| {
                    println!("[fetch_preferences] Failed to parse JSON: {}", e);
                    format!("Failed to parse JSON: {}", e)
                })?;

                let decrypted_body = encryption_client.decrypt_data(encrypted_body)?;
                println!("[fetch_preferences] Successfully decrypted preferences.");

                decrypted_body
            } else {
                println!("[fetch_preferences] Failed to fetch preferences. Using default JSON.");
                serde_json::to_string_pretty(&default_commands).unwrap_or_default()
            }
        }
        Err(e) => {
            println!("[fetch_preferences] HTTP request failed: {}", e);
            println!("[fetch_preferences] Falling back to local default JSON.");
            serde_json::to_string_pretty(&default_commands).unwrap_or_default()
        }
    };

    println!("[fetch_preferences] Filtering JSON for environment: {}", env);
    match filter_json_by_env(&preferences_json, env) {
        Ok(filtered_json) => {
            println!("[fetch_preferences] Successfully filtered JSON.");
            Ok(filtered_json)
        }
        Err(e) => {
            println!("[fetch_preferences] Failed to filter JSON: {}", e);
            Err(format!("Failed to filter JSON: {}", e))
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
	.manage(GenerateState::default())
        .invoke_handler(tauri::generate_handler![list_models, generate, fetch_preferences, execute_command, get_username])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
