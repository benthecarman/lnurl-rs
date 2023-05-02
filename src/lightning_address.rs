use crate::lnurl::LnUrl;
use crate::Error;
use email_address::EmailAddress;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct LightningAddress {
    value: EmailAddress,
}

impl LightningAddress {
    pub fn new(value: &str) -> Result<Self, Error> {
        EmailAddress::from_str(value)
            .map(|value| LightningAddress { value })
            .map_err(|_| Error::InvalidLightningAddress)
    }

    #[inline]
    pub fn lnurlp_url(&self) -> String {
        format!(
            "https://{}/.well-known/lnurlp/{}",
            self.value.domain(),
            self.value.local_part()
        )
    }

    #[inline]
    pub fn lnurl(&self) -> LnUrl {
        LnUrl::from_url(self.lnurlp_url())
    }
}

impl FromStr for LightningAddress {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        LightningAddress::new(s)
    }
}

impl Serialize for LightningAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.value.as_str())
    }
}

impl<'de> Deserialize<'de> for LightningAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        LightningAddress::new(&s).map_err(serde::de::Error::custom)
    }
}

impl Display for LightningAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(test)]
mod test {
    use crate::lightning_address::LightningAddress;
    use crate::lnurl::LnUrl;
    use std::str::FromStr;

    #[test]
    fn test_parsing() {
        let address = LightningAddress::from_str("ben@opreturnbot.com");
        assert!(address.is_ok());
        assert_eq!(
            address.unwrap().lnurlp_url(),
            "https://opreturnbot.com/.well-known/lnurlp/ben"
        );
    }

    #[test]
    fn test_invalid_parsing() {
        assert!(LightningAddress::from_str("invalid").is_err());
        assert!(LightningAddress::from_str("####").is_err());
        assert!(LightningAddress::from_str("LNURL1DP68GURN8GHJ7UM9WFMXJCM99E3K7MF0V9CXJ0M385EKVCENXC6R2C35XVUKXEFCV5MKVV34X5EKZD3EV56NYD3HXQURZEPEXEJXXEPNXSCRVWFNV9NXZCN9XQ6XYEFHVGCXXCMYXYMNSERXFQ5FNS").is_err());
    }

    #[test]
    fn test_lnurl() {
        let address = LightningAddress::from_str("ben@opreturnbot.com").unwrap();
        let lnurl = LnUrl::from_str("lnurl1dp68gurn8ghj7mmswfjhgatjde3x7apwvdhk6tewwajkcmpdddhx7amw9akxuatjd3cz7cn9dc94s6d4").unwrap();

        assert_eq!(address.lnurl(), lnurl);
    }
}
