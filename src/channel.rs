use crate::Tag;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelResponse {
    /// Remote node address of form node_key@ip_address:port_number
    pub uri: String,
    /// a second-level URL which would initiate an OpenChannel message from target LN node
    pub callback: String,
    /// random or non-random string to identify the user's LN WALLET when using the callback URL
    pub k1: String,
    /// tag of the request
    pub tag: Tag,
}
