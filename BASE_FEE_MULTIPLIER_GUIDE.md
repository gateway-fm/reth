# Base Fee Multiplier Guide

## How to Run the Tests

### Run all tests for the chainspec package:
```bash
cargo test -p reth-chainspec
```

### Run only the base fee multiplier tests:
```bash
cargo test -p reth-chainspec test_base_fee_change_multiplier
```

### Run a specific test:
```bash
cargo test -p reth-chainspec test_base_fee_change_multiplier_from_genesis
```

### Run tests with output:
```bash
cargo test -p reth-chainspec test_base_fee_change_multiplier -- --nocapture
```

## Where to Put the Multiplier Configuration

The multiplier configuration goes in your **genesis JSON file** directly in the `config` section.

### Example Genesis File with Multiplier

Here's an example of a complete genesis file with the multiplier configuration:

```json
{
  "config": {
    "chainId": 12345,
    "londonBlock": 0,
    "baseFeeChangeMultipliers": {
      "100": 0.01,
      "200": 1.0
    }
  },
  "timestamp": "0x0",
  "gasLimit": "0x1c9c380",
  "difficulty": "0x0",
  "extraData": "0x",
  "gasUsed": "0x0",
  "baseFeePerGas": "0x3b9aca00",
  "alloc": {},
  "number": "0x0",
  "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000"
}
```

### Key Points:

1. **Location**: The multiplier goes directly in `config.baseFeeChangeMultipliers`
2. **Format**: It's a JSON object where:
   - **Keys** are block numbers (as strings): `"100"`, `"200"`, etc.
   - **Values** are multipliers (as numbers): `0.01`, `1.0`, etc.
3. **Behavior**:
   - At block 100, the multiplier `0.01` will be applied (reduces base fee to 1%)
   - At block 200, the multiplier `1.0` will be applied (no change, back to normal)
   - Blocks before 100 will use the default multiplier of `1.0` (no change)

### How It Works:

- The multiplier for a given block is determined by finding the **highest configured block number that is ≤ the current block**
- The multiplier is applied to the **base fee delta** (change in base fee), not the final base fee value
- This means the multiplier affects how much the base fee changes, not the absolute value
- Example:
  - Block 50: Uses multiplier `1.0` (default, no multiplier configured yet)
  - Block 100: Uses multiplier `0.01` (from the "100" entry) - base fee changes will be 1% of normal
  - Block 150: Uses multiplier `0.01` (still using the "100" entry)
  - Block 200: Uses multiplier `1.0` (from the "200" entry) - base fee changes return to normal
  - Block 300: Uses multiplier `1.0` (still using the "200" entry)

### Using Your Genesis File with Reth

When starting Reth, you can use your custom genesis file:

```bash
# If your genesis file is at /path/to/genesis.json
reth init --chain /path/to/genesis.json

# Or if you're using a custom chain
reth node --chain /path/to/genesis.json
```

The multiplier will automatically be applied when calculating base fees for blocks.

