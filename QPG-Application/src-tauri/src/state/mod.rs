use tauri::async_runtime::{RwLock, Mutex};
use std::collections::HashMap;
use tokio::sync::OnceCell;
use crate::encryption::EncryptionClient;
use ollama_rs::Ollama;

pub struct OllamaInstance(pub Mutex<Ollama>);
pub struct EncryptionClientInstance(pub Mutex<EncryptionClient>);
pub struct ChatIDs(pub Mutex<HashMap<String, bool>>);

pub struct GenerateState {
    username: OnceCell<String>,
    platform_info: OnceCell<String>,
    full_json_example: RwLock<String>,
    filtered_json_example: RwLock<String>,
    best_match_json_example: RwLock<String>,
    startup_apps: RwLock<Vec<String>>,
}

impl Default for GenerateState {
    fn default() -> Self {
        Self {
            username: OnceCell::new(),
            platform_info: OnceCell::new(),
            full_json_example: RwLock::new(String::new()),
            filtered_json_example: RwLock::new(String::new()),
            best_match_json_example: RwLock::new(String::new()),
	    startup_apps: RwLock::new(vec![
		"gnome-tweaks".to_string(),
		"mousepad".to_string(),
	    ]),
        }
    }
}

impl GenerateState {
    pub async fn get_username(&self, app_handle: &tauri::AppHandle) -> String {
        self.username
            .get_or_init(|| async {
                crate::commands::get_username(app_handle.clone())
                    .await
                    .unwrap_or_else(|_| "unknown_user".to_string())
            })
            .await
            .clone()
    }

    pub async fn get_platform_info(&self) -> String {
	self.platform_info
            .get_or_init(|| async {
		get_platform_info()
            })
            .await
            .clone()
    }

    pub async fn get_full_json(&self) -> String {
        self.full_json_example.read().await.clone()
    }

    pub async fn get_filtered_json(&self) -> String {
        self.filtered_json_example.read().await.clone()
    }

    pub async fn get_best_match_json(&self) -> String {
        self.best_match_json_example.read().await.clone()
    }

    pub async fn get_startup_apps(&self) -> Vec<String> {
	self.startup_apps.read().await.clone()
    }

    pub async fn set_best_match_json(&self, value: &str) {
        let mut writer = self.best_match_json_example.write().await;
        *writer = value.to_string();
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
fn get_platform_info() -> String {
    #[cfg(target_os = "macos")]
    {
        return "macos".into();
    }
    #[cfg(target_os = "windows")]
    {
        return "windows".into();
    }
    #[cfg(target_os = "linux")]
    {
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
