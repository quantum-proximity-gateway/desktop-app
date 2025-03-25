use tauri::{AppHandle, State};
use crate::state::{OllamaInstance, ChatIDs, GenerateState, EncryptionClientInstance};
use crate::models::{ChatRequest, GenerateResult, ModelResponse};
use crate::preferences::{update_json_current_value, gather_valid_commands_for_env};
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use ollama_rs::generation::chat::request::ChatMessageRequest;
use serde_json::{Value};
use tauri_plugin_shell::ShellExt;

pub async fn generate_impl(
    request: ChatRequest,
    encryption_instance: State<'_, EncryptionClientInstance>,
    g_ollama: State<'_, OllamaInstance>,
    seen_chats: State<'_, ChatIDs>,
    app_handle: AppHandle,
    state: State<'_, GenerateState>
) -> Result<GenerateResult, String> {
    let username = state.get_username(&app_handle).await;
    let platform_info = state.get_platform_info().await;

    if state.get_full_json().await.is_empty() {
	println!("[generate] Full JSON empty; fetching preferences from server...");
	
	crate::preferences::fetch_preferences_impl(
            &username,
            &encryption_instance,
            &platform_info,
            &state
	)
	    .await
	    .map_err(|err| format!("Failed to automatically fetch preferences: {}", err))?;
    }

    let filtered_json = state.get_filtered_json().await;
    let mut ollama = g_ollama.0.lock().await;
    let mut seen_chats = seen_chats.0.lock().await;

    if !seen_chats.contains_key(&request.chat_id) {
        seen_chats.insert(request.chat_id.clone(), true);

        let sys_prompt = format!(
            r#"
You're an assistant that only replies in JSON format with keys "message" and "command".
It is very important that you stick to the following JSON format.

Your main job is to act as a computer accessibility coach that will reply to queries with a JSON
that has the following keys:
- "message": Something you want to say to the user
- "command": A gsettings accessibility command to run

Below is a reference JSON that shows possible accessibility commands 
for the current environment ({}): 

{}

The prompt will always begin with a snippet of the reference JSON that is the most
likely command the user is referring to. You will need to add a value to the end 
of the command found in the "command" field, and use "current" to help you figure 
out how to decide this new value. Remember, always reply with just the final JSON object, like:

{{
  "message": "...",
  "command": "..."
}}
"#,
            platform_info, filtered_json
        );

        if let Err(e) = ollama
            .send_chat_messages_with_history(
                ChatMessageRequest::new(
                    request.model.clone(),
                    vec![ChatMessage::system(sys_prompt)]
                ),
                request.chat_id.clone()
            )
            .await
        {
            return Err(format!("Failed to send initial chat message: {}", e));
        }
    }

    let best_match = crate::preferences::find_best_match(&request.prompt, &filtered_json);
    println!("Best match for prompt '{}': {:?}", request.prompt, best_match);

    let best_match_json = match best_match {
        Some(ref key) => {
            let parsed_json: Value = serde_json::from_str(&filtered_json).unwrap_or(Value::Null);

            if let Value::Object(mut root) = parsed_json {
                if let Some(matching_value) = root.remove(key) {
                    let mut new_obj = serde_json::Map::new();
                    new_obj.insert(key.clone(), matching_value);

                    let snippet = serde_json::to_string_pretty(&Value::Object(new_obj))
                        .unwrap_or_else(|_| filtered_json.clone());

                    state.set_best_match_json(&snippet).await;
                    snippet
                } else {
                    filtered_json.clone()
                }
            } else {
                filtered_json.clone()
            }
        }
        None => {
            let old_snippet = state.get_best_match_json().await;
            if !old_snippet.is_empty() {
                old_snippet
            } else {
                filtered_json.clone()
            }
        }
    };

    let user_prompt = format!("{}\n\n {}", best_match_json, request.prompt);

    match ollama
        .send_chat_messages_with_history(
            ChatMessageRequest::new(
                request.model, 
                vec![ChatMessage::user(user_prompt)]
            ),
            request.chat_id,
        )
        .await
    {
        Ok(mut res) => {
            println!("Received initial response: {:?}", res);
            let response_str = res
                .message
                .as_ref()
                .map(|m| m.content.clone())
                .unwrap_or_default();

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

pub async fn execute_command_impl(
    command: String,
    update: bool,
    app_handle: AppHandle,
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
        return Err(format!("Unrecognized/unauthorized command base: '{}'. Will not execute command.", base_cmd_str));
    }

    println!("Attempting to run shell command: {}", command);
    let shell = app_handle.shell();
    let username = match super::get_username(app_handle.clone()).await {
        Ok(username) => username,
        Err(e) => {
            println!("Warning: failed to get username from whoami: {}", e);
            return Err("Failed to fetch username.".to_string());
        }
    };

    match shell.command(&base_parts[0]).args(&base_parts[1..]).arg(last_value).output().await {
        Ok(output) => {
            if output.status.success() {
                let stdout_str = String::from_utf8(output.stdout).unwrap_or_default();
                println!("Command result: {:?}", stdout_str);

                if update {
                    if let Err(err) = update_json_current_value(
                        &username,
                        &base_cmd_str,
                        last_value,
                        encryption_instance,
                        state,
                    ).await {
                        println!("Warning: error updating JSON current value: {}", err);
                    }
                }
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

pub async fn execute_command_app_impl(
    command: String,
    app_handle: AppHandle,
    state: State<'_, GenerateState>,
) -> Result<(), String> {
    if !command.ends_with(" &") {
	return Err("Startup app command must end w/ ' &'".to_string());
    }

    let command_base = command.trim_end_matches(" &").trim().to_string();
    let startup_apps = state.get_startup_apps().await;

    if !startup_apps.contains(&command_base) {
	return Err(format!("Unrecognized/unauthorized command base: '{}'. Will not execute command.", command_base));
    }

    println!("Attempting to run shell command: {}", command);
    let shell = app_handle.shell();

    match shell.command(&command_base).arg("&").output().await {
	Ok(output) => {
	    if output.status.success() {
		let stdout_str = String::from_utf8(output.stdout).unwrap_or_default();
		println!("Command succeeded, stdout: {:?}", stdout_str)
	    } else {
		println!("Exit with code: {}", output.status.code().unwrap_or_default());
	    }
	}
	Err(e) => {
            println!("Failed to execute command: {} with error {}", command, e);	    
	}
    }
    println!("[execute_startup_app_command] Command executed: {}", command);

    Ok(())
}
