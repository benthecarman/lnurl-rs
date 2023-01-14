#![allow(clippy::result_large_err)]

pub mod api;

#[cfg(any(feature = "async", feature = "async-https"))]
pub mod r#async;
#[cfg(feature = "blocking")]
pub mod blocking;
pub mod lnurl;

pub use api::*;
#[cfg(feature = "blocking")]
pub use blocking::BlockingClient;
#[cfg(any(feature = "async", feature = "async-https"))]
pub use r#async::AsyncClient;
use std::{fmt, io};

// All this copy-pasted from rust-esplora-client

#[derive(Debug, Clone, Default)]
pub struct Builder {
    /// Optional URL of the proxy to use to make requests to the Esplora server
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
    /// Error during ureq HTTP request
    #[cfg(feature = "blocking")]
    Ureq(ureq::Error),
    /// Transport error during the ureq HTTP call
    #[cfg(feature = "blocking")]
    UreqTransport(ureq::Transport),
    /// Error during reqwest HTTP request
    #[cfg(any(feature = "async", feature = "async-https"))]
    Reqwest(reqwest::Error),
    /// HTTP response error
    HttpResponse(u16),
    /// IO error during ureq response read
    Io(io::Error),
    /// No header found in ureq response
    NoHeader,
    /// Invalid number returned
    Parsing(std::num::ParseIntError),
    /// Invalid Bitcoin data returned
    BitcoinEncoding(bitcoin::consensus::encode::Error),
    /// Invalid Hex data returned
    Hex(bitcoin::hashes::hex::Error),
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
#[cfg(feature = "blocking")]
impl_error!(::ureq::Transport, UreqTransport, Error);
#[cfg(any(feature = "async", feature = "async-https"))]
impl_error!(::reqwest::Error, Reqwest, Error);
impl_error!(io::Error, Io, Error);
impl_error!(std::num::ParseIntError, Parsing, Error);
impl_error!(bitcoin::hashes::hex::Error, Hex, Error);

#[cfg(test)]
mod tests {
    #[cfg(all(feature = "blocking", any(feature = "async", feature = "async-https")))]
    use super::*;

    #[cfg(all(feature = "blocking", any(feature = "async", feature = "async-https")))]
    async fn setup_clients() -> (BlockingClient, AsyncClient) {
        let blocking_client = Builder::default().build_blocking().unwrap();
        let async_client = Builder::default().build_async().unwrap();

        (blocking_client, async_client)
    }

    #[cfg(all(feature = "blocking", any(feature = "async", feature = "async-https")))]
    #[tokio::test]
    async fn test_lnurl_pay() {
        let url = "https://opreturnbot.com/.well-known/lnurlp/ben";
        let (blocking_client, async_client) = setup_clients().await;

        let res = blocking_client.make_request(url).unwrap();
        let res_async = async_client.make_request(url).await.unwrap();
        assert_eq!(res.tag(), Tag::PayRequest);
        assert_eq!(res_async.tag(), Tag::PayRequest);
    }

    #[cfg(all(feature = "blocking", any(feature = "async", feature = "async-https")))]
    #[tokio::test]
    async fn test_get_invoice() {
        let url = "https://opreturnbot.com/.well-known/lnurlp/ben";
        let (blocking_client, async_client) = setup_clients().await;

        let res = blocking_client.make_request(url).unwrap();
        let res_async = async_client.make_request(url).await.unwrap();
        assert_eq!(res.tag(), Tag::PayRequest);
        assert_eq!(res_async.tag(), Tag::PayRequest);

        let msats = 1_000_000;
        let invoice = blocking_client
            .get_invoice(res.callback().as_str(), msats)
            .unwrap();
        let invoice_async = async_client
            .get_invoice(res.callback().as_str(), msats)
            .await
            .unwrap();

        assert_eq!(invoice.invoice().amount_milli_satoshis(), Some(msats));
        assert_eq!(invoice_async.invoice().amount_milli_satoshis(), Some(msats));
    }
}
