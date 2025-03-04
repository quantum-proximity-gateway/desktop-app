use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::{AddBos, Special};
use llama_cpp_2::sampling::LlamaSampler;
use std::io::Write;

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

fn main() {
    // Initialize the generator once.
    let generator = LlamaGenerator::new("models/granite-3.0-8b-instruct-IQ4_XS.gguf");

    // Example prompt.
    let prompt = "<|im_start|>user\nHello! How are you?<|im_end|>\n<|im_start|>assistant\n";
    let output = generator.generate(prompt);

    println!("\nFinal output: {}", output);
}