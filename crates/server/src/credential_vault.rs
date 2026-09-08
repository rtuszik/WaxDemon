use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, Generate, KeyInit, Payload},
};
use secrecy::{ExposeSecret, SecretBox, SecretSlice, SecretString};
use std::collections::BTreeMap;
use waxdemon_db::connections::EncryptedConnection;
use waxdemon_discogs::oauth::OAuthCredentials;

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("invalid credential encryption configuration")]
    Configuration,
    #[error("credential encryption failed")]
    Encryption,
    #[error("credential decryption failed")]
    Decryption,
}

pub struct CredentialVault {
    active_key_id: String,
    keys: BTreeMap<String, SecretBox<[u8; 32]>>,
}

impl CredentialVault {
    pub fn new(
        active_key_id: String,
        keys: BTreeMap<String, SecretBox<[u8; 32]>>,
    ) -> Result<Self, CredentialError> {
        if !keys.contains_key(&active_key_id)
            || keys.keys().any(|id| {
                id.is_empty()
                    || id.len() > 64
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            })
        {
            return Err(CredentialError::Configuration);
        }
        Ok(Self {
            active_key_id,
            keys,
        })
    }

    pub fn encrypt(
        &self,
        user_id: i64,
        credentials: &OAuthCredentials,
    ) -> Result<EncryptedConnection, CredentialError> {
        if user_id <= 0 {
            return Err(CredentialError::Encryption);
        }
        let key = self
            .keys
            .get(&self.active_key_id)
            .ok_or(CredentialError::Configuration)?;
        let cipher = XChaCha20Poly1305::new_from_slice(key.expose_secret())
            .map_err(|_| CredentialError::Configuration)?;
        let nonce = XNonce::try_generate().map_err(|_| CredentialError::Encryption)?;
        let plaintext: SecretSlice<u8> = serde_json::to_vec(&[
            credentials.token().expose_secret(),
            credentials.secret().expose_secret(),
        ])
        .map_err(|_| CredentialError::Encryption)?
        .into();
        let aad = associated_data(user_id, &self.active_key_id);
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext.expose_secret(),
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| CredentialError::Encryption)?;
        Ok(EncryptedConnection {
            key_id: self.active_key_id.clone(),
            nonce: nonce.to_vec(),
            ciphertext,
        })
    }

    pub fn decrypt(
        &self,
        user_id: i64,
        connection: &EncryptedConnection,
    ) -> Result<OAuthCredentials, CredentialError> {
        if user_id <= 0 {
            return Err(CredentialError::Decryption);
        }
        let key = self
            .keys
            .get(&connection.key_id)
            .ok_or(CredentialError::Decryption)?;
        let cipher = XChaCha20Poly1305::new_from_slice(key.expose_secret())
            .map_err(|_| CredentialError::Decryption)?;
        let nonce = XNonce::try_from(connection.nonce.as_slice())
            .map_err(|_| CredentialError::Decryption)?;
        let aad = associated_data(user_id, &connection.key_id);
        let plaintext: SecretSlice<u8> = cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &connection.ciphertext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| CredentialError::Decryption)?
            .into();
        let [token, secret]: [SecretString; 2] = serde_json::from_slice(plaintext.expose_secret())
            .map_err(|_| CredentialError::Decryption)?;
        OAuthCredentials::new(token, secret).map_err(|_| CredentialError::Decryption)
    }
}

fn associated_data(user_id: i64, key_id: &str) -> String {
    format!("waxdemon:discogs-connection:v1:{user_id}:{key_id}")
}
