# Attestcoin ABI Encoding

A Rust library for encoding Ethereum transactions and receipts into ABI format, specifically designed for the Attestcoin protocol. This library provides efficient encoding that respects Solidity's stack limitations through a sophisticated chunking mechanism.

## Overview

The Attestcoin ABI encoding library transforms Ethereum transactions and their corresponding receipts into a standardized ABI-encoded format. The primary innovation of this library is its **chunked encoding approach**, which is specifically designed to work around Solidity's stack depth limitations.

## Why Chunked Encoding?

### The Solidity Stack Problem

Solidity has a strict stack depth limit of **1024 slots**. When dealing with complex Ethereum transactions (especially Type 3 and Type 4 transactions), encoding all transaction data as a single large tuple can easily exceed this limit, causing smart contract calls to fail with stack overflow errors.

Consider a Type 3 (EIP-4844) transaction with:
- Multiple access list entries
- Large blob versioned hashes arrays  
- Complex authorization lists
- Extensive log data in receipts

Encoding all this data in one tuple could require 1500+ stack slots, exceeding Solidity's limit.

### The Chunking Solution

Our chunking strategy divides transaction data into logical, smaller chunks that can be processed independently:

```rust
// Instead of one massive tuple:
// (nonce, gasLimit, from, to, value, input, accessList, blobHashes, ..., logs, receipts)

// We use multiple smaller chunks:
// Chunk 1: Common fields (nonce, gasLimit, from, to, value, input)
// Chunk 2: Transaction-specific fields (gasPrice, accessList, signatures)  
// Chunk 3: Receipt data (status, gasUsed, logs, blooms)
// Chunk 4: Additional data for complex transaction types
```

This approach ensures that:
- Each chunk stays well below the 1024 stack limit
- Smart contracts can process chunks sequentially
- Memory usage is optimized
- Encoding remains deterministic and verifiable

## Supported Transaction Types

The library supports all current Ethereum transaction types with optimized chunking:

| Type | Description | Chunks | Key Features |
|------|-------------|---------|--------------|
| **Type 0** | Legacy transactions | 3 | Basic transaction data, gas price, signatures |
| **Type 1** | EIP-2930 (Access Lists) | 3 | Adds access list encoding |
| **Type 2** | EIP-1559 (Dynamic Fees) | 3 | Max fee per gas, priority fees |
| **Type 3** | EIP-4844 (Blob transactions) | 4 | Blob versioned hashes, blob gas |
| **Type 4** | EIP-7702 (Account Abstraction) | 4 | Authorization lists, delegation |

## Chunk Structure

### Chunk 1: Common Transaction Fields
```rust
(
    nonce: uint64,
    gasLimit: uint64, 
    from: address,
    isToNull: bool,        // true for contract creation
    to: address,
    value: uint256,
    input: bytes           // transaction data
)
```

### Chunk 2: Transaction Type Specific Fields
Varies by transaction type, but includes signatures, gas pricing, and type-specific data.

### Chunk 3: Extended Data (Types 3 & 4)
Additional authorization lists, blob data, or other complex structures.

### Chunk 3/4: Receipt Data
```rust
(
    status: uint8,         // success/failure
    gasUsed: uint64,
    logs: Log[],          // event logs
    logsBloom: bytes      // bloom filter
)
```

## Usage

```rust
use attestcoin_abi_encoding::{abi_encode, EncodingVersion};
use alloy::rpc::types::{Transaction, TransactionReceipt};

// Encode a transaction and receipt
let result = abi_encode(transaction, receipt, EncodingVersion::V1)?;

// Get the encoded bytes
let encoded_bytes = result.abi();

// Verify the encoding version
assert_eq!(result.version(), EncodingVersion::V1);
```

## Encoding Format

The final encoded output follows this structure:

```rust
(
    transactionType: uint8,    // 0, 1, 2, 3, or 4
    chunks: bytes[]           // Array of ABI-encoded chunks
)
```

Each chunk in the array is independently ABI-encoded, allowing for:
- Efficient on-chain processing
- Reduced stack usage
- Parallel verification
- Modular decoding

## Technical Details

### Stack Optimization

The chunking strategy ensures that no single operation exceeds ~800 stack items, leaving a safety margin below Solidity's 1024 limit. This is achieved by:

1. **Logical grouping**: Related fields are grouped together
2. **Size monitoring**: Each chunk's complexity is tracked
3. **Dynamic splitting**: Large arrays are distributed across chunks
4. **Sequential processing**: Chunks can be processed one at a time

### Memory Efficiency

- Chunks are encoded lazily during the encoding process
- No unnecessary data duplication
- Optimal byte packing within each chunk
- Minimal metadata overhead

### Deterministic Encoding

The encoding is completely deterministic:
- Same transaction + receipt = same encoded output
- Field ordering is standardized
- No random elements or timestamps
- Suitable for cryptographic verification

## Integration with Smart Contracts

Smart contracts can efficiently process the chunked encoding:

```solidity
function processTransaction(bytes memory encodedData) external {
    (uint8 txType, bytes[] memory chunks) = abi.decode(encodedData, (uint8, bytes[]));
    
    // Process each chunk individually - no stack overflow!
    for (uint i = 0; i < chunks.length; i++) {
        processChunk(txType, i, chunks[i]);
    }
}
```

