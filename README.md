# lnurl-rs

[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/benthecarman/lnurl-rs/blob/master/LICENSE)
[![lnurl-rs on crates.io](https://img.shields.io/crates/v/lnurl-rs.svg)](https://crates.io/crates/lnurl-rs)
[![lnurl-s on docs.rs](https://docs.rs/lnurl-rs/badge.svg)](https://docs.rs/lnurl-rs)

A rust implementation of [LNURL](https://github.com/lnurl/luds). Supports plaintext, TLS and Onion servers. Blocking or
async. WASM enabled.

## Supported

- lnurl-auth
- lnurl-pay
- lightning-address
- lnurl-withdraw
- lnurl-channel

## Examples

### Lnurl Pay

```rustc
let ln_addr = LightningAddress::from_str("ben@zaps.benthecarman.com").unwrap();
let async_client = Builder::default().build_async().unwrap();

let res = async_client.make_request(url).await.unwrap();

if let LnUrlPayResponse(pay) = res {
    let msats = 1_000_000;
    let pay_result = async_client.get_invoice(&pay, msats, None).await.unwrap();

    let invoice = Bolt11Invoice::from_str(&pay_result.invoice()).unwrap();

    assert_eq!(invoice.amount_milli_satoshis(), Some(msats));
} else {
    panic!("Wrong response type");
}
```
