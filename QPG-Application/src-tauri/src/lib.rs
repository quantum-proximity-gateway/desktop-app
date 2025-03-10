use tauri::async_runtime::block_on;
use tauri::Manager;

mod commands;
mod preferences;
mod state;
mod encryption;
mod models;

// Re-export commands so Tauri's `generate_handler!` can see them easily
pub use commands::{
    execute_command, fetch_preferences, generate, get_username,
    init_startup_commands, list_models,
};

// Tauri plugin initialization and main entry
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(move |app| {
            let handle = app.app_handle();

            // Run startup commands asynchronously
            tauri::async_runtime::block_on(async move {
                let encryption_instance = handle.state::<state::EncryptionClientInstance>();
                let generate_state = handle.state::<state::GenerateState>();

                if let Err(err) = commands::startup::init_startup_commands(
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

                let autostart_manager = app.autolaunch();
                let _ = autostart_manager.enable();
                println!("registered for autostart? {}", autostart_manager.is_enabled().unwrap());
                let _ = autostart_manager.disable();
            }

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        // Manage the encryption client
        .manage(state::EncryptionClientInstance(tauri::async_runtime::Mutex::new(
            block_on(async {
                match encryption::EncryptionClient::new(preferences::SERVER_URL).await {
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
        // Manage the Ollama instance
        .manage(state::OllamaInstance(tauri::async_runtime::Mutex::new(
            ollama_rs::Ollama::new_with_history_from_url(
                url::Url::parse(preferences::OLLAMA_BASE_URL).unwrap(),
                50,
            )
        )))
        // Keep track of seen chat IDs
        .manage(state::ChatIDs(tauri::async_runtime::Mutex::new(std::collections::HashMap::new())))
        // Manage custom state
        .manage(state::GenerateState::default())
        // Register your Tauri commands here
        .invoke_handler(tauri::generate_handler![
            list_models,
            init_startup_commands,
            generate,
            fetch_preferences,
            execute_command,
            get_username
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
