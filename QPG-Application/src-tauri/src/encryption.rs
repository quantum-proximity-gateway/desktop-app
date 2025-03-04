use serde::{Serialize, Deserialize};
use base64::prelude::*;
use kyberlib::*;
use uuid::Uuid;
use reqwest::Client;
use serde_json::json;

const SERVER_URL: &str = "http://127.0.0.1:8000";

#[derive(Serialize, Deserialize)]
pub struct EncapsulationResult {
    ciphertext_b64: String,
    secret: Box<[u8]>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SharedSecretInput {
    client_id: String,
    public_key_b64: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EncryptionInput {
    plaintext: String,
    public_key_b64: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EncryptionResult {
    plaintext: String,
    public_key_b64: String
}

pub struct EncryptionClient {
    shared_secret: Box<[u8]>,
    pub client_id: String,
}

impl EncryptionClient {

    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let client_id = Uuid::new_v4().to_string();

        let data = SharedSecretInput {
            client_id: client_id.clone(),
            public_key_b64: Self::initiate_kem(&client_id).await?,
        };

        let results: EncapsulationResult = Self::generate_shared_secret(data)?;
        let ciphertext_b64 = results.ciphertext_b64;

        Self::complete_kem(&client_id, ciphertext_b64).await?;
        let shared_secret = results.secret;
        
        
        Ok(Self {
            shared_secret,
            client_id,
        })
    }

    pub async fn initiate_kem(client_id: &str) -> Result<String, String> {
        let client = Client::new();
        let response = client.post(format!("{}/kem/initiate", SERVER_URL))
            .json(&json!({
                "client_id": client_id,
            }))
            .send()
            .await.map_err(|e| e.to_string())?;

        if response.status().is_success() {
            println!("Request successful");
            let json_response = response.json::<serde_json::Value>().await.map_err(|e| e.to_string())?;
            let public_key_b64 = json_response["public_key_b64"].as_str().ok_or("public_key_b64 not found in response")?.to_string();
            return Ok(public_key_b64);
        } else {
            println!("Request failed with status: {}", response.status());
            Err("Error occured whilst initiating KEM exchange".to_string())
        }
    }

    pub async fn complete_kem(client_id: &str, ciphertext_b64: String) -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new();
        let response = client.post(format!("{}/kem/complete", SERVER_URL))
            .json(&json!({
                "client_id": client_id,
                "ciphertext_b64": ciphertext_b64
            }))
            .send()
            .await?;

        if response.status().is_success() {
            println!("{:?}", response.json().await?);
            println!("Request successful");
        } else {
            println!("Request failed with status: {}", response.status());
        }

        Ok(())
    }

    pub fn generate_shared_secret(data: SharedSecretInput) -> Result<EncapsulationResult, String> {
        let pk_bytes = BASE64_STANDARD.decode(data.public_key_b64).map_err(|e| format!("Failed to decode base64: {:?}", e))?;
        let pk_boxed: Box<[u8]> = pk_bytes.into_boxed_slice();
        let encapsulation_result = encapsulate(pk_boxed).map_err(|e| format!("{:?}", e))?; // ML-KEM-512
        let ciphertext_b64: String = BASE64_STANDARD.encode(encapsulation_result.ciphertext());
    
        let result = EncapsulationResult {
            ciphertext_b64: ciphertext_b64,
            secret: encapsulation_result.sharedSecret()
        };
        Ok(result)
    }

}

