use anyhow::anyhow;
use bitcoin::hashes::{sha256, Hash, HashEngine, Hmac, HmacEngine};
use bitcoin::util::bip32::{ChildNumber, DerivationPath};
use std::convert::TryInto;
use std::str::FromStr;
use url::Url;

/// Derive a derivation path from a hashing key and a url
/// This is for LUD-05
pub fn get_derivation_path(hashing_key: [u8; 32], url: Url) -> anyhow::Result<DerivationPath> {
    // There exists a private hashingKey which is derived by user LN WALLET using m/138'/0 path.
    let mut engine = HmacEngine::<sha256::Hash>::new(&hashing_key);

    // LN SERVICE full domain name is extracted from login LNURL
    let host = url.host().ok_or(anyhow!("No host"))?;

    // and then hashed using hmacSha256(hashingKey, full service domain name)
    engine.input(host.to_string().as_bytes());
    let derivation_mat = Hmac::<sha256::Hash>::from_engine(engine).into_inner();

    // First 16 bytes are taken from resulting hash and then turned into a sequence of 4 u32 values
    let uints: [u32; 4] = (0..4)
        .map(|i| u32::from_be_bytes(derivation_mat[(i * 4)..((i + 1) * 4)].try_into().unwrap()))
        .collect::<Vec<u32>>()
        .try_into()
        .expect("slice with incorrect length");
    // parse into ChildNumbers so we handle hardened vs unhardened
    let children = uints.map(ChildNumber::from);

    // which are in turn used to derive a service-specific linkingKey using m/138'/<long1>/<long2>/<long3>/<long4> path
    let path = DerivationPath::from_str(&format!(
        "m/138'/{}/{}/{}/{}",
        children[0], children[1], children[2], children[3]
    ))
    .map_err(|e| anyhow!("Error deriving path: {e}"))?;

    Ok(path)
}

#[cfg(test)]
mod test {
    use bitcoin::hashes::hex::FromHex;
    use bitcoin::util::bip32::{ChildNumber, DerivationPath};
    use std::str::FromStr;
    use url::Url;

    #[test]
    fn test_lud_05_static_test_vector() {
        let hashing_key: [u8; 32] =
            FromHex::from_hex("7d417a6a5e9a6a4a879aeaba11a11838764c8fa2b959c242d43dea682b3e409b")
                .unwrap();
        let url = Url::parse("https://site.com").unwrap();

        let path = super::get_derivation_path(hashing_key, url).unwrap();
        let expected = DerivationPath::from_str(&format!(
            "m/138'/{}/{}/{}/{}",
            ChildNumber::from(1588488367),
            ChildNumber::from(2659270754),
            ChildNumber::from(38110259),
            ChildNumber::from(4136336762),
        ))
        .unwrap();

        assert_eq!(path, expected);
    }
}
