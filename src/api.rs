use crate::channel::ChannelResponse;
use crate::pay::PayResponse;
use crate::withdraw::WithdrawalResponse;
use crate::Error as LnUrlError;
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
        Tag::ChannelRequest => {
            let resp: ChannelResponse = serde_json::from_value(json)?;
            Ok(LnUrlResponse::LnUrlChannelResponse(resp))
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum LnUrlResponse {
    LnUrlPayResponse(PayResponse),
    LnUrlWithdrawResponse(WithdrawalResponse),
    LnUrlChannelResponse(ChannelResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tag {
    #[serde(rename = "payRequest")]
    PayRequest,
    #[serde(rename = "withdrawRequest")]
    WithdrawRequest,
    #[serde(rename = "channelRequest")]
    ChannelRequest,
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tag::PayRequest => write!(f, "payRequest"),
            Tag::WithdrawRequest => write!(f, "withdrawRequest"),
            Tag::ChannelRequest => write!(f, "channelRequest"),
        }
    }
}

impl FromStr for Tag {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "payRequest" => Ok(Tag::PayRequest),
            "withdrawRequest" => Ok(Tag::WithdrawRequest),
            "channelRequest" => Ok(Tag::ChannelRequest),
            _ => Err(serde_json::Error::custom("Unknown tag")),
        }
    }
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
