use crate::Error as LnUrlError;
use bitcoin::hashes::sha256::Hash as Sha256;
use bitcoin_hashes::Hash;
use lightning_invoice::Invoice;
use serde::de::Error;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::str::FromStr;

pub fn decode_ln_url_response(string: &str) -> Result<LnUrlResponse, LnUrlError> {
    let json: serde_json::Value = serde_json::from_str(string)?;
    decode_ln_url_response_from_json(json)
}

pub fn decode_ln_url_response_from_json(
    json: serde_json::Value,
) -> Result<LnUrlResponse, LnUrlError> {
    let obj = json.as_object().ok_or(LnUrlError::InvalidResponse)?;
    let tag_str = obj
        .get("tag")
        .and_then(|v| v.as_str())
        .ok_or(LnUrlError::InvalidResponse)?;

    let tag = Tag::from_str(tag_str)?;
    match tag {
        Tag::PayRequest => {
            let pay_response: PayResponse = serde_json::from_value(json)?;
            Ok(LnUrlResponse::LnUrlPayResponse(pay_response))
        }
        Tag::WithdrawRequest => {
            let resp: WithdrawalResponse = serde_json::from_value(json)?;
            Ok(LnUrlResponse::LnUrlWithdrawResponse(resp))
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum LnUrlResponse {
    LnUrlPayResponse(PayResponse),
    LnUrlWithdrawResponse(WithdrawalResponse),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Tag {
    #[serde(rename = "payRequest")]
    PayRequest,
    #[serde(rename = "withdrawRequest")]
    WithdrawRequest,
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tag::PayRequest => write!(f, "payRequest"),
            Tag::WithdrawRequest => write!(f, "withdrawRequest"),
        }
    }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// can not be less than 1 or more than `max_withdrawable`
    #[serde(rename = "minWithdrawable")]
    pub min_withdrawable: Option<u64>,
    /// tag of the request
    pub tag: Tag,
}

/// Response is the response format returned by Service.
/// Example: `{\"status\":\"ERROR\",\"reason\":\"error detail...\"}"`
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum Response {
    #[serde(rename = "ERROR")]
    Error { reason: String },
    #[serde(rename = "OK")]
    Ok { event: Option<String> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_from_str() {
        let tests = vec![
            (
                r#"{"status":"ERROR","reason":"error detail..."}"#,
                Response::Error {
                    reason: "error detail...".to_string(),
                },
            ),
            (
                r#"{"status":"OK","event":"LOGGEDIN"}"#,
                Response::Ok {
                    event: Some("LOGGEDIN".to_string()),
                },
            ),
        ];

        for test in tests {
            let resp: Response = serde_json::from_str(test.0).unwrap();
            assert_eq!(resp, test.1);
        }
    }
    #[test]
    fn response_to_str() {
        let tests = vec![
            (
                r#"{"status":"ERROR","reason":"error detail..."}"#,
                Response::Error {
                    reason: "error detail...".to_string(),
                },
            ),
            (
                r#"{"status":"OK","event":"LOGGEDIN"}"#,
                Response::Ok {
                    event: Some("LOGGEDIN".to_string()),
                },
            ),
        ];

        for test in tests {
            let json = serde_json::to_string(&test.1).unwrap();
            assert_eq!(json, test.0);
        }
    }
}
