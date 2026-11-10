# OP-Stack deposit fixtures

Each `deposit_<chain>_<block>.json` holds the index-0 (L1-attributes) deposit transaction and its
receipt, exactly as returned by the RPC. The matching `_leaf.hex` is the expected V1 leaf, produced by
the reference implementation from gluwa/creditcoin3#1319 (`common/eth/src/op_stack.rs`).

- `base_sepolia_46388021`: a Canyon-or-later deposit. The leaf nonce is the receipt's `depositNonce`.
- `op_mainnet_110000000`: a pre-Canyon deposit. The RPC reports a nonzero tx nonce but neither header
  root commits to it, so the leaf nonce is zero.

## asc-contracts fixtures

`asc_contracts_op_stack_deposits.json` is a byte-identical copy of `test/fixtures/op-stack-deposits.json`
from gluwa/asc-contracts at `7a2bb0d444f460422c2ed4af037bf889961c4670` (PR #47). Its Solidity tests
decode the `txBytes` leaves with `EvmV1Decoder2`. `tests/asc_contracts_fixtures.rs` rebuilds each leaf
from the fixture's expected fields with this crate and requires the same bytes, so the Rust encoder and
the Solidity decoder are pinned to each other. Refresh by copying the file from a newer asc-contracts
revision and updating the commit above.
