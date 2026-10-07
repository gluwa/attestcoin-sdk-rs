# Attestcoin SDK (Rust)

[![CI](https://github.com/gluwa/attestcoin-sdk-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/gluwa/attestcoin-sdk-rs/actions/workflows/rust.yml)

Rust libraries for building and encoding queries against the Attestcoin protocol.

| Crate | crates.io | Description |
| --- | --- | --- |
| [`attestcoin-abi-encoding`](attestcoin-abi-encoding) | [![crates.io](https://img.shields.io/crates/v/attestcoin-abi-encoding.svg)](https://crates.io/crates/attestcoin-abi-encoding) | Encodes Ethereum transactions and receipts into the chunked ABI format used for proof submission. |
| [`attestcoin-query-builder`](attestcoin-query-builder) | [![crates.io](https://img.shields.io/crates/v/attestcoin-query-builder.svg)](https://crates.io/crates/attestcoin-query-builder) | Builds Attestcoin proving queries from transactions, events and function calls. |

Documentation for the protocol lives at <https://docs.attestcoin.org>.

## Development

```sh
cargo build --workspace
cargo test --workspace
```

The toolchain is pinned in [`rust-toolchain.toml`](rust-toolchain.toml). The `attestcoin-query-builder` tests fetch real transactions from a public RPC endpoint and need network access.

Releases are published to crates.io by pushing a tag named `<crate>-<version>`, for example `attestcoin-abi-encoding-0.7.0`.

## License

[Unlicense](https://unlicense.org)
