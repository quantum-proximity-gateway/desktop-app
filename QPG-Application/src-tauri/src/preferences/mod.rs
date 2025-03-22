use reqwest::Client;
use std::fs;
use serde_json::Value;
use tauri::State;
use crate::state::{EncryptionClientInstance, GenerateState};
use crate::models::{AppConfig, Setting, UpdateJSONPreferencesRequest, DefaultValue};
use crate::encryption::DecryptData;

pub const OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
pub const SERVER_URL: &str = "https://litestar-server.1t65wn3ankpt.eu-gb.codeengine.appdomain.cloud";

pub async fn fetch_preferences_impl(
    username: &str,
    encryption_instance: &State<'_, EncryptionClientInstance>,
    env: &str,
    state: &State<'_, GenerateState>,
) -> Result<String, String> {
    println!("[fetch_preferences] Fetching preferences for user: {}", username);
    println!("[fetch_preferences] Platform environment: {}", env);

    let encryption_client = encryption_instance.0.lock().await;

    fn load_default_app_config(path: &str) -> Result<AppConfig, Box<dyn std::error::Error>> {
        let file_contents = fs::read_to_string(path)?;
        let config: AppConfig = serde_json::from_str(&file_contents)?;
        Ok(config)
    }
    let default_commands = match load_default_app_config("src/json_example.json") {
        Ok(config) => {
            println!("[fetch_preferences] Successfully loaded default JSON.");
            serde_json::to_value(config).unwrap_or(Value::Null)
        }
        Err(e) => {
            println!("[fetch_preferences] Failed to load default JSON: {}", e);
            Value::Null
        }
    };

    let mut url = url::Url::parse(&format!("{}/preferences/{}", SERVER_URL, username)).unwrap();
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

pub fn filter_json_by_env(json_str: &str, env: &str) -> Result<String, serde_json::Error> {
    let mut data: Value = serde_json::from_str(json_str)?;

    if let Value::Object(ref mut categories) = data {
        for (_key, setting_value) in categories.iter_mut() {
            if let Value::Object(ref mut setting_obj) = setting_value {
                if let Some(Value::Object(commands)) = setting_obj.get_mut("commands") {
                    *commands = commands
                        .iter()
                        .filter(|(k, _)| k.as_str() == env)
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                }
            }
        }
    }
    serde_json::to_string_pretty(&data)
}

pub fn find_best_match(prompt: &str, json_str: &str) -> Option<String> {
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

pub async fn update_json_current_value(
    username: &str,
    base_command: &str,
    new_value_str: &str,
    encryption_instance: State<'_, EncryptionClientInstance>,
    state: State<'_, GenerateState>,
) -> Result<(), String> {
    let encryption_client = encryption_instance.0.lock().await;
    let client = Client::new();

    let current_full_json = state.get_full_json().await;
    let mut config: AppConfig = serde_json::from_str(&current_full_json)
        .map_err(|e| format!("Could not parse preferences into AppConfig: {}", e))?;

    let mut found_match = false;
    for (_key, setting) in config.iter_mut() {
        if setting.commands.gnome.trim() == base_command.trim() {
            let new_val: DefaultValue = parse_new_value(new_value_str, &setting.current);
            setting.current = new_val;
            found_match = true;
            println!("Updated command '{}': current is now '{}'", base_command, new_value_str);
            break;
        }
    }

    if !found_match {
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
        .json(&encrypted_payload?)
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

pub async fn gather_valid_commands_for_env(
    state: &crate::state::GenerateState,
    env: &str,
) -> Result<std::collections::HashSet<String>, String> {
    let full_json = state.get_full_json().await;
    if full_json.is_empty() {
        return Ok(std::collections::HashSet::new());
    }

    let parsed: Value = match serde_json::from_str(&full_json) {
        Ok(val) => val,
        Err(e) => {
            return Err(format!("Unable to parse full JSON in gather_valid_commands_for_env: {}", e));
        }
    };

    let mut valid_commands = std::collections::HashSet::new();
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

fn preprocess_text(text: &str) -> std::collections::HashSet<String> {
    let stopwords: std::collections::HashSet<&str> = [
        "the", "is", "to", "a", "and", "for", "on", "in", "of", "with", 
        "set", "enable", "disable"
    ]
    .iter()
    .cloned()
    .collect();

    let stemmer = rust_stemmers::Stemmer::create(rust_stemmers::Algorithm::English);

    text.to_lowercase()
        .split_whitespace()
        .filter(|word| !stopwords.contains(*word))
        .map(|word| stemmer.stem(word).to_string())
        .collect()
}

fn cosine_similarity(set1: &std::collections::HashSet<String>, set2: &std::collections::HashSet<String>) -> f64 {
    let intersection = set1.intersection(set2).count() as f64;
    let norm1 = set1.len() as f64;
    let norm2 = set2.len() as f64;

    if norm1 == 0.0 || norm2 == 0.0 {
        return 0.0;
    }
    intersection / (norm1.sqrt() * norm2.sqrt())
}
