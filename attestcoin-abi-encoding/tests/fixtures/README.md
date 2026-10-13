# OP-Stack deposit fixtures

Each `deposit_<chain>_<block>.json` holds the index-0 (L1-attributes) deposit transaction and its
receipt, exactly as returned by the RPC. The matching `_leaf.hex` is the expected V1 leaf, produced by
the reference implementation from gluwa/creditcoin3#1319 (`common/eth/src/op_stack.rs`).

- `base_sepolia_46388021`: a Canyon-or-later deposit. The leaf nonce is the receipt's `depositNonce`.
- `op_mainnet_110000000`: a pre-Canyon deposit. The RPC reports a nonzero tx nonce but neither header
  root commits to it, so the leaf nonce is zero.
