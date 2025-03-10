use ollama_rs::Ollama;
use ollama_rs::generation::chat::MessageRole;
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse};
use tauri::State;
use tauri::Manager;
use tauri::async_runtime::block_on;
use tauri_plugin_shell::ShellExt;
use url::Url;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use std::fs;
use std::collections::{HashMap, HashSet};
use tokio::sync::{Mutex as TokioMutex, RwLock, OnceCell};
use reqwest::Client;
mod encryption;
use encryption::{DecryptData, EncryptionClient};
use rust_stemmers::{Algorithm, Stemmer};

const OLLAMA_BASE_URL: &str = "http://localhost:11434";
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
    let map = parsed_json.as_object()?;

    let prompt_tokens = preprocess_text(prompt);

    let mut best_match: Option<String> = None;
    let mut highest_score = 0.0;

    for (key, setting_val) in map {
        if let Value::Object(obj) = setting_val {
            if let Some(Value::Object(commands_obj)) = obj.get("commands") {
                if !commands_obj.is_empty() {
                    let key_tokens = preprocess_text(key);
                    let score = cosine_similarity(&prompt_tokens, &key_tokens);
                    if score > highest_score {
                        highest_score = score;
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
	for (_, setting_value) in categories.iter_mut() {
	    if let Value::Object(ref mut setting_obj) = setting_value {
		if let Some(Value::Object(commands)) = setting_obj.get_mut("commands") {
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

pub struct GenerateState {
    username: OnceCell<String>,
    platform_info: OnceCell<String>,
    full_json_example: RwLock<String>,
    filtered_json_example: RwLock<String>,
}

impl Default for GenerateState {
    fn default() -> Self {
	Self {
	    username: OnceCell::new(),
	    platform_info: OnceCell::new(),
	    full_json_example: RwLock::new(String::new()),
	    filtered_json_example: RwLock::new(String::new()),
	}
    }
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

    pub async fn get_full_json(&self) -> String {
	self.full_json_example.read().await.clone()
    }

    pub async fn get_filtered_json(&self) -> String {
	self.filtered_json_example.read().await.clone()
    }

    pub async fn update_jsons(&self, new_full: &str, new_filtered: &str) {
	{
	    let mut w_full = self.full_json_example.write().await;
	    *w_full = new_full.to_string();
	}
	{
	    let mut w_filtered = self.filtered_json_example.write().await;
	    *w_filtered = new_filtered.to_string();
	}
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
    let platform_info = state.get_platform_info().await;

    if state.get_full_json().await.is_empty() {
	println!("[generate] Full JSON empty; fetching preferences from server...");
	if let Err(err) = fetch_preferences(&username, encryption_instance, &platform_info, state.clone()).await {
	    return Err(format!("Failed to automatically fetch preferences: {}", err));
	}
    }
    let filtered_json = state.get_filtered_json().await;
    
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
}}"#, platform_info, filtered_json);
	println!("System Prompt Initialized");
	
        if let Err(e) = ollama.send_chat_messages_with_history(
            ChatMessageRequest::new(request.model.clone(), vec![ChatMessage::system(sys_prompt)]),
            request.chat_id.clone()).await {
            return Err(format!("Failed to send initial chat message: {}", e));
        }
    }

    let best_match = find_best_match(&request.prompt, &filtered_json);
    println!("Best match for prompt '{}': {:?}", request.prompt, best_match);

    let best_match_json = match best_match.as_ref() {
        Some(key) => {
            let parsed_json: Value = serde_json::from_str(&filtered_json).unwrap_or(Value::Null);

	    if let Value::Object(mut root) = parsed_json {
		if let Some(matching_value) = root.remove(key) {
		    let mut new_obj = serde_json::Map::new();
		    new_obj.insert(key.clone(), matching_value);

		    serde_json::to_string_pretty(&Value::Object(new_obj))
			.unwrap_or_else(|_| filtered_json.clone())
		} else {
		    filtered_json.clone()
		}
	    } else {
		filtered_json.clone()
	    }
        }
        None => filtered_json.clone(),
    };
    println!("[generate] Filtered JSON for best match: {}", best_match_json);

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
	    let response_str = res.message.as_ref().map(|m| m.content.clone()).unwrap_or_default();

	    match serde_json::from_str::<ModelResponse>(&response_str) {
		Ok(parsed_response) => {
		    res.message = Some(ChatMessage::new(
			MessageRole::Assistant,
			parsed_response.message.clone(),
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
    state: State<'_, GenerateState>
) -> Result<(), String> {
    let encryption_client = encryption_instance.0.lock().await;
    let client = Client::new();

    let current_full_json = state.get_full_json().await;

    let mut config: AppConfig = serde_json::from_str(&current_full_json)
        .map_err(|e| format!("Could not parse preferences into AppConfig: {}", e))?;

    let mut found_match = false;
    for (_key, setting) in config.iter_mut() {
        if setting.commands.gnome.trim() == base_command.trim() {
            let new_val: DefaultValue = parse_new_value(new_value_str, &setting.default);
            setting.current = Some(new_val);
            found_match = true;
            println!("Updated command '{}': current is now '{}'", base_command, new_value_str);
            break;
        }
    }

    if !found_match { // no match in json for command
        println!("No command exists for: {}", base_command);
        return Ok(());
    }

    let updated_full_json = serde_json::to_string_pretty(&config)
	.map_err(|e| format!("Failed to serialize updated JSON: {}", e))?;

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

    let platform_info = state.get_platform_info().await;
    let newly_filtered = filter_json_by_env(&updated_full_json, &platform_info)
	.map_err(|e| format!("Failed to filter updated JSON: {}", e))?;

    state.update_jsons(&updated_full_json, &newly_filtered).await;
    
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

async fn gather_valid_commands_for_env(
    state: &GenerateState,
    env: &str,
) -> Result<HashSet<String>, String> {
    let full_json = state.get_full_json().await;
    if full_json.is_empty() {
	return Ok(HashSet::new());
    }

    let parsed: Value = match serde_json::from_str(&full_json) {
	Ok(val) => val,
	Err(e) => {
	    return Err(format!("Unable to parse full JSON in gather valid commands fn: {}", e));
	}
    };

    let mut valid_commands = HashSet::new();

    if let Value::Object(map) = parsed {
	for (_, setting_value) in map.iter() {

	    if let Ok(setting) = serde_json::from_value::<Setting>(setting_value.clone()) {
		let env_cmd = match env {
		    s if s.contains("gnome") => &setting.commands.gnome,
		    "macos" => &setting.commands.macos,
		    "windows" => &setting.commands.windows,
		    _ => "",
		};
		let trimmed = env_cmd.trim();
		if !trimmed.is_empty() {
		    valid_commands.insert(trimmed.to_string());
		}
	    }
	    
	}
    }

    Ok(valid_commands)
}

#[tauri::command]
async fn execute_command(
    command: String,
    app_handle: tauri::AppHandle,
    encryption_instance: State<'_, EncryptionClientInstance>,
    state: State<'_, GenerateState>,
) -> Result<(), String> {
    let command_parts: Vec<&str> = command.split_whitespace().collect();
    if command_parts.len() < 2 {
	return Err(format!("Invalid command format: must have base command + 1 argument"));
    }

    let (base_parts, value_part) = command_parts.split_at(command_parts.len() - 1);
    let last_value = value_part.first().unwrap();
    let base_cmd_str = base_parts.join(" ");

    let platform_info = state.get_platform_info().await;
    let valid_commands = gather_valid_commands_for_env(&state, &platform_info).await?;
    if !valid_commands.contains(&base_cmd_str) {
	return Err(format!("Unrecognised/unauthorised command base: '{}'. Will not execute command.", base_cmd_str));
    }

    println!("Attempting to run shell command: {}", command);
    
    let shell = app_handle.shell();
    let username = match get_username(app_handle.clone()).await {
        Ok(username) => username,
        Err(e) => {
            println!("Warning: failed to get username from whoami: {}", e);
            return Err(format!("Failed to fetch username."));
        }
    };

    match shell.command(&base_parts[0]).args(&base_parts[1..]).arg(last_value).output().await {
        Ok(output) => {
            if output.status.success() {
		let stdout_str = String::from_utf8(output.stdout).unwrap_or_else(|_| "".to_string());
                println!("Command result: {:?}", stdout_str);

		let base_command_str = base_parts.join(" ");
		let new_value_str = last_value.to_string();

                update_json_current_value(
                    &username,
                    &base_command_str,
                    &new_value_str,
                    encryption_instance.clone(),
		    state,
                )
                    .await?;
            } else {
                println!("Exit with code: {}", output.status.code().unwrap_or_default());
            }
        }
        Err(e) => {
            println!("Failed to execute command: {} with error {}", command, e);
        }
    }
    println!("[execute_command] Command executed: {}", command);

    Ok(())
}


#[derive(Debug, Serialize, Deserialize, Default)]
struct Commands {
    #[serde(default)]
    windows: String,
    #[serde(default)]
    macos: String,
    #[serde(default)]
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
    #[serde(default)]
    commands: Commands,
}

pub type AppConfig = HashMap<String, Setting>;

#[tauri::command]
async fn fetch_preferences(
    username: &str,
    encryption_instance: State<'_, EncryptionClientInstance>,
    env: &str,
    state: State<'_, GenerateState>,
) -> Result<String, String> {
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

    let flattened = match serde_json::from_str::<Value>(&preferences_json) {
        Ok(mut val) => {
            if let Value::Object(ref mut root_obj) = val {
                if let Some(Value::Object(inner_prefs)) = root_obj.remove("preferences") {
                    match serde_json::to_string_pretty(&Value::Object(inner_prefs)) {
                        Ok(s) => s,
                        Err(e) => {
                            println!("Failed flattening 'preferences': {}", e);
                            preferences_json.clone()
                        }
                    }
                } else {
                    preferences_json.clone()
                }
            } else {
                preferences_json.clone()
            }
        }
        Err(e) => {
            println!("Failed to parse server JSON: {}", e);
            preferences_json.clone()
        }
    };

    println!("[fetch_preferences] Filtering JSON for environment: {}", env);
    let filtered_json_str = match filter_json_by_env(&flattened, env) {
        Ok(fj) => {
            println!("[fetch_preferences] Successfully filtered JSON.");
            fj
        }
        Err(e) => {
	    let msg = format!("Failed to filter JSON: {}", e);
            println!("[fetch_preferences] {}", msg);
            return Err(msg);
        }
    };

    state.update_jsons(&flattened, &filtered_json_str).await;

    Ok(filtered_json_str)
}

#[tauri::command]
async fn init_startup_commands(
    app_handle: tauri::AppHandle,
    encryption_instance: State<'_, EncryptionClientInstance>,
    state: State<'_, GenerateState>,
) -> Result<(), String> {
    let username = state.get_username(&app_handle).await;
    let platform_info = state.get_platform_info().await;
    println!("[startup_init] username = {}, platform_info = {}", username, platform_info);

    if state.get_full_json().await.is_empty() {
	println!("[startup_init] Full JSON empty; fetching preferences from server...");
	fetch_preferences(&username, encryption_instance.clone(), &platform_info, state.clone())
            .await
            .map_err(|e| format!("Failed to fetch preferences during startup: {}", e))?;
    }

    let filtered_json = state.get_filtered_json().await;
    let parsed: Value = serde_json::from_str(&filtered_json)
	.map_err(|e| format!("Failed to parse filtered JSON: {}", e))?;

    if let Value::Object(obj) = parsed {
	for (_, setting_value) in obj.iter() {

	    if let Ok(setting) = serde_json::from_value::<Setting>(setting_value.clone()) {
		let command_str = match platform_info.as_str() {
		    "windows" => setting.commands.windows.clone(),
		    "macos" => setting.commands.macos.clone(),
		    s if s.contains("gnome") => setting.commands.gnome.clone(),
		    _ => "".to_string(),
		};
		if command_str.trim().is_empty() {
		    continue;
		}

		let final_value = match &setting.current {
		    Some(cv) => cv,
		    None => &setting.default,
		};

		let value_str = match final_value {
		    DefaultValue::Bool(b) => b.to_string(),
		    DefaultValue::Float(f) => f.to_string(),
		    DefaultValue::String(s) => s.to_string(),
		};

		let full_command = format!("{} {}", command_str.trim(), value_str);
		println!("[startup_init] Executing: {}", full_command);
		if let Err(e) = execute_command(full_command, app_handle.clone(), encryption_instance.clone(), state.clone()).await {
		    println!("Warning: failed to run startup command. Error: {}", e);
		}
	    }

	}
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(move |app| {
	    let handle = app.app_handle(); 
            
            tauri::async_runtime::block_on(async move {
                let encryption_instance = handle.state::<EncryptionClientInstance>();
                let generate_state = handle.state::<GenerateState>();

                if let Err(err) = init_startup_commands(
                    handle.clone(),
                    encryption_instance.clone(),
                    generate_state.clone(),
                )
                .await
                {
                    eprintln!("Failed to run startup init: {}", err);
                }
            });
	    
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
        .invoke_handler(tauri::generate_handler![list_models, init_startup_commands, generate, fetch_preferences, execute_command, get_username])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
