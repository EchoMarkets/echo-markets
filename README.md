# Echo Markets

A decentralized prediction market running directly on Bitcoin via the [Charms](https://charms.dev) protocol. Create/resolve/cancel markets, mint/trade/burn YES/NO outcome tokens, claim fees, and redeem winning positions - all on Bitcoin's base layer without requiring sidechains or trusted intermediaries.

## Overview

Echo Markets enables users to create prediction markets where participants can bet on binary outcomes (Yes/No) by trading fungible tokens. The protocol uses a dual-token system:

- **YES tokens**: Represent a bet that the outcome will be "Yes"
- **NO tokens**: Represent a bet that the outcome will be "No"

Users mint complete sets (1 YES + 1 NO) by depositing collateral. After market resolution, holders of winning tokens can redeem them for collateral, while losing tokens become worthless.

## Features

- ✅ **Native Bitcoin Integration**: Runs directly on Bitcoin using Charms protocol
- ✅ **Decentralized**: No trusted intermediaries
- ✅ **Dual-Token System**: YES/NO tokens for binary outcomes
- ✅ **Fee Collection**: Configurable trading fees (basis points)
- ✅ **Time-Limited Markets**: Trading and resolution deadlines
- ✅ **Market Cancellation**: Creator can cancel before resolution
- ✅ **P2P Trading**: Direct token swaps between users
- ✅ **Supply Limits**: Min/max supply controls to prevent dust and bound market size
- ✅ **Comprehensive Testing**: Unit tests and integration tests
- ✅ **WASM Compatible**: Pure Rust implementation compatible with zkVM

## Architecture

### Market Lifecycle

```
State Machine:

    [Create]
       |
       v
   [Active] <---> [TradingClosed] (after trading_deadline)
       |                |
       |                |
       +---> [Resolved] <--+
       |                   |
       |                   |
       v                   v
   [Cancelled]        [Redeem]
```

### State Transitions

1. **Active**: Market is open for trading

   - Users can mint/burn tokens
   - Users can transfer tokens
   - Market cannot be resolved yet

2. **TradingClosed**: Trading deadline has passed

   - No new minting/burning allowed
   - Token transfers still allowed
   - Market can be resolved

3. **Resolved**: Market outcome has been determined

   - No trading or transfers allowed
   - Only redemption of winning tokens
   - Fees can be claimed by creator

4. **Cancelled**: Market was cancelled by creator
   - Only possible before resolution
   - All tokens can be redeemed for refund

### Token Economics

- **Minting**: User deposits `collateral`, pays `fee = collateral * fee_bps / 10000`, receives `shares = collateral - fee` in YES + NO tokens
- **Burning**: User burns equal YES + NO tokens, receives collateral back (minus fees already paid)
- **Trading**: Users can transfer YES/NO tokens to each other (P2P trading)
- **Redemption**: After resolution, winning token holders redeem 1:1 for collateral

### Market Parameters

- `trading_deadline`: Unix timestamp when trading stops
- `resolution_deadline`: Unix timestamp when resolution becomes available
- `fee_bps`: Trading fee in basis points (100 = 1%)
- `min_bet`: Minimum collateral required to mint tokens
- `max_supply`: Maximum total supply of YES/NO tokens

## Installation

### Prerequisites

- Rust (latest stable version)
- [Charms CLI](https://docs.charms.dev/guides/charms-apps/get-started/)
- WASM target for Rust

### Setup

1. **Install Rust and WASM target:**

```bash
rustup target add wasm32-wasip1
```

2. **Clone the repository:**

```bash
git clone https://github.com/EchoMarkets/echo-markets.git
cd echo-markets
```

3. **Build the project:**

```bash
cargo build --release --target wasm32-wasip1
```

The resulting WASM binary will be at `./target/wasm32-wasip1/release/echo-markets.wasm`.

Alternatively, use the Charms CLI:

```bash
app_bin=$(charms app build)
```

4. **Get the verification key:**

```bash
export app_vk=$(charms app vk)
```

## Usage

### Creating a Market

```bash
# Set environment variables
export in_utxo_0="<your-funding-utxo>"
export market_id=$(echo -n "${in_utxo_0}" | sha256sum | cut -d' ' -f1)
export question_hash=$(echo -n "Will BTC reach 100k by Dec 2025?" | sha256sum | cut -d' ' -f1)
export trading_deadline=1735603200
export resolution_deadline=1735689600
export fee_bps=100
export min_bet=10000
export max_supply=1000000000000
export creator_pubkey="02..."  # 33-byte compressed pubkey hex
export addr_0="tb1p..."  # Taproot address

# Validate the spell
cat ./spells/create-market.yaml | envsubst | charms spell check --app-bins=${app_bin}
```

### Minting Tokens

```bash
# Set up token IDs
export yes_token_id=$(echo -n "${market_id}YES" | sha256sum | cut -d' ' -f1)
export no_token_id=$(echo -n "${market_id}NO" | sha256sum | cut -d' ' -f1)

# Set market state variables
export market_utxo_id="<market-nft-utxo>"
export user_btc_utxo="<user-collateral-utxo>"
export old_yes_supply=0
export old_no_supply=0
export new_yes_supply=990000
export new_no_supply=990000
export mint_amount=1000000
export current_timestamp=$(date +%s)
export addr_market="tb1p..."
export addr_user="tb1p..."

# Validate the spell
cat ./spells/mint-shares.yaml | envsubst | charms spell check --app-bins=${app_bin}
```

### Burning Shares

```bash
# Set market state variables
export market_utxo_id="<market-nft-utxo>"
export user_yes_utxo="<user-yes-tokens-utxo>"
export user_no_utxo="<user-no-tokens-utxo>"
export old_yes_supply=990000
export old_no_supply=990000
export burn_amount=100000  # Amount of sets to burn (must be equal YES + NO)
export new_yes_supply=$((old_yes_supply - burn_amount))
export new_no_supply=$((old_no_supply - burn_amount))
export current_timestamp=$(date +%s)
export addr_market="tb1p..."
export addr_user="tb1p..."

# Validate the spell
cat ./spells/burn-shares.yaml | envsubst | charms spell check --app-bins=${app_bin}
```

### Trading Tokens (P2P Swap)

```bash
export alice_yes_utxo="<alice-yes-tokens-utxo>"
export bob_no_utxo="<bob-no-tokens-utxo>"
export trade_amount=50000
export addr_alice="tb1p..."
export addr_bob="tb1p..."

cat ./spells/trade.yaml | envsubst | charms spell check --app-bins=${app_bin}
```

### Resolving a Market

```bash
export market_utxo_id="<market-nft-utxo>"
export outcome="Yes"  # or "No" or "Invalid"
export resolver_pubkey="<resolver-public-key-hex>"
export resolution_signature="<signature-hex>"
export resolution_timestamp=$(date +%s)
export yes_supply=990000
export no_supply=990000
export accumulated_fees=10000
export addr_market="tb1p..."

cat ./spells/resolve-market.yaml | envsubst | charms spell check --app-bins=${app_bin}
```

### Redeeming Winning Tokens

```bash
export market_utxo_id="<market-nft-utxo>"
export user_tokens_utxo="<user-tokens-utxo>"
export yes_tokens_burned=990000
export no_tokens_burned=0  # or equal amount if Invalid
export outcome="Yes"  # must match market resolution
export addr_market="tb1p..."
export addr_user="tb1p..."

cat ./spells/redeem.yaml | envsubst | charms spell check --app-bins=${app_bin}
```

### Cancelling a Market

```bash
export market_utxo_id="<market-nft-utxo>"
export creator_signature="<creator-signature-hex>"
export cancel_timestamp=$(date +%s)
export yes_supply=990000
export no_supply=990000
export addr_market="tb1p..."

cat ./spells/cancel-market.yaml | envsubst | charms spell check --app-bins=${app_bin}
```

### Claiming Fees

```bash
export market_utxo_id="<market-nft-utxo>"
export accumulated_fees=10000
export yes_supply=990000
export no_supply=990000
export addr_market="tb1p..."
export addr_creator="tb1p..."

cat ./spells/claim-fees.yaml | envsubst | charms spell check --app-bins=${app_bin}
```

## Testing

### Running Unit Tests

```bash
cargo test
```

### Running Integration Tests

```bash
cargo test --test integration_tests
```

### Testing Spells

The project includes comprehensive spell testing scripts:

```bash
# Dry run (validates YAML structure)
./test-spells.sh

# Real test (requires testnet UTXOs)
./test-spells.sh --real

# Simple validation (no CLI required)
./validate-spells.sh
```

## Documentation

### Generating API Documentation

Generate HTML documentation from the Rust code:

```bash
# Generate documentation
cargo doc

# Generate and open in browser
cargo doc --open

# Generate docs without dependencies (faster)
cargo doc --no-deps
```

The documentation will be available at `target/doc/echo_markets/index.html` and includes all doc comments from the source code.

## Project Structure

```
echo-markets/
├── src/
│   ├── lib.rs              # Core contract implementation
│   ├── tests.rs            # Unit tests
│   ├── integration_tests.rs # Integration tests
│   └── main.rs             # Entry point (for local testing)
├── spells/                 # Charms spell definitions
│   ├── create-market.yaml
│   ├── mint-shares.yaml
│   ├── burn-shares.yaml
│   ├── trade.yaml
│   ├── resolve-market.yaml
│   ├── redeem.yaml
│   ├── cancel-market.yaml
│   └── claim-fees.yaml
├── test-spells.sh          # Spell testing script
├── validate-spells.sh      # Simple spell validation
├── Cargo.toml              # Rust dependencies
└── README.md               # This file
```

## Security Considerations

### Time Validation

1. **Primary Enforcement**: Scrolls enforces trading deadlines at the transaction layer
2. **Contract Validation**: Timestamp passed in operation data is validated by the contract
3. **Defense-in-Depth**: Both layers provide security

### Signature Verification

- **Resolution Signatures**: Uses Schnorr signatures (k256) for WASM compatibility
- **Creator Signatures**: Market creator can cancel markets with valid signature
- **Oracle Proofs**: Cross-chain oracle verification (simplified trusted oracle for Hackathon MVP)

### Supply Limits

- **Min Bet**: Prevents dust attacks and ensures meaningful trades
- **Max Supply**: Bounds market size and prevents overflow
- **Checked Arithmetic**: All supply operations use checked arithmetic to prevent overflow

## Technical Details

### Dependencies

- **charms-sdk**: Charms protocol SDK for Bitcoin apps
- **k256**: Pure Rust secp256k1 implementation (WASM compatible)
- **sha2**: SHA256 hashing
- **bincode**: Binary serialization
- **serde**: Serialization framework

### Build Configuration

The project is configured for optimal WASM compilation:

- `opt-level = 3`: Maximum optimization
- `lto = "fat"`: Full link-time optimization
- `panic = "abort"`: Smaller binary size
- `overflow-checks = true`: Safety checks enabled

### WASM Compatibility

All dependencies are pure Rust implementations compatible with `wasm32-wasip1` target. The `k256` crate is used instead of `secp256k1` to avoid C dependencies that don't compile for WASM.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.

## Resources

- [Charms Documentation](https://docs.charms.dev)
- [Charms Getting Started Guide](https://docs.charms.dev/guides/charms-apps/get-started/)
- [Charms Introduction](https://docs.charms.dev/guides/charms-apps/introduction/)
- [Bitcoin Taproot](https://en.bitcoin.it/wiki/Taproot)

## Acknowledgments

- Built with [Charms](https://charms.dev) protocol for the [The BOS Hackathon](https://www.encodeclub.com/programmes/enchanting-utxo-bitcoin-smart-contracts-by-bitcoinos) - Building Bitcoin Smart Contracts with the BitcoinOS Stack
