use bitcoin::hashes::sha256::Hash as Sha256;
use bitcoin_hashes::Hash;
use lightning_invoice::Invoice;
use serde::de::Error;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub trait LnURLResponse {
    fn tag(&self) -> Tag;
    fn callback(&self) -> String;
}

pub fn decode_ln_url_response(string: &str) -> Box<dyn LnURLResponse> {
    let json: serde_json::Value = serde_json::from_str(string).unwrap();
    decode_ln_url_response_from_json(json)
}

pub fn decode_ln_url_response_from_json(json: serde_json::Value) -> Box<dyn LnURLResponse> {
    let obj = json.as_object().unwrap();
    let tag_str = obj.get("tag").unwrap().as_str().unwrap();
    let tag = Tag::from_str(tag_str).unwrap();
    match tag {
        Tag::PayRequest => {
            let pay_response: PayResponse = serde_json::from_value(json).unwrap();
            Box::new(pay_response)
        }
        Tag::WithdrawRequest => panic!("Not implemented"),
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Tag {
    #[serde(rename = "payRequest")]
    PayRequest,
    #[serde(rename = "withdrawRequest")]
    WithdrawRequest,
}

impl FromStr for Tag {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "payRequest" => Ok(Tag::PayRequest),
            "withdrawRequest" => Ok(Tag::WithdrawRequest),
            _ => Err(serde_json::Error::custom("Unknown tag")),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PayResponse {
    /// a second-level url which give you an invoice with a GET request
    /// and an amount
    pub callback: String,
    /// max sendable amount for a given user on a given service
    #[serde(rename = "maxSendable")]
    pub max_sendable: u64,
    /// min sendable amount for a given user on a given service,
    /// can not be less than 1 or more than `maxSendable`
    #[serde(rename = "minSendable")]
    pub min_sendable: u64,
    /// tag of the request
    pub tag: Tag,
    /// Metadata json which must be presented as raw string here,
    /// this is required to pass signature verification at a later step
    pub metadata: String,
}

impl LnURLResponse for PayResponse {
    fn tag(&self) -> Tag {
        self.tag.clone()
    }

    fn callback(&self) -> String {
        self.callback.clone()
    }
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

#[derive(Debug, Serialize, Deserialize)]
pub struct WithdrawalResponse {
    /// A default withdrawal invoice description
    #[serde(rename = "defaultDescription")]
    pub default_description: String,
    /// a second-level url which would accept a withdrawal
    /// lightning invoice as query parameter
    pub callback: String,
    /// an ephemeral secret which would allow user to withdraw funds
    pub k1: String,
    /// max withdrawable amount for a given user on a given service
    #[serde(rename = "maxWithdrawable")]
    pub max_withdrawable: u64,
    /// An optional field, defaults to 1 MilliSatoshi if not present,
    /// can not be less than 1 or more than `maxWithdrawable`
    #[serde(rename = "minWithdrawable")]
    pub min_withdrawable: Option<u64>,
    /// tag of the request
    pub tag: Tag,
}

impl LnURLResponse for WithdrawalResponse {
    fn tag(&self) -> Tag {
        self.tag.clone()
    }

    fn callback(&self) -> String {
        self.callback.clone()
    }
}
