use crate::lightning_address::LightningAddress;
use crate::Error;
use bech32::primitives::decode::CheckedHrpstring;
use bech32::{Checksum, Fe1024, Fe32, Hrp};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// The bech32 checksum without its 1023-character length limit.
///
/// LUD-01 does not cap the length of an LNURL, and `encode` cannot fail, so a
/// long URL must still encode and decode rather than panic or be rejected.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum LnUrlBech32 {}

impl Checksum for LnUrlBech32 {
    type MidstateRepr = u32;
    type CorrectionField = Fe1024;
    const ROOT_GENERATOR: Self::CorrectionField = Fe1024::new([Fe32::P, Fe32::X]);
    const ROOT_EXPONENTS: core::ops::RangeInclusive<usize> = 24..=26;

    const CODE_LENGTH: usize = usize::MAX;
    const CHECKSUM_LENGTH: usize = 6;
    // Copied from BIP-173, the same as `bech32::Bech32`.
    const GENERATOR_SH: [u32; 5] = [
        0x3b6a_57b2,
        0x2650_8e6d,
        0x1ea1_19fa,
        0x3d42_33dd,
        0x2a14_62b3,
    ];
    const TARGET_RESIDUE: u32 = 1;
}

const LNURL_HRP: Hrp = Hrp::parse_unchecked("lnurl");

#[derive(Debug, PartialEq, Clone, Ord, PartialOrd, Eq, Hash)]
pub struct LnUrl {
    pub url: String,
}

impl LnUrl {
    #[inline]
    pub fn encode(&self) -> String {
        bech32::encode::<LnUrlBech32>(LNURL_HRP, self.url.as_bytes())
            .expect("LnUrlBech32 has no length limit")
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
            let checked =
                CheckedHrpstring::new::<LnUrlBech32>(s).map_err(|_| Error::InvalidLnUrl)?;
            let url = String::from_utf8(checked.byte_iter().collect())
                .map_err(|_| Error::InvalidLnUrl)?;
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
    fn long_url_round_trip() {
        // longer than the 1023-character limit of `bech32::Bech32`
        let url = format!("https://service.com/{}", "x".repeat(2000));
        let encoded = LnUrl::from_url(url.clone()).to_string();
        assert!(encoded.len() > 1023);
        assert_eq!(LnUrl::from_str(&encoded).unwrap().url, url);
        assert_eq!(LnUrl::from_str(&encoded.to_uppercase()).unwrap().url, url);
    }

    #[test]
    fn decode_rejects_bad_checksum() {
        let mut encoded = LnUrl::from_url("https://service.com/api".to_string()).encode();
        let last = encoded.pop().unwrap();
        encoded.push(if last == 'q' { 'p' } else { 'q' });
        assert!(LnUrl::from_str(&encoded).is_err());
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
