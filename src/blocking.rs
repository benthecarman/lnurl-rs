//! LNURL by way of `ureq` HTTP client.
#![allow(clippy::result_large_err)]

use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::PublicKey;
use std::time::Duration;

use ureq::{Agent, Proxy};

use crate::channel::ChannelResponse;
use crate::lnurl::LnUrl;
use crate::pay::{decode_invoice_response, LnURLPayInvoice, PayResponse};
use crate::withdraw::WithdrawalResponse;
use crate::{
    callback_url, decode_ln_url_response_from_json, Builder, Error, LnUrlResponse, Response,
};

#[derive(Debug, Clone)]
pub struct BlockingClient {
    agent: Agent,
}

impl BlockingClient {
    /// build a blocking client from a [`Builder`]
    pub fn from_builder(builder: Builder) -> Result<Self, Error> {
        let mut agent_builder = ureq::AgentBuilder::new();

        if let Some(timeout) = builder.timeout {
            agent_builder = agent_builder.timeout(Duration::from_secs(timeout));
        }

        if let Some(proxy) = &builder.proxy {
            agent_builder = agent_builder.proxy(Proxy::new(proxy).map_err(Error::Ureq)?);
        }

        Ok(Self::from_agent(agent_builder.build()))
    }

    /// build a blocking client from an [`Agent`]
    pub fn from_agent(agent: Agent) -> Self {
        BlockingClient { agent }
    }

    pub fn make_request(&self, url: &str) -> Result<LnUrlResponse, Error> {
        let resp = self.agent.get(url).call();

        match resp {
            Ok(resp) => {
                let json: serde_json::Value = resp.into_json()?;
                decode_ln_url_response_from_json(json)
            }
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    pub fn get_invoice(
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

        let resp = self.agent.get(url.as_str()).call();

        match resp {
            Ok(resp) => {
                let json: serde_json::Value = resp.into_json()?;
                decode_invoice_response(json, msats)
            }
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    pub fn do_withdrawal(
        &self,
        withdrawal: &WithdrawalResponse,
        invoice: &str,
    ) -> Result<Response<()>, Error> {
        let url = callback_url(
            &withdrawal.callback,
            &[("k1", withdrawal.k1.as_str()), ("pr", invoice)],
        )
        .map_err(|_| Error::InvalidResponse)?;

        let resp = self.agent.get(url.as_str()).call();

        match resp {
            Ok(resp) => Ok(resp.into_json()?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    pub fn open_channel(
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

        let resp = self.agent.get(url.as_str()).call();

        match resp {
            Ok(resp) => Ok(resp.into_json()?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    pub fn lnurl_auth(
        &self,
        lnurl: LnUrl,
        sig: Signature,
        key: PublicKey,
    ) -> Result<Response<()>, Error> {
        let sig = sig.to_string();
        let key = key.to_string();
        let url = callback_url(&lnurl.url, &[("sig", sig.as_str()), ("key", key.as_str())])
            .map_err(|_| Error::InvalidLnUrl)?;

        let resp = self.agent.get(url.as_str()).call();

        match resp {
            Ok(resp) => Ok(resp.into_json()?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }
}
