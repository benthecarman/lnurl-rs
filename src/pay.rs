use crate::Tag;
use bitcoin::hashes::sha256::Hash as Sha256;
use bitcoin::hashes::Hash;
use lightning_invoice::Invoice;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
}

impl PayResponse {
    pub fn metadata_json(&self) -> serde_json::Value {
        serde_json::from_str(&self.metadata).unwrap()
    }

    pub fn metadata_hash(&self) -> [u8; 32] {
        Sha256::hash(self.metadata.as_bytes()).into_inner()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LnURLPayInvoice {
    /// Encoded bolt 11 invoice
    pr: String,
}

impl LnURLPayInvoice {
    pub fn invoice(&self) -> Invoice {
        Invoice::from_str(&self.pr).unwrap()
    }
}
