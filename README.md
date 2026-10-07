# Attestcoin SDK (Rust)

Rust libraries for building and encoding queries against the Attestcoin protocol.

| Crate | Description |
| --- | --- |
| [`attestcoin-abi-encoding`](attestcoin-abi-encoding) | Encodes Ethereum transactions and receipts into the chunked ABI format used for proof submission. |
| [`attestcoin-query-builder`](attestcoin-query-builder) | Builds Attestcoin proving queries from transactions, events and function calls. |

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
