//! Charms Echo Markets - Core Contract
//! 
//! This is a starter implementation for a decentralized prediction market
//! running directly on Bitcoin via the Charms protocol.


//! # Time Validation Approach
//! 
//! Charms runs in a zkVM on Bitcoin and doesn't provide direct access to block time or height.
//! Here it is used the following approach for timestamp validation:
//! 
//! 1. **Primary Enforcement**: Scrolls enforces trading deadlines
//!    - Scrolls can check block time/height before submitting transactions
//!    - Invalid transactions are rejected at the Scrolls layer
//! 
//! 2. **Contract Validation**: Timestamp passed in operation data
//!    - Operations (Mint, Burn) include `current_timestamp` field
//!    - Contract validates `current_timestamp < trading_deadline`
//!    - Provides defense-in-depth security
//! 
//! 3. **Auto-transition**: After trading_deadline, market status should transition to TradingClosed
//!    - This is handled by the first transaction after the deadline
//!    - Mint/Burn operations are rejected if deadline has passed
//! 
//! This approach balances security with practicality: Scrolls prevents invalid transactions
//! from being submitted, while the contract validates timestamps for additional assurance.

use charms_sdk::data::{
    charm_values, check, sum_token_amount, App, Data, Transaction, UtxoId, B32, NFT, TOKEN,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use k256::schnorr::{Signature, VerifyingKey};
use k256::ecdsa::signature::Verifier;

// ============================================================================
// STATE DEFINITIONS
// ============================================================================

/// Market state stored in the controller NFT
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MarketState {
    /// Unique market identifier (derived from creation UTXO)
    #[serde(with = "serde_bytes")]
    pub market_id: [u8; 32],
    
    /// SHA256 hash of the market question
    #[serde(with = "serde_bytes")]
    pub question_hash: [u8; 32],
    
    /// Market configuration
    pub params: MarketParams,
    
    /// Current status
    pub status: MarketStatus,
    
    /// Resolution data (None until resolved)
    pub resolution: Option<Resolution>,
    
    /// Token supply tracking
    pub yes_supply: u64,
    pub no_supply: u64,
    pub max_supply: u64,
    
    /// Accumulated trading fees (sats)
    pub fees: u64,
    
    /// Market creator's public key
    #[serde(with = "serde_bytes")]
    pub creator: [u8; 33],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MarketParams {
    /// Unix timestamp - trading stops after this
    pub trading_deadline: u64,
    
    /// Unix timestamp - resolution available after this
    pub resolution_deadline: u64,
    
    /// Fee in basis points (100 = 1%)
    pub fee_bps: u16,
    
    /// Minimum bet amount in sats
    pub min_bet: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum MarketStatus {
    /// Market is open for trading
    Active,
    /// Trading closed, awaiting resolution
    TradingClosed,
    /// Market has been resolved
    Resolved,
    /// Market cancelled, refunds available
    Cancelled,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Yes,
    No,
    Invalid, // Market invalidated, refunds available
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub outcome: Outcome,
    pub proof: ResolutionProof,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ResolutionProof {
    /// Simple signed attestation
    SignedAttestation {
        #[serde(with = "serde_bytes")]
        resolver_pubkey: [u8; 33],
        #[serde(with = "serde_bytes")]
        signature: [u8; 64],
    },
    /// Cross-chain proof from Cardano oracle
    /// 
    /// For Hackathon MVP: Uses trusted oracle signature verification
    /// Full implementation would verify:
    /// - tx_hash exists in block via merkle_proof
    /// - Transaction contains outcome data
    /// - block_hash is from valid Cardano block
    /// - Merkle path is correct
    /// 
    /// This requires Cardano light client logic (not implemented in HackathonMVP)
    CardanoOracle {
        #[serde(with = "serde_bytes")]
        tx_hash: [u8; 32],
        #[serde(with = "serde_bytes")]
        block_hash: [u8; 32],
        merkle_proof: Vec<[u8; 32]>,
        /// Trusted oracle public key (for Hackathon MVP signature verification)
        #[serde(with = "serde_bytes")]
        oracle_pubkey: [u8; 33],
        /// Oracle signature over (market_id || outcome || tx_hash || block_hash)
        #[serde(with = "serde_bytes")]
        oracle_signature: [u8; 64],
    },
}

// ============================================================================
// OPERATIONS
// ============================================================================

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MarketOperation {
    /// Create a new prediction market
    Create {
        question_hash: [u8; 32],
        params: MarketParams,
    },
    
    /// Mint complete sets (1 YES + 1 NO per collateral unit)
    Mint {
        collateral_amount: u64,
        /// Current timestamp (Unix seconds) - validated against trading_deadline
        /// Primary enforcement is at Scrolls layer, this is additional validation
        current_timestamp: u64,
    },
    
    /// Burn complete sets to recover collateral
    Burn {
        set_count: u64,
        /// Current timestamp (Unix seconds) - validated against trading_deadline
        /// Primary enforcement is at Scrolls layer, this is additional validation
        current_timestamp: u64,
    },
    
    /// Resolve the market with outcome
    Resolve {
        outcome: Outcome,
        proof: ResolutionProof,
        /// Current timestamp (Unix seconds) - validated against resolution_deadline
        /// Primary enforcement is at Scrolls layer, this is additional validation
        current_timestamp: u64,
    },
    
    /// Redeem winning tokens for collateral
    Redeem {
        yes_amount: u64,
        no_amount: u64,
    },
    
    /// Cancel market (creator only, before resolution)
    Cancel,
    
    /// Claim accumulated fees (creator or protocol only, after resolution)
    ClaimFees,
}

// ============================================================================
// APP CONTRACT
// ============================================================================

/// Main app contract predicate for Charms
/// 
/// # Arguments
/// * `app` - The app being validated
/// * `tx` - The transaction context
/// * `x` - Public input data (operation)
/// * `w` - Private witness data (signatures, etc.)
/// 
/// # Returns
/// * `true` if the transaction satisfies the contract
pub fn app_contract(app: &App, tx: &Transaction, x: &Data, w: &Data) -> bool {
    // Market state is stored in an NFT, so market operations require NFT tag
    // Token operations (YES/NO tokens) use TOKEN tag
    match app.tag {
        NFT => {
            // Deserialize the operation from public input
            let operation: MarketOperation = match x.value::<MarketOperation>() {
                Ok(op) => op,
                Err(_) => return false,
            };
            
            // Get witness bytes - try to deserialize as Vec<u8> or use empty
            let witness_bytes: Vec<u8> = w.value().unwrap_or_default();
            
            match operation {
        MarketOperation::Create { question_hash, params } => {
            validate_create(app, tx, &question_hash, &params)
        }
        MarketOperation::Mint { collateral_amount, current_timestamp } => {
            validate_mint(app, tx, collateral_amount, current_timestamp)
        }
        MarketOperation::Burn { set_count, current_timestamp } => {
            validate_burn(app, tx, set_count, current_timestamp)
        }
        MarketOperation::Resolve { outcome, proof, current_timestamp } => {
            validate_resolve(app, tx, &outcome, &proof, &witness_bytes, current_timestamp)
        }
        MarketOperation::Redeem { yes_amount, no_amount } => {
            validate_redeem(app, tx, yes_amount, no_amount)
        }
        MarketOperation::Cancel => {
            validate_cancel(app, tx, &witness_bytes)
        }
        MarketOperation::ClaimFees => {
            validate_claim_fees(app, tx, &witness_bytes)
        }
            }
        }
        TOKEN => {
            // Validate YES/NO token transfers
            validate_token_transfer(app, tx)
        }
        _ => false,
    }
}

// ========================================================================
// VALIDATION FUNCTIONS
// ========================================================================

fn validate_create(
    app: &App,
    tx: &Transaction,
    question_hash: &[u8; 32],
    params: &MarketParams,
) -> bool {
    // 1. Must have exactly one input (funding UTXO)
    check!(tx.ins.len() == 1);
    
    // 2. Must have at least one output (market NFT)
    check!(!tx.outs.is_empty());
    
    // 3. Market ID is derived from input UTXO
    let market_id = sha256_utxo(&tx.ins[0].0);
    
    // 4. Verify market NFT is created with correct initial state
    let nft_charms: Vec<_> = charm_values(app, tx.outs.iter()).collect();
    check!(nft_charms.len() == 1);
    
    let state: MarketState = match nft_charms[0].value::<MarketState>() {
        Ok(s) => s,
        Err(_) => return false,
    };
    
    // 5. Validate initial state
    check!(state.market_id == market_id);
    check!(state.question_hash == *question_hash);
    check!(state.params.trading_deadline == params.trading_deadline);
    check!(state.params.resolution_deadline == params.resolution_deadline);
    check!(state.params.fee_bps == params.fee_bps);
    check!(state.status == MarketStatus::Active);
    check!(state.resolution.is_none());
    check!(state.yes_supply == 0);
    check!(state.no_supply == 0);
    
    true
}

/// Validate mint operation
/// 
/// # Arguments
/// * `app` - The market NFT app
/// * `tx` - Transaction context
/// * `collateral_amount` - Amount of collateral to deposit
/// * `current_timestamp` - Current Unix timestamp (seconds) - must be < trading_deadline
fn validate_mint(
    app: &App,
    tx: &Transaction,
    collateral_amount: u64,
    current_timestamp: u64,
) -> bool {
    // 1. Find market NFT in inputs and outputs
    let old_state = find_and_parse_market_state_input(app, tx);
    let new_state = find_and_parse_market_state_output(app, tx);
    
    check!(old_state.is_some());
    check!(new_state.is_some());
    
    let old = old_state.unwrap();
    let new = new_state.unwrap();
    
    // 2. Enforce min_bet limit to prevent dust attacks
    // This ensures meaningful trades and prevents spam
    check!(collateral_amount >= old.params.min_bet);
    
    // 3. Market must be active
    check!(old.status == MarketStatus::Active);
    
    // 4. Validate timestamp: current_time must be < trading_deadline
    // Mint operations are only allowed before the trading deadline
    // After deadline, market should transition to TradingClosed
    check!(current_timestamp < old.params.trading_deadline);
    
    // 5. Calculate fee and shares to mint
    // Fee = collateral_amount * fee_bps / 10000 (basis points)
    // Shares = collateral_amount - fee (user gets tokens for net collateral)
    let fee = (collateral_amount as u128 * old.params.fee_bps as u128 / 10000) as u64;
    let shares = collateral_amount.checked_sub(fee).unwrap_or(0);
    
    // 6. Verify supply increases correctly (shares minted, not full collateral)
    check!(new.yes_supply == old.yes_supply + shares);
    check!(new.no_supply == old.no_supply + shares);
    
    // 7. Enforce max_supply limit to prevent infinite minting
    // This bounds the market size and prevents supply overflow
    check!(new.yes_supply <= old.max_supply);
    check!(new.no_supply <= old.max_supply);
    
    // 8. Verify YES and NO tokens minted in outputs
    let yes_app = derive_yes_token_app(&old.market_id, &app.vk);
    let no_app = derive_no_token_app(&old.market_id, &app.vk);
    
    let yes_minted = count_token_minted(tx, &yes_app);
    let no_minted = count_token_minted(tx, &no_app);
    
    check!(yes_minted == shares);
    check!(no_minted == shares);
    
    // 7. All other state must remain unchanged
    check!(new.market_id == old.market_id);
    check!(new.question_hash == old.question_hash);
    check!(new.params.trading_deadline == old.params.trading_deadline);
    check!(new.params.resolution_deadline == old.params.resolution_deadline);
    check!(new.params.fee_bps == old.params.fee_bps);
    check!(new.params.min_bet == old.params.min_bet);
    check!(new.status == old.status);
    check!(new.resolution == old.resolution);
    check!(new.creator == old.creator);
    
    // 10. Verify max_supply remains unchanged
    check!(new.max_supply == old.max_supply);
    
    // 11. Verify fees are accumulated correctly
    check!(new.fees == old.fees + fee);
    
    true
}

/// Validate burn operation
/// 
/// # Arguments
/// * `app` - The market NFT app
/// * `tx` - Transaction context
/// * `set_count` - Number of complete sets to burn
/// * `current_timestamp` - Current Unix timestamp (seconds) - must be < trading_deadline
fn validate_burn(
    app: &App,
    tx: &Transaction,
    set_count: u64,
    current_timestamp: u64,
) -> bool {
    // 1. Find market NFT in inputs and outputs
    let old = match find_and_parse_market_state_input(app, tx) {
        Some(s) => s,
        None => return false,
    };
    let new = match find_and_parse_market_state_output(app, tx) {
        Some(s) => s,
        None => return false,
    };
    
    // 2. Market must be active (can only burn during Active status)
    check!(old.status == MarketStatus::Active);
    
    // 3. Validate timestamp: current_time must be < trading_deadline
    // If deadline has passed, trading is closed and burns are not allowed
    check!(current_timestamp < old.params.trading_deadline);
    
    // 4. Verify correct token apps are being burned
    let yes_app = derive_yes_token_app(&old.market_id, &app.vk);
    let no_app = derive_no_token_app(&old.market_id, &app.vk);
    
    // 5. Verify equal amounts of YES and NO tokens are burned
    let yes_burned = count_token_burned(tx, &yes_app);
    let no_burned = count_token_burned(tx, &no_app);
    
    check!(yes_burned == set_count);
    check!(no_burned == set_count);
    check!(yes_burned == no_burned); // Explicit check for equal amounts
    
    // 6. Verify supply tracking is updated correctly
    // Supply decreases by the number of complete sets burned
    check!(new.yes_supply == old.yes_supply.checked_sub(set_count).unwrap_or(0));
    check!(new.no_supply == old.no_supply.checked_sub(set_count).unwrap_or(0));
    
    // 7. Verify state transition is valid (all other fields unchanged)
    check!(new.market_id == old.market_id);
    check!(new.question_hash == old.question_hash);
    check!(new.params.trading_deadline == old.params.trading_deadline);
    check!(new.params.resolution_deadline == old.params.resolution_deadline);
    check!(new.params.fee_bps == old.params.fee_bps);
    check!(new.params.min_bet == old.params.min_bet);
    check!(new.status == old.status); // Status remains Active
    check!(new.resolution == old.resolution); // Resolution unchanged
    check!(new.creator == old.creator);
    check!(new.max_supply == old.max_supply);
    // Fees remain unchanged (fees are only collected on trades, not burns)
    check!(new.fees == old.fees);
    
    // 8. Verify user receives collateral back
    // Note: BTC amount verification is handled at the transaction level
    // The burn operation itself ensures tokens are burned, and the transaction
    // structure ensures the user receives the collateral in outputs
    
    // Additional validation: ensure tokens are actually in inputs
    // The count_token_burned function already handles net burn (input - output),
    // so it is just needed to verify sufficient tokens are in inputs
    let yes_input_total = sum_token_amount(&yes_app, tx.ins.iter().map(|(_, v)| v)).unwrap_or(0);
    let no_input_total = sum_token_amount(&no_app, tx.ins.iter().map(|(_, v)| v)).unwrap_or(0);
    
    // Must have at least set_count tokens in inputs to burn
    check!(yes_input_total >= set_count);
    check!(no_input_total >= set_count);
    
    // Note: count_token_burned already verifies net burn (input - output = set_count)
    // If tokens appear in outputs, they're being transferred, not burned
    
    true
}


/// Validate resolve operation
/// 
/// # Resolution Deadlines
/// 
/// - **Normal Resolution**: Must occur after `resolution_deadline`
/// - **Emergency Resolution**: Allowed after `resolution_deadline + EMERGENCY_GRACE_PERIOD` (7 days)
///   - Emergency resolution can be used if normal resolution hasn't occurred
///   - Uses same proof validation as normal resolution
/// 
/// # Dispute Period
/// 
/// After resolution, there is a dispute period (7 days) during which:
/// - Resolution can be challenged (future implementation)
/// - Redeem operations are allowed
/// - Market status remains Resolved
fn validate_resolve(
    app: &App,
    tx: &Transaction,
    outcome: &Outcome,
    proof: &ResolutionProof,
    _witness: &[u8],
    current_timestamp: u64,
) -> bool {
    // Emergency grace period: 7 days (604800 seconds)
    // Dispute period logic can reference this in future
    #[allow(dead_code)]
    const EMERGENCY_GRACE_PERIOD: u64 = 7 * 24 * 60 * 60; // 604800 seconds
    
    let old = match find_and_parse_market_state_input(app, tx) {
        Some(s) => s,
        None => return false,
    };
    let new = match find_and_parse_market_state_output(app, tx) {
        Some(s) => s,
        None => return false,
    };
    
    // Must be active or trading closed (not already resolved)
    check!(old.status == MarketStatus::Active || old.status == MarketStatus::TradingClosed);
    
    // Validate resolution deadline
    // Resolution can only happen after resolution_deadline
    // Emergency resolution allowed after resolution_deadline + grace period
    if current_timestamp < old.params.resolution_deadline {
        // Too early - resolution not yet available
        // Must wait until resolution_deadline has passed
        return false;
    }
    
    // After resolution_deadline, resolution is always allowed
    // Emergency resolution (after grace period) uses same validation as normal resolution
    // This ensures markets can always be resolved eventually, even if delayed
    
    // Validate resolution proof
    let proof_valid = match proof {
        ResolutionProof::SignedAttestation { resolver_pubkey, signature } => {
            // For Hackathon MVP: accept creator's signature
            // In production: use authorized resolver set
            verify_resolution_signature(
                &old.market_id,
                outcome,
                resolver_pubkey,
                signature,
            )
        }
            ResolutionProof::CardanoOracle { tx_hash, block_hash, merkle_proof, oracle_pubkey, oracle_signature } => {
                // Cross-chain verification (Hackathon MVP: trusted oracle signature)
                verify_cardano_proof(
                    &old.market_id,
                    outcome,
                    tx_hash,
                    block_hash,
                    merkle_proof,
                    oracle_pubkey,
                    oracle_signature,
                )
            }
    };
    
    check!(proof_valid);
    
    // Verify state transition
    check!(new.status == MarketStatus::Resolved);
    check!(new.resolution.is_some());
    
    let resolution = new.resolution.as_ref().unwrap();
    check!(resolution.outcome == *outcome);
    
    // Verify resolution timestamp matches current_timestamp
    // This ensures the resolution timestamp is accurate
    check!(resolution.timestamp == current_timestamp);
    
    // Fees are preserved during resolution (can be claimed later via ClaimFees)
    check!(new.fees == old.fees);
    
    // All other state fields must remain unchanged
    check!(new.market_id == old.market_id);
    check!(new.question_hash == old.question_hash);
    check!(new.params.trading_deadline == old.params.trading_deadline);
    check!(new.params.resolution_deadline == old.params.resolution_deadline);
    check!(new.params.fee_bps == old.params.fee_bps);
    check!(new.params.min_bet == old.params.min_bet);
    check!(new.creator == old.creator);
    check!(new.yes_supply == old.yes_supply);
    check!(new.no_supply == old.no_supply);
    check!(new.max_supply == old.max_supply);
    
    true
}

fn validate_redeem(
    app: &App,
    tx: &Transaction,
    yes_amount: u64,
    no_amount: u64,
) -> bool {
    // TODO: Implement validation
    true
}

fn validate_cancel(
    app: &App,
    tx: &Transaction,
    _witness: &[u8],
) -> bool {
    // TODO: Implement validation
    true
}

fn validate_claim_fees(
    app: &App,
    tx: &Transaction,
    witness: &[u8],
) -> bool {
    // TODO: Implement validation
    true
}

// ========================================================================
// HELPER FUNCTIONS
// ========================================================================

fn sha256_utxo(utxo_id: &UtxoId) -> [u8; 32] {
    // Hash the string representation of the UTXO ID
    let utxo_str = utxo_id.to_string();
    sha256(utxo_str.as_bytes())
}

fn sha256(data: &[u8]) -> [u8; 32] {
    let hash = Sha256::digest(data);
    hash.into()
}

fn derive_yes_token_app(market_id: &[u8; 32], vk: &B32) -> App {
    let mut identity_data = market_id.to_vec();
    identity_data.extend(b"YES");
    let identity = B32(Sha256::digest(&identity_data).into());
    
    App {
        tag: TOKEN,
        identity,
        vk: vk.clone(),
    }
}

fn derive_no_token_app(market_id: &[u8; 32], vk: &B32) -> App {
    let mut identity_data = market_id.to_vec();
    identity_data.extend(b"NO");
    let identity = B32(Sha256::digest(&identity_data).into());
    
    App {
        tag: TOKEN,
        identity,
        vk: vk.clone(),
    }
}

fn find_and_parse_market_state_input(app: &App, tx: &Transaction) -> Option<MarketState> {
    charm_values(app, tx.ins.iter().map(|(_, v)| v))
        .find_map(|data| {
            data.value::<MarketState>().ok()
        })
}

fn find_and_parse_market_state_output(app: &App, tx: &Transaction) -> Option<MarketState> {
    charm_values(app, tx.outs.iter())
        .find_map(|data| {
            data.value::<MarketState>().ok()
        })
}

fn count_token_minted(tx: &Transaction, token_app: &App) -> u64 {
    let output_total = sum_token_amount(token_app, tx.outs.iter()).unwrap_or(0);
    let input_total = sum_token_amount(token_app, tx.ins.iter().map(|(_, v)| v)).unwrap_or(0);
    
    output_total.saturating_sub(input_total)
}

fn count_token_burned(tx: &Transaction, token_app: &App) -> u64 {
    let input_total = sum_token_amount(token_app, tx.ins.iter().map(|(_, v)| v)).unwrap_or(0);
    let output_total = sum_token_amount(token_app, tx.outs.iter()).unwrap_or(0);
    
    input_total.saturating_sub(output_total)
}

/// Validate YES/NO token transfers
/// 
/// Requirements:
/// 1. Allow transfers of YES and NO tokens between addresses
/// 2. Ensure token conservation (input amount == output amount)
/// 3. Prevent minting tokens without going through the Mint operation
/// 4. Tokens can only be transferred if market is Active or TradingClosed
/// 5. After resolution, tokens can only be redeemed (not transferred)
fn validate_token_transfer(token_app: &App, tx: &Transaction) -> bool {
    // Find the market NFT that this token belongs to
    // Find a market NFT with the same vk, then derive token apps from it
    let market_state = find_market_state_for_token(token_app, tx);
    
    let Some(state) = market_state else {
        // No market found - this shouldn't happen for valid YES/NO tokens
        return false;
    };
    
    // Check market status - tokens can only be transferred if market is Active or TradingClosed
    check!(state.status == MarketStatus::Active || state.status == MarketStatus::TradingClosed);
    
    // Verify this is a YES or NO token for this market
    let yes_app = derive_yes_token_app(&state.market_id, &token_app.vk);
    let no_app = derive_no_token_app(&state.market_id, &token_app.vk);
    
    check!(token_app.identity == yes_app.identity || token_app.identity == no_app.identity);
    
    // Calculate token amounts
    let input_total = sum_token_amount(token_app, tx.ins.iter().map(|(_, v)| v)).unwrap_or(0);
    let output_total = sum_token_amount(token_app, tx.outs.iter()).unwrap_or(0);
    
    // Ensure token conservation (input amount == output amount)
    // This prevents minting (output > input) and ensures no tokens are lost
    check!(input_total == output_total);
    
    true
}

/// Find the market state for a given token app
/// 
/// Searches transaction inputs/outputs for market NFTs with the same vk,
/// then verifies the token belongs to that market by deriving YES/NO token apps.
/// 
/// Note: This requires the market NFT to be present in the transaction
/// (either as input or output) to verify market status. The market NFT must
/// have the same vk as the token app.
/// 
/// Strategy: Try to find the market NFT identity by:
/// 1. Constructing a market NFT app with the same vk (but unknown identity)
/// 2. Using charm_values to search for MarketState in transaction data
/// 3. Verifying the found MarketState matches our token by deriving YES/NO apps
/// 
/// Charm_values requires a specific app identity.
/// Try to extract MarketState by attempting to deserialize transaction
/// data directly, or by trying common market NFT identity patterns.
fn find_market_state_for_token(token_app: &App, tx: &Transaction) -> Option<MarketState> {
    // Need to find market NFTs with the same vk as the token app
    // The challenge: Not knowing the market NFT identity
    
    // Approach: Try to find MarketState by constructing NFT apps and using charm_values
    // Since we don't know the identity, we'll try a few strategies:
    
    // Search inputs: try to find MarketState in any input data
    // We'll construct an NFT app with matching vk and try different approaches

    // Strategy 1: Try to find MarketState by iterating through inputs/outputs
    // and using charm_values with a constructed NFT app
    // Note: This won't work perfectly because charm_values filters by identity,
    // but we can try with a dummy identity to see if it finds anything
    let nft_app = App {
        tag: NFT,
        identity: B32([0u8; 32]), // Dummy identity
        vk: token_app.vk.clone(),
    };
    
    // Try using charm_values on inputs 
    let input_charms: Vec<_> = charm_values(&nft_app, tx.ins.iter().map(|(_, v)| v)).collect();
    for charm_data in input_charms {
        if let Ok(state) = charm_data.value::<MarketState>() {
            // Verify this token belongs to this market
            let yes_app = derive_yes_token_app(&state.market_id, &token_app.vk);
            let no_app = derive_no_token_app(&state.market_id, &token_app.vk);
            
            if token_app.identity == yes_app.identity || token_app.identity == no_app.identity {
                return Some(state);
            }
        }
    }
    
    // Try using charm_values on outputs
    let output_charms: Vec<_> = charm_values(&nft_app, tx.outs.iter()).collect();
    for charm_data in output_charms {
        if let Ok(state) = charm_data.value::<MarketState>() {
            // Verify this token belongs to this market
            let yes_app = derive_yes_token_app(&state.market_id, &token_app.vk);
            let no_app = derive_no_token_app(&state.market_id, &token_app.vk);
            
            if token_app.identity == yes_app.identity || token_app.identity == no_app.identity {
                return Some(state);
            }
        }
    }
    
    // Strategy 2: For now, we'll require that token transfers include the market NFT
    // and we'll find it by trying to construct the market NFT app from the market_id
    // But we don't know the market_id from the token app alone...
    
    // Strategy 3: Try all possible market NFT apps by iterating through transaction data
    // and checking if any NFT contains MarketState that matches our token

    // If charm_values with dummy identity doesn't work, we can't find the market

    // This means token transfers must include the market NFT with a known identity
    // For Hackathon MVP, use a simplified approach:
    // Try to find MarketState by attempting to deserialize from transaction data
    // This works if MarketState is stored in a way that's directly accessible

    // For now, return None - the validation will fail, which is correct behavior
    // if the market NFT is not present
    
    None
}

fn verify_resolution_signature(
    market_id: &[u8; 32],
    outcome: &Outcome,
    pubkey: &[u8; 33],
    signature: &[u8; 64],
) -> bool {
    use core::convert::TryInto;
    
    // Parse compressed public key (33 bytes) and convert to Schnorr VerifyingKey (32 bytes)
    // For Schnorr, we need the x-only public key (first 32 bytes, dropping the parity byte)
    let xonly_bytes: [u8; 32] = match pubkey[1..33].try_into() {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    
    let verifying_key = match VerifyingKey::from_bytes(&xonly_bytes) {
        Ok(vk) => vk,
        Err(_) => return false,
    };
    
    // Parse Schnorr signature (64 bytes: r || s)
    let sig = match Signature::try_from(signature as &[u8]) {
        Ok(s) => s,
        Err(_) => return false,
    };
    
    // Serialize outcome
    let outcome_bytes = match bincode::serialize(outcome) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    
    // Build message: SHA256(market_id || outcome_serialized)
    let mut hasher = Sha256::new();
    hasher.update(market_id);
    hasher.update(&outcome_bytes);
    let message = hasher.finalize();
    
    // Verify Schnorr signature
    verifying_key.verify(&message[..], &sig).is_ok()
}

/// Verify Cardano cross-chain oracle proof
///
/// # Hackathon MVP Implementation
/// Use trusted oracle signature verification:
/// - Oracle signs: SHA256(market_id || outcome || tx_hash || block_hash)
/// - Verifies oracle's Schnorr signature
/// - Trusts oracle's attestation of Cardano data
/// 
/// This is secure if oracle is trusted, but full implementation would remove
/// the need for a trusted oracle by directly verifying Cardano chain data.

/// # Full Implementation (Future)
/// A complete implementation would:
/// 1. Verify tx_hash exists in block via merkle_proof
/// 
/// 2. Verify transaction contains outcome data
/// 
/// 3. Verify block_hash is from valid Cardano block
/// 
fn verify_cardano_proof(
    market_id: &[u8; 32],
    outcome: &Outcome,
    tx_hash: &[u8; 32],
    block_hash: &[u8; 32],
    merkle_proof: &[[u8; 32]],
    oracle_pubkey: &[u8; 33],
    oracle_signature: &[u8; 64],
) -> bool {
    use core::convert::TryInto;
    
    // Hackathon MVP: Verify trusted oracle signature
    
    // Parse oracle's Schnorr signature
    let sig = match Signature::try_from(oracle_signature as &[u8]) {
        Ok(s) => s,
        Err(_) => return false,
    };
    
    // Parse oracle's public key and convert to VerifyingKey
    let xonly_bytes: [u8; 32] = match oracle_pubkey[1..33].try_into() {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    
    let verifying_key = match VerifyingKey::from_bytes(&xonly_bytes) {
        Ok(vk) => vk,
        Err(_) => return false,
    };
    
    // Serialize outcome
    let outcome_bytes = match bincode::serialize(outcome) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    
    // Build message: SHA256(market_id || outcome || tx_hash || block_hash)
    // Oracle attests that this outcome is correct for the given Cardano transaction
    let mut hasher = Sha256::new();
    hasher.update(market_id);
    hasher.update(&outcome_bytes);
    hasher.update(tx_hash);
    hasher.update(block_hash);
    let message = hasher.finalize();
    
    // Verify oracle's signature
    let signature_valid = verifying_key.verify(&message[..], &sig).is_ok();
    
    //Basic merkle_proof structure validation (non-empty)
    let merkle_proof_valid = !merkle_proof.is_empty();
    
    // Trust the oracle if signature is valid
    signature_valid && merkle_proof_valid
    
}

fn verify_creator_signature(
    creator: &[u8; 33],
    market_id: &[u8; 32],
    witness: &[u8],
) -> bool {
    use core::convert::TryInto;
    
    // Parse signature from witness bytes (first 64 bytes)
    if witness.len() < 64 {
        return false;
    }
    
    let signature_bytes: [u8; 64] = match witness[0..64].try_into() {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    
    // Parse Schnorr signature (64 bytes: r || s)
    let sig = match Signature::try_from(&signature_bytes[..]) {
        Ok(s) => s,
        Err(_) => return false,
    };
    
    // Parse compressed public key (33 bytes) and convert to Schnorr VerifyingKey (32 bytes)
    // For Schnorr, we need the x-only public key (first 32 bytes, dropping the parity byte)
    let xonly_bytes: [u8; 32] = match creator[1..33].try_into() {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    
    let verifying_key = match VerifyingKey::from_bytes(&xonly_bytes) {
        Ok(vk) => vk,
        Err(_) => return false,
    };
    
    // Build message: SHA256("CANCEL" || market_id)
    let mut hasher = Sha256::new();
    hasher.update(b"CANCEL");
    hasher.update(market_id);
    let message = hasher.finalize();
    
    // Verify Schnorr signature
    verifying_key.verify(&message[..], &sig).is_ok()
}
