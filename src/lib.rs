#![allow(clippy::large_enum_variant)]
#![allow(clippy::result_large_err)]

pub mod api;
mod auth;
pub mod channel;
pub mod lightning_address;
pub mod lnurl;
pub mod pay;
pub mod withdraw;

#[cfg(any(feature = "async", feature = "async-https"))]
pub mod r#async;
#[cfg(feature = "blocking")]
pub mod blocking;

pub use auth::get_derivation_path;

pub use api::*;
#[cfg(feature = "blocking")]
pub use blocking::BlockingClient;
#[cfg(any(feature = "async", feature = "async-https"))]
pub use r#async::AsyncClient;
use std::{fmt, io};

// All this copy-pasted from rust-esplora-client

#[derive(Debug, Clone, Default)]
pub struct Builder {
    /// Optional URL of the proxy to use to make requests to the LNURL server
    ///
    /// The string should be formatted as: `<protocol>://<user>:<password>@host:<port>`.
    ///
    /// Note that the format of this value and the supported protocols change slightly between the
    /// blocking version of the client (using `ureq`) and the async version (using `reqwest`). For more
    /// details check with the documentation of the two crates. Both of them are compiled with
    /// the `socks` feature enabled.
    ///
    /// The proxy is ignored when targeting `wasm32`.
    pub proxy: Option<String>,
    /// Socket timeout.
    pub timeout: Option<u64>,
}

impl Builder {
    /// Set the proxy of the builder
    pub fn proxy(mut self, proxy: &str) -> Self {
        self.proxy = Some(proxy.to_string());
        self
    }

    /// Set the timeout of the builder
    pub fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// build a blocking client from builder
    #[cfg(feature = "blocking")]
    pub fn build_blocking(self) -> Result<BlockingClient, Error> {
        BlockingClient::from_builder(self)
    }

    /// build an asynchronous client from builder
    #[cfg(feature = "async")]
    pub fn build_async(self) -> Result<AsyncClient, Error> {
        AsyncClient::from_builder(self)
    }
}

/// Errors that can happen during a sync with a LNURL service
#[derive(Debug)]
pub enum Error {
    /// Error decoding lnurl
    InvalidLnUrl,
    /// Error decoding lightning address
    InvalidLightningAddress,
    /// Invalid LnURL pay comment
    InvalidComment,
    /// Invalid amount on request
    InvalidAmount,
    /// Error during ureq HTTP request
    #[cfg(feature = "blocking")]
    Ureq(ureq::Error),
    /// Error during reqwest HTTP request
    #[cfg(any(feature = "async", feature = "async-https"))]
    Reqwest(reqwest::Error),
    /// HTTP response error
    HttpResponse(u16),
    /// IO error during ureq response read
    Io(io::Error),
    /// Error decoding JSON
    Json(serde_json::Error),
    /// Invalid Response
    InvalidResponse,
    /// Other error
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

macro_rules! impl_error {
    ( $from:ty, $to:ident ) => {
        impl_error!($from, $to, Error);
    };
    ( $from:ty, $to:ident, $impl_for:ty ) => {
        impl std::convert::From<$from> for $impl_for {
            fn from(err: $from) -> Self {
                <$impl_for>::$to(err)
            }
        }
    };
}

impl std::error::Error for Error {}
#[cfg(any(feature = "async", feature = "async-https"))]
impl_error!(::reqwest::Error, Reqwest, Error);
impl_error!(io::Error, Io, Error);
impl_error!(serde_json::Error, Json, Error);

#[cfg(all(feature = "blocking", any(feature = "async", feature = "async-https")))]
#[cfg(test)]
mod tests {
    use crate::lightning_address::LightningAddress;
    use crate::LnUrlResponse::LnUrlPayResponse;
    use crate::{AsyncClient, BlockingClient, Builder};
    use lightning_invoice::Bolt11Invoice;
    use nostr::prelude::ZapRequestData;
    use nostr::{EventBuilder, JsonUtil, Keys};
    use std::str::FromStr;

    #[cfg(all(feature = "blocking", any(feature = "async", feature = "async-https")))]
    async fn setup_clients() -> (BlockingClient, AsyncClient) {
        let blocking_client = Builder::default().build_blocking().unwrap();
        let async_client = Builder::default().build_async().unwrap();

        (blocking_client, async_client)
    }

    #[cfg(all(feature = "blocking", any(feature = "async", feature = "async-https")))]
    #[tokio::test]
    async fn test_get_invoice() {
        let url = "https://benthecarman.com/.well-known/lnurlp/ben";
        let (blocking_client, async_client) = setup_clients().await;

        let res = blocking_client.make_request(url).unwrap();
        let res_async = async_client.make_request(url).await.unwrap();

        // check res_async
        match res_async {
            LnUrlPayResponse(_) => {}
            _ => panic!("Wrong response type"),
        }

        if let LnUrlPayResponse(pay) = res {
            let msats = 1_000_000;
            let invoice = blocking_client
                .get_invoice(&pay, msats, None, None)
                .unwrap();
            let invoice_async = async_client
                .get_invoice(&pay, msats, None, None)
                .await
                .unwrap();

            let invoice = Bolt11Invoice::from_str(invoice.invoice()).unwrap();
            let invoice_async = Bolt11Invoice::from_str(invoice_async.invoice()).unwrap();

            assert_eq!(invoice.amount_milli_satoshis(), Some(msats));
            assert_eq!(invoice_async.amount_milli_satoshis(), Some(msats));
        } else {
            panic!("Wrong response type");
        }
    }

    #[cfg(all(feature = "blocking", any(feature = "async", feature = "async-https")))]
    #[tokio::test]
    async fn test_get_zap_invoice() {
        let url = "https://benthecarman.com/.well-known/lnurlp/ben";
        let (blocking_client, async_client) = setup_clients().await;

        let res = blocking_client.make_request(url).unwrap();
        let res_async = async_client.make_request(url).await.unwrap();

        // check res_async
        match res_async {
            LnUrlPayResponse(_) => {}
            _ => panic!("Wrong response type"),
        }

        if let LnUrlPayResponse(pay) = res {
            let msats = 1_000_000;

            let keys = Keys::generate();
            let event = {
                let data = ZapRequestData {
                    public_key: keys.public_key(),
                    relays: vec![],
                    amount: Some(msats),
                    lnurl: None,
                    event_id: None,
                    event_coordinate: None,
                };
                EventBuilder::new_zap_request(data).to_event(&keys).unwrap()
            };

            let invoice = blocking_client
                .get_invoice(&pay, msats, Some(event.as_json()), None)
                .unwrap();
            let invoice_async = async_client
                .get_invoice(&pay, msats, Some(event.as_json()), None)
                .await
                .unwrap();

            let invoice = Bolt11Invoice::from_str(invoice.invoice()).unwrap();
            let invoice_async = Bolt11Invoice::from_str(invoice_async.invoice()).unwrap();

            assert_eq!(invoice.amount_milli_satoshis(), Some(msats));
            assert_eq!(invoice_async.amount_milli_satoshis(), Some(msats));
        } else {
            panic!("Wrong response type");
        }
    }

    #[cfg(all(feature = "blocking", any(feature = "async", feature = "async-https")))]
    #[tokio::test]
    async fn test_get_invoice_with_comment() {
        let url = "https://getalby.com/.well-known/lnurlp/nvk";
        let (blocking_client, async_client) = setup_clients().await;

        let res = blocking_client.make_request(url).unwrap();
        let res_async = async_client.make_request(url).await.unwrap();

        // check res_async
        match res_async {
            LnUrlPayResponse(_) => {}
            _ => panic!("Wrong response type"),
        }

        if let LnUrlPayResponse(pay) = res {
            let msats = 1_000_000;

            let comment = "test comment".to_string();

            let invoice = blocking_client
                .get_invoice(&pay, msats, None, Some(&comment))
                .unwrap();
            let invoice_async = async_client
                .get_invoice(&pay, msats, None, Some(&comment))
                .await
                .unwrap();

            let invoice = Bolt11Invoice::from_str(invoice.invoice()).unwrap();
            let invoice_async = Bolt11Invoice::from_str(invoice_async.invoice()).unwrap();

            assert_eq!(invoice.amount_milli_satoshis(), Some(msats));
            assert_eq!(invoice_async.amount_milli_satoshis(), Some(msats));
        } else {
            panic!("Wrong response type");
        }
    }

    #[cfg(all(feature = "blocking", any(feature = "async", feature = "async-https")))]
    #[tokio::test]
    async fn test_get_invoice_ln_addr() {
        let ln_addr = LightningAddress::from_str("ben@opreturnbot.com").unwrap();
        let (blocking_client, async_client) = setup_clients().await;

        let res = blocking_client
            .make_request(ln_addr.lnurlp_url().as_str())
            .unwrap();
        let res_async = async_client
            .make_request(ln_addr.lnurlp_url().as_str())
            .await
            .unwrap();

        // check res_async
        match res_async {
            LnUrlPayResponse(_) => {}
            _ => panic!("Wrong response type"),
        }

        if let LnUrlPayResponse(pay) = res {
            let msats = 1_000_000;
            let invoice = blocking_client
                .get_invoice(&pay, msats, None, None)
                .unwrap();
            let invoice_async = async_client
                .get_invoice(&pay, msats, None, None)
                .await
                .unwrap();

            let invoice = Bolt11Invoice::from_str(invoice.invoice()).unwrap();
            let invoice_async = Bolt11Invoice::from_str(invoice_async.invoice()).unwrap();

            assert_eq!(invoice.amount_milli_satoshis(), Some(msats));
            assert_eq!(invoice_async.amount_milli_satoshis(), Some(msats));
        } else {
            panic!("Wrong response type");
        }
    }
}
