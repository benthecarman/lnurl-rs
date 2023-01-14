use crate::Error;
use bech32::{ToBase32, Variant};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

pub struct LnUrl {
    pub url: String,
}

impl LnUrl {
    pub fn encode(&self) -> String {
        let base32 = self.url.as_bytes().to_base32();
        bech32::encode("lnurl", base32, Variant::Bech32).unwrap()
    }

    pub fn decode(lnurl: String) -> Result<LnUrl, Error> {
        LnUrl::from_str(lnurl.as_str())
    }

    pub fn from_url(url: String) -> Result<LnUrl, Error> {
        Ok(LnUrl { url })
    }
}

impl Display for LnUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.encode())
    }
}

impl FromStr for LnUrl {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        if s.to_lowercase().starts_with("lnurl") {
            let (_, data, _) = bech32::decode(s).unwrap();
            let bytes =
                bech32::FromBase32::from_base32(data.as_ref()).map_err(|_| Error::InvalidLnUrl)?;
            let url = String::from_utf8(bytes).map_err(|_| Error::InvalidLnUrl)?;
            Ok(LnUrl { url })
        } else {
            Err(Error::InvalidLnUrl)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn encode_test() {
        let url = "https://service.com/api?q=3fc3645b439ce8e7f2553a69e5267081d96dcd340693afabe04be7b0ccd178df";
        let expected =
            "LNURL1DP68GURN8GHJ7UM9WFMXJCM99E3K7MF0V9CXJ0M385EKVCENXC6R2C35XVUKXEFCV5MKVV34X5EKZD3EV56NYD3HXQURZEPEXEJXXEPNXSCRVWFNV9NXZCN9XQ6XYEFHVGCXXCMYXYMNSERXFQ5FNS";

        let lnurl = super::LnUrl::from_url(url.to_string()).unwrap();
        assert_eq!(lnurl.to_string().to_uppercase(), expected);
    }

    #[test]
    fn decode_tests() {
        let str =
            "LNURL1DP68GURN8GHJ7UM9WFMXJCM99E3K7MF0V9CXJ0M385EKVCENXC6R2C35XVUKXEFCV5MKVV34X5EKZD3EV56NYD3HXQURZEPEXEJXXEPNXSCRVWFNV9NXZCN9XQ6XYEFHVGCXXCMYXYMNSERXFQ5FNS";
        let expected = "https://service.com/api?q=3fc3645b439ce8e7f2553a69e5267081d96dcd340693afabe04be7b0ccd178df";

        let lnurl = super::LnUrl::decode(str.to_string()).unwrap();
        assert_eq!(lnurl.url, expected);
    }
}
