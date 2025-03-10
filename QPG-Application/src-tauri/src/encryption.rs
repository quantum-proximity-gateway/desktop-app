use serde::{Serialize, Deserialize};
use base64::prelude::*;
use uuid::Uuid;
use reqwest::Client;
use serde_json::json;
use orion::hazardous::kem::mlkem512::{MlKem512, EncapsulationKey};
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng}, Aes256Gcm, Key, Nonce // Or `Aes128Gcm`
};



#[derive(Serialize, Deserialize)]
pub struct EncapsulationResult {
    pub ciphertext_b64: String,
    pub secret: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SharedSecretInput {
    pub client_id: String,
    pub public_key_b64: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EncryptionInput {
    pub plaintext: String,
    pub public_key_b64: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EncryptionResult {
    pub plaintext: String,
    pub public_key_b64: String
}

pub struct EncryptionClient {
    pub shared_secret: Vec<u8>,
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EncryptedData {
    pub ciphertext_b64: String,
    pub nonce_b64: String,
    pub client_id: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DecryptData {
    pub ciphertext_b64: String,
    pub nonce_b64: String,
}

impl EncryptionClient {

    pub async fn new(server_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client_id = Uuid::new_v4().to_string();
	    let public_key_b64 = Self::initiate_kem(&client_id, server_url).await?;

        let data = SharedSecretInput {
            client_id: client_id.clone(),
            public_key_b64,
        };

        let results: EncapsulationResult = Self::generate_shared_secret(data)?;

        Self::complete_kem(&client_id, &results.ciphertext_b64, server_url).await?;
        
        Ok(Self {
            shared_secret: results.secret,
            client_id,
        })
    }

    pub async fn initiate_kem(client_id: &str, server_url: &str) -> Result<String, String> {
        let client = Client::new();
        let response = client.post(format!("{}/kem/initiate", server_url))
            .json(&json!({
                "client_id": client_id,
            }))
            .send()
            .await.map_err(|e| e.to_string())?;

        if response.status().is_success() {
            println!("Request successful");
            let json_response = response.json::<serde_json::Value>().await.map_err(|e| e.to_string())?;
            let public_key_b64 = json_response["public_key_b64"].as_str().ok_or("public_key_b64 not found in response")?.to_string();
            println!("{}", public_key_b64);
            return Ok(public_key_b64);
        } else {
            println!("Request failed with status: {}", response.status());
            Err("Error occured whilst initiating KEM exchange".to_string())
        }
    }

    pub async fn complete_kem(client_id: &str, ciphertext_b64: &str, server_url: &str) -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new();
        let response = client.post(format!("{}/kem/complete", server_url))
            .json(&json!({
                "client_id": client_id,
                "ciphertext_b64": ciphertext_b64
            }))
            .send()
            .await?;

        if response.status().is_success() {
            println!("{:?}", response.text().await?);
            println!("Request successful");
        } else {
            println!("Request failed with status: {}", response.status());
        }

        Ok(())
    }

    pub fn generate_shared_secret(data: SharedSecretInput) -> Result<EncapsulationResult, String> {
        let pk_bytes: Vec<u8> = BASE64_STANDARD.decode(data.public_key_b64).map_err(|e| format!("Failed to decode base64: {:?}", e))?;
        let ek = EncapsulationKey::from_slice(&pk_bytes).map_err(|e| format!("Failed to create encapsulation key: {:?}", e))?;
        let (secret, ciphertext) = MlKem512::encap(&ek).map_err(|e| format!("Failed to encapsulate: {:?}", e))?;
        let result = EncapsulationResult {
            ciphertext_b64: BASE64_STANDARD.encode(ciphertext),
            secret: secret.unprotected_as_bytes().to_vec()
        };
        Ok(result)
    }

    pub fn encrypt_data(&self, data: &str) -> Result<EncryptedData, String> {
        let key = Key::<Aes256Gcm>::from_slice(&self.shared_secret);
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(OsRng);
        let ciphertext = cipher.encrypt(&nonce, data.as_bytes())
            .map_err(|e| format!("Encryption error: {}", e))?;
        let nonce_b64 = BASE64_STANDARD.encode(&nonce);
        Ok(EncryptedData {
            ciphertext_b64: BASE64_STANDARD.encode(&ciphertext),
            nonce_b64,
            client_id: self.client_id.clone()
        })
    }

    pub fn decrypt_data(&self, data: DecryptData) -> Result<String, String> {
        let key = Key::<Aes256Gcm>::from_slice(&self.shared_secret);
        let cipher = Aes256Gcm::new(&key);
        let nonce_vec = BASE64_STANDARD.decode(&data.nonce_b64)
            .map_err(|e| format!("Failed to decode nonce: {}", e))?;
        let ciphertext = BASE64_STANDARD.decode(&data.ciphertext_b64)
            .map_err(|e| format!("Failed to decode ciphertext: {}", e))?;
        
        let nonce = Nonce::from_slice(&nonce_vec);
        let plaintext = cipher.decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| format!("Decryption error: {}", e))?;
        
        let plaintext_str = String::from_utf8(plaintext)
            .map_err(|e| format!("Invalid UTF-8: {}", e))?;
            
        Ok(plaintext_str)
    }

}
