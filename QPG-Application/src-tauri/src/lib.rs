// use ollama_rs::generation::chat::MessageRole;
use url::Url;
// use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse};
// use ollama_rs::Ollama;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;
use tokio::sync::Mutex as TokioMutex;
use tauri_plugin_shell::ShellExt;
use reqwest::Client;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::{AddBos, Special};
use llama_cpp_2::sampling::LlamaSampler;
use std::io::Write;

// const OLLAMA_BASE_URL: &str = "http://localhost:11434";
const SERVER_URL: &str = "http://127.0.0.1:8000";

// struct OllamaInstance(TokioMutex<Ollama>);
// struct ChatIDs(TokioMutex<HashMap<String, bool>>);

pub struct LlamaGenerator {
    backend: LlamaBackend,
    model: LlamaModel,
}

impl LlamaGenerator {
    /// Loads the model from the given path and returns a reusable generator.
    pub fn new(model_path: &str) -> Self {
        let backend = LlamaBackend::init().unwrap();
        let params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, model_path, &params)
            .expect("unable to load model");
        Self { backend, model }
    }

    /// Generates text from the given prompt.
    ///
    /// This method creates a new context for each prompt, tokenizes the prompt, and then
    /// iteratively decodes tokens until either an end-of-stream token is produced or a fixed
    /// token limit is reached.
    pub fn generate(&self, prompt: &str) -> String {
        let ctx_params = LlamaContextParams::default();
        let mut ctx = self.model.new_context(&self.backend, ctx_params)
            .expect("unable to create the llama_context");

        // Tokenize the input prompt.
        let tokens_list = self.model
            .str_to_token(prompt, AddBos::Always)
            .unwrap_or_else(|_| panic!("failed to tokenize {}", prompt));

        // Set a fixed generation length (here: 64 tokens)
        let n_len = 64;
        let mut batch = LlamaBatch::new(512, 1);
        let last_index = tokens_list.len() as i32 - 1;

        // Prepare the batch with the prompt tokens.
        for (i, token) in (0_i32..).zip(tokens_list.into_iter()) {
            let is_last = i == last_index;
            batch.add(token, i, &[0], is_last).unwrap();
        }
        ctx.decode(&mut batch).expect("llama_decode() failed");

        let mut n_cur = batch.n_tokens();
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut sampler = LlamaSampler::greedy();
        let mut output = String::new();

        // Generate tokens until reaching the desired length or an EOS token.
        while n_cur <= n_len {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token);

            // Stop if end-of-stream token is reached.
            if token == self.model.token_eos() {
                break;
            }

            let output_bytes = self.model
                .token_to_bytes(token, Special::Tokenize)
                .expect("token_to_bytes failed");

            let mut output_string = String::with_capacity(32);
            let _ = decoder.decode_to_string(&output_bytes, &mut output_string, false);
            output.push_str(&output_string);

            batch.clear();
            batch.add(token, n_cur, &[0], true).unwrap();
            n_cur += 1;
            ctx.decode(&mut batch).expect("failed to eval");
        }

        output
    }
}

// #[tauri::command]
// async fn list_models() -> Result<Vec<String>, String> {
//     let ollama = Ollama::new_with_history_from_url(
//         Url::parse(OLLAMA_BASE_URL).unwrap(),
//         50,
//     );
//     let default_model_name = "granite3-dense:8b".to_string();

//     let local_models = match ollama.list_local_models().await {
//         Ok(res) => res,
//         Err(e) => return Err(format!("Failed to list models: {}", e)),
//     };

//     if local_models.is_empty() { // Download model in the case that it does not exist
//         println!("No local models found. Pulling {}...", default_model_name);
//         if let Err(e) = ollama.pull_model(default_model_name.into(), false).await {
//             return Err(format!("Failed to pull model: {}", e));
//         }
//     }

//     let updated_models = match ollama.list_local_models().await {
//         Ok(res) => res,
//         Err(e) => return Err(format!("Failed to list models: {}", e)),
//     };

//     let models: Vec<String> = updated_models.into_iter().map(|model| model.name).collect();
//     Ok(models)
// }

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

fn parse_model_response(json_str: String) -> Result<ModelResponse, serde_json::Error> {
    // Sometimes the model might not respond in the right format, we need to think of a way to handle that.
    let parsed_response: ModelResponse = serde_json::from_str(&json_str)?;
    Ok(parsed_response)
}

#[derive(Serialize)]
struct GenerateResult {
    model_response: String,
    command: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PreferencesAPIResponse {
    preferences: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct UpdateJSONPreferencesRequest {
    username: String,
    preferences: AppConfig,
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

#[tauri::command]
async fn generate(
    request: ChatRequest,
    app_handle: tauri::AppHandle,
) -> Result<GenerateResult, String> {
    println!("Generating response for {:?}", request);

    // // Fetch username using whoami
    // let username = match get_username(app_handle.clone()).await {
    //     Ok(username) => username,
    //     Err(e) => {
    //         println!("Warning: failed to get username from whoami: {}", e);
    //         return Err("Failed to fetch username.".into());
    //     }
    // };
    // println!("Username: {:?}", username);

    // // Fetch preferences from the server
    // let client = Client::new();
    // let url = format!("{}/preferences/{}", SERVER_URL, username);
    // let response = client
    //     .get(&url)
    //     .send()
    //     .await
    //     .map_err(|e| format!("Failed to fetch preferences: {}", e))?;
    // println!("HTTP response: {:?}", response.status());
    // if !response.status().is_success() {
    //     return Err(format!("Failed to fetch preferences: {}", response.status()));
    // }
    // let response_body = response
    //     .text()
    //     .await
    //     .map_err(|e| format!("Failed to read response body: {}", e))?;
    // println!("Response body: {}", response_body);

    // let preferences: PreferencesAPIResponse = serde_json::from_str(&response_body)
    //     .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    // let json_example = preferences.preferences.to_string();
    let json_example = r#"
{
  "zoom": {
    "lower_bound": 0.5,
    "upper_bound": 3.0,
    "default": 1.0,
    "current": 1.0,
    "commands": {
      "windows": "",
      "macos": "",
      "gnome": "gsettings set org.gnome.desktop.interface text-scaling-factor"
    }
  },
  "on_screen_keyboard": {
    "lower_bound": null,
    "upper_bound": null,
    "default": false,
    "current": false,
    "commands": {
      "windows": "",
      "macos": "",
      "gnome": "gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled"
    }
  },
  "magnifier": {
    "lower_bound": 0.1,
    "upper_bound": 32.0,
    "default": 1.0,
    "current": 1.0,
    "commands": {
      "windows": "",
      "macos": "",
      "gnome": "gsettings set org.gnome.desktop.a11y.magnifier mag-factor"
    }
  },
  "enable_animation": {
    "lower_bound": null,
    "upper_bound": null,
    "default": true,
    "current": true,
    "commands": {
      "windows": "",
      "macos": "",
      "gnome": "gsettings set org.gnome.desktop.interface enable-animations"
    }
  },
  "screen_reader": {
    "lower_bound": null,
    "upper_bound": null,
    "default": false,
    "current": false,
    "commands": {
      "windows": "",
      "macos": "",
      "gnome": "gsettings set org.gnome.desktop.a11y.applications screen-reader-enabled"
    }
  },
  "cursor_size": {
    "lower_bound": 0.0,
    "upper_bound": 128.0,
    "default": 24.0,
    "current": 24.0,
    "commands": {
      "windows": "",
      "macos": "",
      "gnome": "gsettings set org.gnome.desktop.interface cursor-size"
    }
  },
  "font_name": {
    "lower_bound": null,
    "upper_bound": null,
    "default": "Cantarell 11",
    "current": "Cantarell 11",
    "commands": {
      "windows": "p",
      "macos": "",
      "gnome": "gsettings set org.gnome.desktop.interface font-name"
    }
  },
  "locate_pointer": {
    "lower_bound": null,
    "upper_bound": null,
    "default": false,
    "current": false,
    "commands": {
      "windows": "",
      "macos": "",
      "gnome": "gsettings set org.gnome.desktop.interface locate-pointer"
    }
  }
}
    "#;

    // Build the system prompt that instructs the model
    let sys_prompt = format!(
        r#""You're an assistant that only replies in JSON format with keys "message" and "command".
It is very important that you stick to the following JSON format.

Your main job is to act as a computer accessibility coach that will reply to queries with a JSON
that has the following keys:
- "message": Something you want to say to the user
- "command": A gsettings accessibility command to run

Below is a reference JSON that shows possible accessibility commands for GNOME:

{}

The "current" field is the current value on the computer, while the "lower_bound",
"upper_bound", and "default" fields represent the ranges/values in gsettings.
Use this reference to inform your responses if needed. However, always reply with just
the final JSON object, like:

{{
  "message": "...",
  "command": "..."
}}"#,
        json_example
    );

    // Combine the system prompt and user prompt into one conversation prompt.
    // The format here follows your example with special tokens.
    let combined_prompt = format!(
        "<|im_start|>system\n{}\n<|im_end|>\n<|im_start|>user\n{}\n<|im_end|>\n<|im_start|>assistant\n",
        sys_prompt, request.prompt
    );
    println!("Combined prompt: {}", combined_prompt);

    // Create a new LlamaGenerator instance (model path is hardcoded per your example)
    let generator = LlamaGenerator::new("models/granite-3.0-8b-instruct-IQ4_XS.gguf");

    // Generate output synchronously using your generator.
    let output = generator.generate(&combined_prompt);
    println!("\nFinal output: {}", output);

    // Parse the model's JSON output into your expected ModelResponse.
    match parse_model_response(output) {
        Ok(parsed_response) => {
            Ok(GenerateResult {
                model_response: parsed_response.message,
                command: Some(parsed_response.command),
            })
        }
        Err(e) => Err(format!("Failed to parse model response: {}", e)),
    }
}

async fn update_json_current_value(
    username: &str,
    base_command: &str,
    new_value_str: &str,
) -> Result<(), String> {
    let client = Client::new();
    let get_url = format!("{}/preferences/{}", SERVER_URL, username);
    let response = client
        .get(&get_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch preferences: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch preferences: status {}",
            response.status()
        ));
    }

    let prefs_resp = response
        .json::<PreferencesAPIResponse>()
        .await
        .map_err(|e| format!("Failed to parse preferences JSON: {}", e))?;

    let mut config: AppConfig = serde_json::from_value(prefs_resp.preferences)
        .map_err(|e| format!("Could not parse preferences into AppConfig: {}", e))?;

    let mut found_match = false;
    for (key, setting) in config.iter_mut() {
        if setting.commands.gnome.trim() == base_command.trim() {
            let new_val: DefaultValue = parse_new_value(new_value_str, &setting.default);
            setting.current = Some(new_val);

            found_match = true;
            println!("Updated '{}': current is now '{}'", key, new_value_str);
            break;
        }
    }

    if !found_match { // no match in json for command
        println!("No command exists for: {}", base_command);
        return Ok(());
    }

    let update_payload = UpdateJSONPreferencesRequest {
        username: username.to_string(),
        preferences: config,
    };

    let post_url = format!("{}/preferences/update", SERVER_URL);
    let update_resp = client
        .post(&post_url)
        .json(&update_payload)
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

#[tauri::command]
async fn execute_command(
    command: String,
    app_handle: tauri::AppHandle
) -> Result<(), String> {
    // execute shell command https://v2.tauri.app/plugin/shell/
    let shell = app_handle.shell();
    
    let username = match get_username(app_handle.clone()).await {
        Ok(username) => username,
        Err(e) => {
            println!("Warning: failed to get username from whoami: {}", e);
	    return Err(format!("Failed to fetch username."));
        }
    };

    println!("Attempting to run shell command: {}", command);

    let command_parts: Vec<&str> = command.split_whitespace().collect();
    if let Some((cmd, args)) = command_parts.split_first() {
	match shell.command(cmd).args(args).output().await { // so unsafe we need to whitelist only gsettings
            Ok(output) => {
                if output.status.success() {
		    let stdout_str = String::from_utf8(output.stdout).unwrap_or_else(|_| "".to_string());
                    println!("Command result: {:?}", stdout_str);

                    if !args.is_empty() {
                        let new_value_str = args.last().unwrap().to_string();
                        let base_command_str = {
                            let without_last = &args[..args.len() - 1];
                            format!("{} {}", cmd, without_last.join(" "))
                        };

                        update_json_current_value(
                            &username,
                            &base_command_str,
                            &new_value_str,
                        )
                        .await?;
	        	    }
                } else {
                    println!("Exit with code: {}", output.status.code().unwrap());
                }
            }
            Err(e) => {
                println!("Failed to execute command: {} with error {}", command, e);
            }
        }
    } else {
	match shell.command(command.clone()).output().await { // so unsafe we need to whitelist only gsettings
            Ok(output) => {
                if output.status.success() {
                    println!("Command result: {:?}", String::from_utf8(output.stdout));
                } else {
                    println!("Exit with code: {}", output.status.code().unwrap());
                }
            }
            Err(e) => {
                println!("Failed to execute command: {} with error {}", command, e);
            }
        }
    }
    
    // we will need to save new command settings here
    println!("Command executed: {}", command);

    Ok(())
}


#[derive(Debug, Serialize, Deserialize)]
struct Commands {
    windows: String,
    macos: String,
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
    commands: Commands,
}

pub type AppConfig = HashMap<String, Setting>;

#[tauri::command]
async fn fetch_preferences(app_handle: tauri::AppHandle) -> Result<AppConfig, String> {
    use std::fs;
    use serde_json;

    fn load_default_app_config(path: &str) -> Result<AppConfig, Box<dyn std::error::Error>> {
        let file_contents = fs::read_to_string(path)?;
        let config: AppConfig = serde_json::from_str(&file_contents)?;
        Ok(config)
    }

    let default_commands = match load_default_app_config("src/json_example.json") {
        Ok(config) => config,
        Err(e) => {
            println!("Failed to load defaults: {}", e);
            HashMap::new()
        }
    };

    let username = match get_username(app_handle.clone()).await {
        Ok(username) => username,
        Err(e) => {
            println!("Warning: failed to get username from whoami: {}", e);
	    return Err(format!("Failed to fetch username."));
        }
    };

    let api_url = format!("{}/devices/{}/preferences", SERVER_URL, username);
    let client = Client::new();

    match client.get(api_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<AppConfig>().await {
                    Ok(preferences) => Ok(preferences),
                    Err(e) => {
                        println!("Failed to parse preferences: {}", e);
                        Ok(default_commands) // Placeholder data
                    }
                }
            } else {
                println!("Failed to fetch preferences. Status: {}", response.status());
                Ok(default_commands) // Placeholder data
            }
        }
        Err(e) => {
            println!("HTTP request failed: {}", e);
            Ok(default_commands) // Placeholder data
        }
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
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
        .invoke_handler(tauri::generate_handler![generate, fetch_preferences, execute_command, get_username])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
