use tauri::AppHandle;
use tauri::State;
use crate::preferences;
use crate::state::{EncryptionClientInstance, GenerateState};
use crate::models::{DefaultValue, Setting};
use serde_json::Value;

#[tauri::command]
pub async fn init_startup_commands(
    app_handle: AppHandle,
    encryption_instance: State<'_, EncryptionClientInstance>,
    state: State<'_, GenerateState>,
) -> Result<(), String> {
    let username = state.get_username(&app_handle).await;
    let platform_info = state.get_platform_info().await;
    println!("[startup_init] username = {}, platform_info = {}", username, platform_info);

    if state.get_full_json().await.is_empty() {
        println!("[startup_init] Full JSON empty; fetching preferences from server...");
        preferences::fetch_preferences_impl(
	    &username,
	    &encryption_instance,
	    &platform_info,
	    &state
	).await?;
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

                let value_str = match &setting.current {
                    DefaultValue::Bool(b) => b.to_string(),
                    DefaultValue::Float(f) => f.to_string(),
                    DefaultValue::String(s) => s.to_string(),
                };

                let full_command = format!("{} {}", command_str.trim(), value_str);
                println!("[startup_init] Executing: {}", full_command);

                if let Err(e) = super::generation::execute_command_impl(
                    full_command,
                    false,
                    app_handle.clone(),
                    encryption_instance.clone(),
                    state.clone(),
                ).await {
                    println!("Warning: failed to run startup command. Error: {}", e);
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn init_startup_apps(
    app_handle: AppHandle,
    state: State<'_, GenerateState>,
) -> Result<(), String> {
    let startup_apps = state.get_startup_apps().await;
    println!("[startup_apps_init] startup_apps = {:?}", startup_apps);

    for app in startup_apps {
	let full_command = format!("{} &", app);
	println!("[startup_apps_init] Launching: {}", full_command);

	if let Err(e) = super::generation::execute_command_app_impl(
	    full_command,
	    app_handle.clone(),
	    state.clone(),
	).await {
	    println!("Warning: failed to run startup command. Error: {}", e);
	}
    }
    
    Ok(())
}
