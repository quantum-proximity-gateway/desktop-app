Using https://crates.io/crates/llama-cpp-2

# Setup model
Download the model from Hugging Face.

```bash
wget https://huggingface.co/bartowski/granite-3.0-8b-instruct-GGUF/resolve/main/granite-3.0-8b-instruct-IQ4_XS.gguf
```

Make a directory called `models/` inside `src-tauri/`, and place the model inside.

<!-- sudo apt update
sudo apt install clang -->