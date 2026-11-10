//! LNURL by way of `reqwest` HTTP client.
#![allow(clippy::result_large_err)]

use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::PublicKey;
use reqwest::Client;

use crate::api::*;
use crate::channel::ChannelResponse;
use crate::lnurl::LnUrl;
use crate::pay::{decode_invoice_response, LnURLPayInvoice, PayResponse, VerifyResponse};
use crate::withdraw::WithdrawalResponse;
use crate::{callback_url, Builder, Error};

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

        let amount = msats.to_string();
        let mut params = vec![("amount", amount.as_str())];
        match (&zap_request, comment) {
            (Some(_), Some(_)) => return Err(Error::InvalidComment),
            (Some(zap_request), None) => params.push(("nostr", zap_request.as_str())),
            (None, Some(comment)) => params.push(("comment", comment)),
            (None, None) => {}
        }
        let url = callback_url(&pay.callback, &params).map_err(|_| Error::InvalidResponse)?;

        let resp = self.client.get(url).send().await?;

        let json: serde_json::Value = resp.error_for_status()?.json().await?;
        decode_invoice_response(json, &pay.callback, msats)
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
        let url = callback_url(
            &withdrawal.callback,
            &[("k1", withdrawal.k1.as_str()), ("pr", invoice)],
        )
        .map_err(|_| Error::InvalidResponse)?;
        let resp = self.client.get(url).send().await?;

        Ok(resp.error_for_status()?.json().await?)
    }

    pub async fn open_channel(
        &self,
        channel: &ChannelResponse,
        node_pubkey: PublicKey,
        private: bool,
    ) -> Result<Response<()>, Error> {
        let remote_id = node_pubkey.to_string();
        let private = if private { "1" } else { "0" };
        let url = callback_url(
            &channel.callback,
            &[
                ("k1", channel.k1.as_str()),
                ("remoteid", remote_id.as_str()),
                ("private", private),
            ],
        )
        .map_err(|_| Error::InvalidResponse)?;

        let resp = self.client.get(url).send().await?;

        Ok(resp.error_for_status()?.json().await?)
    }

    pub async fn lnurl_auth(
        &self,
        lnurl: LnUrl,
        sig: Signature,
        key: PublicKey,
    ) -> Result<Response<()>, Error> {
        let sig = sig.to_string();
        let key = key.to_string();
        let url = callback_url(&lnurl.url, &[("sig", sig.as_str()), ("key", key.as_str())])
            .map_err(|_| Error::InvalidLnUrl)?;

        let resp = self.client.get(url).send().await?;

        Ok(resp.error_for_status()?.json().await?)
    }
}
