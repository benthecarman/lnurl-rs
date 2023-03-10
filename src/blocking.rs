//! LNURL by way of `ureq` HTTP client.
#![allow(clippy::result_large_err)]

use bitcoin::PublicKey;
use std::time::Duration;

use ureq::{Agent, Proxy};

use crate::channel::ChannelResponse;
use crate::pay::{LnURLPayInvoice, PayResponse};
use crate::withdraw::WithdrawalResponse;
use crate::{decode_ln_url_response_from_json, Builder, Error, LnUrlResponse, Response};

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
            agent_builder = agent_builder.proxy(Proxy::new(proxy).unwrap());
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

    pub fn get_invoice(&self, pay: &PayResponse, msats: u64) -> Result<LnURLPayInvoice, Error> {
        let symbol = if pay.callback.contains('?') { "&" } else { "?" };

        let resp = self
            .agent
            .get(&format!("{}{}amount={}", pay.callback, symbol, msats))
            .call();

        match resp {
            Ok(resp) => Ok(resp.into_json()?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    pub fn do_withdrawal(
        &self,
        withdrawal: &WithdrawalResponse,
        invoice: &str,
    ) -> Result<Response, Error> {
        let symbol = if withdrawal.callback.contains('?') {
            "&"
        } else {
            "?"
        };

        let url = format!(
            "{}{}k1={}&pr={}",
            withdrawal.callback, symbol, withdrawal.k1, invoice
        );

        let resp = self.agent.get(&url).call();

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
    ) -> Result<Response, Error> {
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

        let resp = self.agent.get(&url).call();

        match resp {
            Ok(resp) => Ok(resp.into_json()?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }
}
