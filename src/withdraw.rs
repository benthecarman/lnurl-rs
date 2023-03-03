use crate::Tag;
use serde::{Deserialize, Serialize};

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
