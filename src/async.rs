//! LNURL by way of `reqwest` HTTP client.
#![allow(clippy::result_large_err)]

use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::PublicKey;
use reqwest::Client;

use crate::api::*;
use crate::channel::ChannelResponse;
use crate::lnurl::LnUrl;
use crate::pay::{LnURLPayInvoice, PayResponse, VerifyResponse};
use crate::withdraw::WithdrawalResponse;
use crate::{Builder, Error};

#[derive(Debug, Clone)]
pub struct AsyncClient {
    pub client: Client,
}

impl Default for AsyncClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// build an async client from a builder
    pub fn from_builder(builder: Builder) -> Result<Self, Error> {
        let mut client_builder = Client::builder();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(proxy) = &builder.proxy {
            client_builder = client_builder.proxy(reqwest::Proxy::all(proxy)?);
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(timeout) = builder.timeout {
            client_builder = client_builder.timeout(core::time::Duration::from_secs(timeout));
        }

        Ok(Self::from_client(client_builder.build()?))
    }

    /// build an async client from the base url and [`Client`]
    pub fn from_client(client: Client) -> Self {
        AsyncClient { client }
    }

    pub async fn make_request(&self, url: &str) -> Result<LnUrlResponse, Error> {
        let resp = self.client.get(url).send().await?;

        let txt = resp.error_for_status()?.text().await?;
        decode_ln_url_response(&txt)
    }

    pub async fn get_invoice(
        &self,
        pay: &PayResponse,
        msats: u64,
        zap_request: Option<String>,
        comment: Option<&str>,
    ) -> Result<LnURLPayInvoice, Error> {
        // verify amount
        if msats < pay.min_sendable || msats > pay.max_sendable {
            return Err(Error::InvalidAmount);
        }

        // verify comment length
        if let Some(comment) = comment {
            if let Some(max_length) = pay.comment_allowed {
                if comment.len() > max_length as usize {
                    return Err(Error::InvalidComment);
                }
            }
        }

        let symbol = if pay.callback.contains('?') { "&" } else { "?" };

        let url = match (zap_request, comment) {
            (Some(_), Some(_)) => return Err(Error::InvalidComment),
            (Some(zap_request), None) => format!(
                "{}{}amount={}&nostr={}",
                pay.callback, symbol, msats, zap_request
            ),
            (None, Some(comment)) => format!(
                "{}{}amount={}&comment={}",
                pay.callback, symbol, msats, comment
            ),
            (None, None) => format!("{}{}amount={}", pay.callback, symbol, msats),
        };

        let resp = self.client.get(&url).send().await?;

        Ok(resp.error_for_status()?.json().await?)
    }

    pub async fn verify(&self, url: &str) -> Result<VerifyResponse, Error> {
        let resp = self.client.get(url).send().await?;

        let rsp: Response<VerifyResponse> = resp.error_for_status()?.json().await?;
        match rsp {
            Response::Error { reason } => Err(Error::Other(reason)),
            Response::Ok(r) => Ok(r),
        }
    }

    pub async fn do_withdrawal(
        &self,
        withdrawal: &WithdrawalResponse,
        invoice: &str,
    ) -> Result<Response<()>, Error> {
        let symbol = if withdrawal.callback.contains('?') {
            "&"
        } else {
            "?"
        };

        let url = format!(
            "{}{}k1={}&pr={}",
            withdrawal.callback, symbol, withdrawal.k1, invoice
        );
        let resp = self.client.get(url).send().await?;

        Ok(resp.error_for_status()?.json().await?)
    }

    pub async fn open_channel(
        &self,
        channel: &ChannelResponse,
        node_pubkey: PublicKey,
        private: bool,
    ) -> Result<Response<()>, Error> {
        let symbol = if channel.callback.contains('?') {
            "&"
        } else {
            "?"
        };

        let url = format!(
            "{}{}k1={}&remoteid={}&private={}",
            channel.callback,
            symbol,
            channel.k1,
            node_pubkey,
            private as i32 // 0 or 1
        );

        let resp = self.client.get(url).send().await?;

        Ok(resp.error_for_status()?.json().await?)
    }

    pub async fn lnurl_auth(
        &self,
        lnurl: LnUrl,
        sig: Signature,
        key: PublicKey,
    ) -> Result<Response<()>, Error> {
        let url = format!("{}&sig={}&key={}", lnurl.url, sig, key);

        let resp = self.client.get(url).send().await?;

        Ok(resp.error_for_status()?.json().await?)
    }
}
