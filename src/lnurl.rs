use crate::lightning_address::LightningAddress;
use crate::Error;
use bech32::{ToBase32, Variant};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, PartialEq, Clone, Ord, PartialOrd, Eq, Hash)]
pub struct LnUrl {
    pub url: String,
}

impl LnUrl {
    #[inline]
    pub fn encode(&self) -> String {
        let base32 = self.url.as_bytes().to_base32();
        bech32::encode("lnurl", base32, Variant::Bech32).unwrap()
    }

    pub fn is_lnurl_auth(&self) -> bool {
        self.url.contains("tag=login") && self.url.contains("k1=")
    }

    pub fn lightning_address(&self) -> Option<LightningAddress> {
        let url = url::Url::from_str(&self.url).ok()?;
        let local_part = url.path().strip_prefix("/.well-known/lnurlp/")?;
        LightningAddress::from_domain_and_local_part(url.host_str()?, local_part).ok()
    }

    #[inline]
    pub fn decode(lnurl: String) -> Result<LnUrl, Error> {
        LnUrl::from_str(&lnurl)
    }

    #[inline]
    pub fn from_url(url: String) -> LnUrl {
        LnUrl { url }
    }
}

impl Display for LnUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.encode())
    }
}

impl Serialize for LnUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.encode())
    }
}

impl<'de> Deserialize<'de> for LnUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        LnUrl::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for LnUrl {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        if s.to_lowercase().starts_with("lnurl") {
            let (_, data, _) = bech32::decode(s).map_err(|_| Error::InvalidLnUrl)?;
            let bytes = bech32::FromBase32::from_base32(&data).map_err(|_| Error::InvalidLnUrl)?;
            let url = String::from_utf8(bytes).map_err(|_| Error::InvalidLnUrl)?;
            Ok(LnUrl { url })
        } else {
            Err(Error::InvalidLnUrl)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_test() {
        let url = "https://service.com/api?q=3fc3645b439ce8e7f2553a69e5267081d96dcd340693afabe04be7b0ccd178df";
        let expected =
            "LNURL1DP68GURN8GHJ7UM9WFMXJCM99E3K7MF0V9CXJ0M385EKVCENXC6R2C35XVUKXEFCV5MKVV34X5EKZD3EV56NYD3HXQURZEPEXEJXXEPNXSCRVWFNV9NXZCN9XQ6XYEFHVGCXXCMYXYMNSERXFQ5FNS";

        let lnurl = LnUrl::from_url(url.to_string());
        assert_eq!(lnurl.to_string().to_uppercase(), expected);
    }

    #[test]
    fn decode_tests() {
        let str =
            "LNURL1DP68GURN8GHJ7UM9WFMXJCM99E3K7MF0V9CXJ0M385EKVCENXC6R2C35XVUKXEFCV5MKVV34X5EKZD3EV56NYD3HXQURZEPEXEJXXEPNXSCRVWFNV9NXZCN9XQ6XYEFHVGCXXCMYXYMNSERXFQ5FNS";
        let expected = "https://service.com/api?q=3fc3645b439ce8e7f2553a69e5267081d96dcd340693afabe04be7b0ccd178df";

        let lnurl = LnUrl::decode(str.to_string()).unwrap();
        assert_eq!(lnurl.url, expected);
    }

    #[test]
    fn lnurl_auth_test() {
        let str = "https://service.com/api?q=3fc3645b439ce8e7f2553a69e5267081d96dcd340693afabe04be7b0ccd178df&tag=login&k1=3fc3645b439ce8e7f2553a69e5267081d96dcd340693afabe04be7b0ccd178df";
        let lnurl = LnUrl::from_url(str.to_string());
        assert!(lnurl.is_lnurl_auth());

        let str = "https://service.com/api?q=3fc3645b439ce8e7f2553a69e5267081d96dcd340693afabe04be7b0ccd178df&tag=login";
        let lnurl = LnUrl::from_url(str.to_string());
        assert!(!lnurl.is_lnurl_auth());

        let str = "https://service.com/api?q=3fc3645b439ce8e7f2553a69e5267081d96dcd340693afabe04be7b0ccd178df&k1=3fc3645b439ce8e7f2553a69e5267081d96dcd340693afabe04be7b0ccd178df";
        let lnurl = LnUrl::from_url(str.to_string());
        assert!(!lnurl.is_lnurl_auth());
    }

    #[test]
    fn lnurl_to_lightning_address() {
        let lightning_address = LightningAddress::from_str("me@benthecarman.com").unwrap();
        let lnurl = lightning_address.lnurl();

        assert_eq!(lnurl.lightning_address(), Some(lightning_address));
    }
}
