use ollama_rs::{generation::completion::{
        request::GenerationRequest, GenerationContext
    },
    Ollama
    
};

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

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, list_models])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
