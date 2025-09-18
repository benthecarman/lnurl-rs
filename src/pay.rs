use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use aes::Aes256;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bitcoin::hashes::sha256::Hash as Sha256;
use bitcoin::hashes::Hash;
use bitcoin::key::XOnlyPublicKey;
use cbc::{Decryptor, Encryptor};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use url::Url;

type Aes256CbcEnc = Encryptor<Aes256>;
type Aes256CbcDec = Decryptor<Aes256>;

use crate::Tag;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PayResponse {
    /// a second-level url which give you an invoice with a GET request
    /// and an amount
    pub callback: String,
    /// max sendable amount for a given user on a given service
    #[serde(rename = "maxSendable")]
    pub max_sendable: u64,
    /// min sendable amount for a given user on a given service,
    /// can not be less than 1 or more than `max_sendable`
    #[serde(rename = "minSendable")]
    pub min_sendable: u64,
    /// tag of the request
    pub tag: Tag,
    /// Metadata json which must be presented as raw string here,
    /// this is required to pass signature verification at a later step
    pub metadata: String,

    /// Optional, if true, the service allows comments
    /// the number is the max length of the comment
    #[serde(rename = "commentAllowed")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment_allowed: Option<u32>,

    /// Optional, if true, the service allows nostr zaps
    #[serde(rename = "allowsNostr")]
    pub allows_nostr: Option<bool>,

    /// Optional, if true, the nostr pubkey that will be used to sign zap events
    #[serde(rename = "nostrPubkey")]
    pub nostr_pubkey: Option<XOnlyPublicKey>,
}

impl PayResponse {
    pub fn metadata_json(&self) -> serde_json::Value {
        serde_json::from_str(&self.metadata).unwrap()
    }

    pub fn metadata_hash(&self) -> [u8; 32] {
        Sha256::hash(self.metadata.as_bytes()).to_byte_array()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VerifyResponse {
    /// If invoice has been settled
    pub settled: bool,
    /// Pre-image of the payment request (when paid)
    pub preimage: Option<String>,
    /// Encoded bolt 11 invoice
    pub pr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LnURLPayInvoice {
    /// Encoded bolt 11 invoice
    pub pr: String,
    /// If this invoice is a hodl invoice
    pub hodl_invoice: Option<bool>,
    /// Optional, if present, can be used to display a message to the user
    /// after the payment has been completed
    #[serde(rename = "successAction")]
    #[serde(skip_serializing_if = "Option::is_none")]
    success_action: Option<SuccessActionParams>,
    /// LUD-21 verify URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify: Option<String>,
}

impl LnURLPayInvoice {
    pub fn new(invoice: String) -> Self {
        Self {
            pr: invoice,
            hodl_invoice: None,
            success_action: None,
            verify: None,
        }
    }

    pub fn invoice(&self) -> &str {
        self.pr.as_str()
    }

    pub fn success_action(&self) -> Option<SuccessAction> {
        self.success_action.clone().map(SuccessAction::from_params)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuccessAction {
    Message(String),
    Url { url: Url, description: String },
    AES(AesParams),
    Unknown(SuccessActionParams),
}

impl SuccessAction {
    pub fn tag(&self) -> &str {
        match self {
            SuccessAction::Message(_) => "message",
            SuccessAction::Url { .. } => "url",
            SuccessAction::AES(_) => "aes",
            SuccessAction::Unknown(params) => params.tag.as_str(),
        }
    }

    pub fn into_params(self) -> SuccessActionParams {
        match self {
            SuccessAction::Message(message) => SuccessActionParams {
                tag: "message".to_string(),
                message: Some(message),
                url: None,
                description: None,
                ciphertext: None,
                iv: None,
            },
            SuccessAction::Url { url, description } => SuccessActionParams {
                tag: "url".to_string(),
                message: None,
                url: Some(url),
                description: Some(description),
                ciphertext: None,
                iv: None,
            },
            SuccessAction::AES(params) => SuccessActionParams {
                tag: "aes".to_string(),
                message: None,
                url: None,
                description: Some(params.description),
                ciphertext: Some(params.ciphertext),
                iv: Some(params.iv),
            },
            SuccessAction::Unknown(params) => params,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AesParams {
    pub description: String,
    pub ciphertext: String,
    pub iv: String,
}

impl AesParams {
    pub fn new(description: String, text: &str, preimage: &[u8; 32]) -> anyhow::Result<AesParams> {
        let iv = bitcoin::secp256k1::rand::random::<[u8; 16]>();
        let cipher = Aes256CbcEnc::new(preimage.into(), &iv.into());
        let encrypted: Vec<u8> = cipher.encrypt_padded_vec_mut::<Pkcs7>(text.as_bytes());
        let ciphertext = BASE64_STANDARD.encode(encrypted);

        let iv = BASE64_STANDARD.encode(iv);
        Ok(AesParams {
            description,
            ciphertext,
            iv,
        })
    }

    pub fn decrypt(&self, preimage: &[u8; 32]) -> anyhow::Result<String> {
        // decode base64
        let iv = BASE64_STANDARD.decode(&self.iv)?;
        let ciphertext = BASE64_STANDARD.decode(&self.ciphertext)?;

        // check iv length
        if iv.len() != 16 {
            return Err(anyhow::anyhow!("iv length is not 16"));
        }
        // turn into generic array
        let iv: [u8; 16] = iv.try_into().unwrap();

        // decrypt
        let cipher = Aes256CbcDec::new(preimage.into(), &iv.into());
        let decrypted: Vec<u8> = cipher
            .decrypt_padded_vec_mut::<Pkcs7>(&ciphertext)
            .map_err(|_| anyhow::anyhow!("decryption failed"))?;

        Ok(String::from_utf8(decrypted)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SuccessActionParams {
    pub tag: String,
    pub message: Option<String>,
    pub url: Option<Url>,
    pub description: Option<String>,
    pub ciphertext: Option<String>,
    pub iv: Option<String>,
}

impl SuccessAction {
    pub fn from_params(params: SuccessActionParams) -> Self {
        match params.tag.as_str() {
            "message" => {
                if params.message.is_none() {
                    return SuccessAction::Unknown(params);
                }
                SuccessAction::Message(params.message.unwrap())
            }
            "url" => {
                if params.url.is_none() || params.description.is_none() {
                    return SuccessAction::Unknown(params);
                }
                SuccessAction::Url {
                    url: params.url.unwrap(),
                    description: params.description.unwrap(),
                }
            }
            "aes" => {
                if params.description.is_none()
                    || params.ciphertext.is_none()
                    || params.iv.is_none()
                {
                    return SuccessAction::Unknown(params);
                }

                SuccessAction::AES(AesParams {
                    description: params.description.unwrap(),
                    ciphertext: params.ciphertext.unwrap(),
                    iv: params.iv.unwrap(),
                })
            }
            _ => SuccessAction::Unknown(params),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Response;

    #[test]
    fn test_encrypt_decrypt() {
        let description = "test_description".to_string();
        let text = "hello world".to_string();
        let preimage = [1u8; 32];

        let params = AesParams::new(description.clone(), &text, &preimage).unwrap();

        let decrypted = params.decrypt(&preimage).unwrap();
        assert_eq!(decrypted, text);
    }

    #[test]
    fn test_parse_verify_settled() {
        let settled = r#"{
  "status": "OK",
  "settled": true,
  "preimage": "123456...",
  "pr": "lnbc10..."
}"#;

        let parsed = serde_json::from_str::<Response<VerifyResponse>>(settled).unwrap();
        let parsed = match parsed {
            Response::Error { .. } => panic!("failed to parse"),
            Response::Ok(p) => p,
        };
        assert!(parsed.settled);
        assert!(parsed.preimage.is_some());
        assert!(parsed.pr.starts_with("lnbc10"));
    }

    #[test]
    fn test_parse_verify_not_settled() {
        let settled = r#"{
  "status": "OK",
  "settled": false,
  "preimage": null,
  "pr": "lnbc10..."
}"#;

        let parsed = serde_json::from_str::<Response<VerifyResponse>>(settled).unwrap();
        let parsed = match parsed {
            Response::Error { .. } => panic!("failed to parse"),
            Response::Ok(p) => p,
        };
        assert!(!parsed.settled);
        assert!(parsed.preimage.is_none());
        assert!(parsed.pr.starts_with("lnbc10"));
    }
}
