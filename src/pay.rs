use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use aes::Aes256;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bech32::primitives::decode::UncheckedHrpstring;
use bitcoin::hashes::sha256::Hash as Sha256;
use bitcoin::hashes::Hash;
use bitcoin::key::XOnlyPublicKey;
use cbc::{Decryptor, Encryptor};
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use url::Url;

type Aes256CbcEnc = Encryptor<Aes256>;
type Aes256CbcDec = Decryptor<Aes256>;

use crate::{Error, Tag};

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

    /// Verify that the BOLT11 invoice's amount equals the requested amount in
    /// millisatoshis, as required by LUD-06 before the invoice is paid.
    ///
    /// Returns [`Error::InvalidInvoice`] if `pr` cannot be parsed as a BOLT11
    /// invoice, and [`Error::InvoiceAmountMismatch`] if the invoice amount is
    /// absent or differs from `msats`.
    pub fn verify_amount(&self, msats: u64) -> Result<(), Error> {
        let invoice_msats = parse_bolt11_amount_msats(&self.pr)?;
        if invoice_msats != Some(msats) {
            return Err(Error::InvoiceAmountMismatch {
                requested_msats: msats,
                invoice_msats,
            });
        }

        Ok(())
    }
}

/// Parse the amount (in millisatoshis) encoded in a BOLT11 invoice without
/// pulling in a full invoice-parsing dependency.
///
/// The amount lives entirely in the human-readable part (HRP) of the invoice,
/// so we only parse and validate the bech32 structure (via the `bech32` crate)
/// and read the amount out of the HRP. The HRP is `ln` + a currency prefix
/// (letters only) + an optional amount, where the amount is one or more digits
/// followed by an optional multiplier letter (`m`/`u`/`n`/`p`).
///
/// Returns `Ok(None)` for an amountless invoice, `Ok(Some(msats))` when an
/// amount is present, and [`Error::InvalidInvoice`] if the HRP is malformed.
// `u128::is_multiple_of` is too new for this crate's MSRV.
#[allow(clippy::manual_is_multiple_of)]
fn parse_bolt11_amount_msats(invoice: &str) -> Result<Option<u64>, Error> {
    let invalid = |msg: &str| Error::InvalidInvoice(msg.to_string());

    // Parse the bech32 structure to split off and validate the human-readable
    // part. We deliberately do not verify the checksum: BOLT11 invoices are not
    // length-bounded, the amount lives entirely in the HRP, and the invoice
    // signature is the (possibly malicious) service's own, so a checksum check
    // would add no protection against a mismatched amount.
    let parsed =
        UncheckedHrpstring::new(invoice).map_err(|e| Error::InvalidInvoice(e.to_string()))?;
    // The HRP is all lower- or all upper-case (mixed case is rejected by the
    // parser above); normalize so the `ln` prefix and multiplier letters match.
    let hrp = parsed.hrp().as_str().to_ascii_lowercase();

    if !hrp.starts_with("ln") {
        return Err(invalid("not a lightning invoice"));
    }

    // The currency prefix contains no digits, so the amount section (if any)
    // starts at the first digit in the HRP.
    let amount = match hrp.find(|c: char| c.is_ascii_digit()) {
        None => return Ok(None), // amountless invoice
        Some(idx) => &hrp[idx..],
    };

    // Split off an optional trailing multiplier letter.
    let (digits, multiplier) = match amount.chars().last() {
        Some(c) if c.is_ascii_digit() => (amount, None),
        Some(c) => (&amount[..amount.len() - 1], Some(c)),
        None => unreachable!("amount is non-empty"),
    };

    let value: u128 = digits
        .parse()
        .map_err(|_| invalid("invalid amount digits"))?;

    // Convert to millisatoshis. 1 BTC = 100_000_000_000 msat, and the BOLT11
    // multipliers scale the value by 10^-3 (m), 10^-6 (u), 10^-9 (n), 10^-12 (p)
    // bitcoin respectively.
    let msats: u128 = match multiplier {
        None => value.checked_mul(100_000_000_000),
        Some('m') => value.checked_mul(100_000_000),
        Some('u') => value.checked_mul(100_000),
        Some('n') => value.checked_mul(100),
        Some('p') => {
            // A pico-bitcoin amount must be a multiple of 10, as 1 msat is the
            // smallest representable unit (10 pico-bitcoin).
            if value % 10 != 0 {
                return Err(invalid("sub-millisatoshi amount"));
            }
            Some(value / 10)
        }
        Some(_) => return Err(invalid("invalid amount multiplier")),
    }
    .ok_or_else(|| invalid("amount overflow"))?;

    let msats = u64::try_from(msats).map_err(|_| invalid("amount overflow"))?;

    Ok(Some(msats))
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

    // Real BOLT11 invoices from the BOLT #11 test vectors, with known amounts.

    // Amountless donation invoice.
    const AMOUNTLESS: &str = "lnbc1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdpl2pkx2ctnv5sxxmmwwd5kgetjypeh2ursdae8g6na6hlh";
    // 2500u = 250_000_000 msat (250_000 sat).
    const INV_250_000_000_MSAT: &str = "lnbc2500u1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdpquwpc4curk03c9wlrswe78q4eyqc7d8d0xqzpuyk0sg5g70me25alkluzd2x62aysf2pyy8edtjeevuv4p2d5p76r4zkmneet7uvyakky2zr4cusd45tftc9c5fh0nnqpnl2jfll544esqchsrnt";
    // 20m = 2_000_000_000 msat (0.02 BTC).
    const INV_2_000_000_000_MSAT: &str = "lnbc20m1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqhp58yjmdan79s6qqdhdzgynm4zwqd5d7xmw5fk98klysy043l2ahrqs9qrsgq7ea976txfraylvgzuxs8kgcw23ezlrszfnh8r6qtfpr6cxga50aj6txm9rxrydzd06dfeawfk6swupvz4erwnyutnjq7x39ymw6j38gp49qdkj";
    // 10m = 1_000_000_000 msat (0.01 BTC).
    const INV_1_000_000_000_MSAT: &str = "lnbc10m1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdp9wpshjmt9de6zqmt9w3skgct5vysxjmnnd9jx2mq8q8a04uqsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygs9q2gqqqqqqsgq7hf8he7ecf7n4ffphs6awl9t6676rrclv9ckg3d3ncn7fct63p6s365duk5wrk202cfy3aj5xnnp5gs3vrdvruverwwq7yzhkf5a3xqpd05wjc";
    // 9678785340p = 967_878_534 msat: a whole-millisatoshi but non-whole-sat amount.
    const INV_967_878_534_MSAT: &str = "lnbc9678785340p1pwmna7lpp5gc3xfm08u9qy06djf8dfflhugl6p7lgza6dsjxq454gxhj9t7a0sd8dgfkx7cmtwd68yetpd5s9xar0wfjn5gpc8qhrsdfq24f5ggrxdaezqsnvda3kkum5wfjkzmfqf3jkgem9wgsyuctwdus9xgrcyqcjcgpzgfskx6eqf9hzqnteypzxz7fzypfhg6trddjhygrcyqezcgpzfysywmm5ypxxjemgw3hxjmn8yptk7untd9hxwg3q2d6xjcmtv4ezq7pqxgsxzmnyyqcjqmt0wfjjq6t5v4khxsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygsxqyjw5qcqp2rzjq0gxwkzc8w6323m55m4jyxcjwmy7stt9hwkwe2qxmy8zpsgg7jcuwz87fcqqeuqqqyqqqqlgqqqqn3qq9q9qrsgqrvgkpnmps664wgkp43l22qsgdw4ve24aca4nymnxddlnp8vh9v2sdxlu5ywdxefsfvm0fq3sesf08uf6q9a2ke0hc9j6z6wlxg5z5kqpu2v9wz";
    // testnet 20m = 2_000_000_000 msat.
    const TESTNET_INV_2_000_000_000_MSAT: &str = "lntb20m1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygshp58yjmdan79s6qqdhdzgynm4zwqd5d7xmw5fk98klysy043l2ahrqspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqfpp3x9et2e20v6pu37c5d9vax37wxq72un989qrsgqdj545axuxtnfemtpwkc45hx9d2ft7x04mt8q7y6t0k2dge9e7h8kpy9p34ytyslj3yu569aalz2xdk8xkd7ltxqld94u8h2esmsmacgpghe9k8";
    // 2500000001p has sub-millisatoshi precision and must be rejected.
    const SUB_MSAT: &str = "lnbc2500000001p1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpusp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygs9qrsgq0lzc236j96a95uv0m3umg28gclm5lqxtqqwk32uuk4k6673k6n5kfvx3d2h8s295fad45fdhmusm8sjudfhlf6dcsxmfvkeywmjdkxcp99202x";

    #[test]
    fn test_parse_bolt11_amount() {
        let parse = |s: &str| parse_bolt11_amount_msats(s).unwrap();

        assert_eq!(parse(AMOUNTLESS), None);
        assert_eq!(parse(INV_250_000_000_MSAT), Some(250_000_000));
        assert_eq!(parse(INV_2_000_000_000_MSAT), Some(2_000_000_000));
        assert_eq!(parse(INV_1_000_000_000_MSAT), Some(1_000_000_000));
        assert_eq!(parse(INV_967_878_534_MSAT), Some(967_878_534));
        assert_eq!(parse(TESTNET_INV_2_000_000_000_MSAT), Some(2_000_000_000));
    }

    #[test]
    fn test_parse_bolt11_amount_invalid() {
        // sub-millisatoshi precision (pico amount not a multiple of 10)
        assert!(matches!(
            parse_bolt11_amount_msats(SUB_MSAT),
            Err(Error::InvalidInvoice(_))
        ));
        // not a lightning invoice
        assert!(matches!(
            parse_bolt11_amount_msats("not a real invoice"),
            Err(Error::InvalidInvoice(_))
        ));
        // amount that overflows u64 millisatoshis. No real invoice can carry
        // such a value, so this exercises the HRP parser directly.
        assert!(matches!(
            parse_bolt11_amount_msats("lnbc99999999999991q"),
            Err(Error::InvalidInvoice(_))
        ));
    }

    #[test]
    fn test_verify_amount_matches() {
        let inv = LnURLPayInvoice::new(INV_250_000_000_MSAT.to_string());
        assert!(inv.verify_amount(250_000_000).is_ok());
    }

    #[test]
    fn test_verify_amount_larger_invoice() {
        // invoice is for 2_000_000_000 msat, but only 250_000_000 was requested
        let inv = LnURLPayInvoice::new(INV_2_000_000_000_MSAT.to_string());
        match inv.verify_amount(250_000_000) {
            Err(Error::InvoiceAmountMismatch {
                requested_msats,
                invoice_msats,
            }) => {
                assert_eq!(requested_msats, 250_000_000);
                assert_eq!(invoice_msats, Some(2_000_000_000));
            }
            other => panic!("expected mismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_verify_amount_smaller_invoice() {
        // invoice is for 250_000_000 msat, but 2_000_000_000 was requested
        let inv = LnURLPayInvoice::new(INV_250_000_000_MSAT.to_string());
        match inv.verify_amount(2_000_000_000) {
            Err(Error::InvoiceAmountMismatch {
                requested_msats,
                invoice_msats,
            }) => {
                assert_eq!(requested_msats, 2_000_000_000);
                assert_eq!(invoice_msats, Some(250_000_000));
            }
            other => panic!("expected mismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_verify_amount_amountless_invoice() {
        let inv = LnURLPayInvoice::new(AMOUNTLESS.to_string());
        match inv.verify_amount(250_000_000) {
            Err(Error::InvoiceAmountMismatch {
                requested_msats,
                invoice_msats,
            }) => {
                assert_eq!(requested_msats, 250_000_000);
                assert_eq!(invoice_msats, None);
            }
            other => panic!("expected mismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_verify_amount_non_whole_sat() {
        // 967_878_534 msat is not a whole number of sats
        let inv = LnURLPayInvoice::new(INV_967_878_534_MSAT.to_string());
        assert!(inv.verify_amount(967_878_534).is_ok());
        // rounding the requested amount to whole sats must be rejected
        assert!(matches!(
            inv.verify_amount(967_878_000),
            Err(Error::InvoiceAmountMismatch { .. })
        ));
    }

    #[test]
    fn test_verify_amount_invalid_invoice() {
        let inv = LnURLPayInvoice::new("not a real invoice".to_string());
        assert!(matches!(
            inv.verify_amount(250_000_000),
            Err(Error::InvalidInvoice(_))
        ));
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
