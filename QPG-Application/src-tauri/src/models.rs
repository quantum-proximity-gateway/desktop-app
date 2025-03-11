use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ollama_rs::generation::chat::{ChatMessageResponse};

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub prompt: String,
    pub chat_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ModelResponse {
    pub message: String,
    pub command: String,
}

#[derive(Serialize)]
pub struct GenerateResult {
    pub ollama_response: ChatMessageResponse,
    pub command: Option<String>,
}

pub type AppConfig = HashMap<String, Setting>;

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

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Commands {
    #[serde(default)]
    pub windows: String,
    #[serde(default)]
    pub macos: String,
    #[serde(default)]
    pub gnome: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DefaultValue {
    Float(f32),
    Bool(bool),
    String(String),
}

#[derive(Serialize, Deserialize)]
pub struct UpdateJSONPreferencesRequest {
    pub username: String,
    pub preferences: AppConfig,
}
