use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ollama_rs::generation::chat::{ChatMessageResponse};

/// Command from the user to begin a new prompt
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub prompt: String,
    pub chat_id: String,
}

/// The type of response your model is expected to produce
#[derive(Serialize, Deserialize, Debug)]
pub struct ModelResponse {
    pub message: String,
    pub command: String,
}

/// The result struct returned from your Tauri command
#[derive(Serialize)]
pub struct GenerateResult {
    pub ollama_response: ChatMessageResponse,
    pub command: Option<String>,
}

/// The shape of your preferences JSON
pub type AppConfig = HashMap<String, Setting>;

/// Wraps a single setting in your config
#[derive(Debug, Serialize, Deserialize)]
pub struct Setting {
    #[serde(default)]
    pub lower_bound: Option<f32>,
    #[serde(default)]
    pub upper_bound: Option<f32>,
    pub current: DefaultValue,
    #[serde(default)]
    pub commands: Commands,
}

/// Commands for each environment
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Commands {
    #[serde(default)]
    pub windows: String,
    #[serde(default)]
    pub macos: String,
    #[serde(default)]
    pub gnome: String,
}

/// The “current” field’s type
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DefaultValue {
    Float(f32),
    Bool(bool),
    String(String),
}

/// Request body used when updating preferences on the server
#[derive(Serialize, Deserialize)]
pub struct UpdateJSONPreferencesRequest {
    pub username: String,
    pub preferences: AppConfig,
}
