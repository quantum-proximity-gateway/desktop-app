use tauri::{Emitter, Listener, Manager};
use std::time::Duration;
mod commands;
mod preferences;
mod state;
mod encryption;
mod models;

pub use commands::{
    execute_command, execute_startup_app_command, fetch_preferences,
    generate, get_username, init_startup_commands,
    init_startup_apps, list_models, check_encryption_client
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(move |app| {
            let app_handle = app.app_handle().clone();
            
            app.manage(state::GenerateState::default());
            
            let encryption_client = tauri::async_runtime::block_on(async {
                match encryption::EncryptionClient::new(preferences::SERVER_URL).await {
                    Ok(client) => {
                        println!("EncryptionClient created successfully!");
                        client
                    }
                    Err(e) => {
                        eprintln!("Failed to create EncryptionClient: {}", e);
                        app_handle.emit("encryption-offline", "Encryption service is offline").unwrap();
                        encryption::EncryptionClient::offline()
                    }
                }
            });
            app.manage(state::EncryptionClientInstance(tauri::async_runtime::Mutex::new(
                encryption_client
            )));
            
            // Now that the required state is managed, run the startup commands.
            let handle = app.app_handle().clone();
            tauri::async_runtime::block_on(async move {
                let encryption_instance = handle.state::<state::EncryptionClientInstance>();
                let generate_state = handle.state::<state::GenerateState>();
                
                if let Err(err) = commands::startup::init_startup_commands(
                    handle.clone(),
                    encryption_instance.clone(),
                    generate_state.clone(),
                )
                .await {
                    eprintln!("Failed to run startup init: {}", err);
                }
            });

	    let listener_handle = app.app_handle().clone();
            listener_handle.listen("frontend-loaded", {
                let captured_handle = listener_handle.clone();

		move |_event| {
		    let handle_for_spawn = captured_handle.clone();
		
                    tauri::async_runtime::spawn(async move {
			tokio::time::sleep(Duration::from_secs(5)).await;
			let generate_state = handle_for_spawn.state::<state::GenerateState>().clone();
			if let Err(err) = commands::startup::init_startup_apps(
                            handle_for_spawn.clone(),
                            generate_state,
			)
			.await {
			    eprintln!("Failed to run startup apps init: {}", err);
			}
		    });
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state::OllamaInstance(tauri::async_runtime::Mutex::new(
            ollama_rs::Ollama::new_with_history_from_url(
                url::Url::parse(preferences::OLLAMA_BASE_URL).unwrap(),
                50,
            )
        )))
        .manage(state::ChatIDs(tauri::async_runtime::Mutex::new(std::collections::HashMap::new())))
        .manage(state::GenerateState::default())
        .invoke_handler(tauri::generate_handler![
            list_models,
            init_startup_commands,
	    init_startup_apps,
            generate,
            fetch_preferences,
            execute_command,
	    execute_startup_app_command,
            get_username,
            check_encryption_client
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
