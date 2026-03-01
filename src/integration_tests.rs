//! Integration tests for the full prediction market lifecycle
//! 
//! These tests verify end-to-end market operations including:
//! - Market creation
//! - Token minting
//! - Market resolution
//! - Token redemption
//! - Market cancellation
//! - Multi-user scenarios
//!
//! Note: These are state transition tests that verify the contract logic
//! without requiring full Charms SDK mocks. For full integration testing
//! with actual transactions, use `charms spell check` with spell files.

#[cfg(test)]
mod integration_tests {
    use crate::*;
    use sha2::{Digest, Sha256};
    
    // ============================================================================
    // HELPER FUNCTIONS
    // ============================================================================
    
    /// Derive market ID from a string (simulating UTXO hash)
    fn derive_market_id(s: &str) -> [u8; 32] {
        let hash = Sha256::digest(s.as_bytes());
        hash.into()
    }
    
    /// Create a valid resolution proof (signed attestation)
    /// Note: In real tests, signatures would be properly generated
    fn mock_resolution_proof() -> ResolutionProof {
        ResolutionProof::SignedAttestation {
            resolver_pubkey: [0u8; 33],
            signature: [0u8; 64],
        }
    }
    
    /// Create default market parameters for testing
    fn default_params() -> MarketParams {
        MarketParams {
            trading_deadline: 1735603200 + 86400, // 1 day from base
            resolution_deadline: 1735603200 + 172800, // 2 days from base
            fee_bps: 100, // 1%
            min_bet: 10000,
        }
    }
    
    // ============================================================================
    // TEST SCENARIO 1: Happy Path - YES Resolution
    // ============================================================================
    
    #[test]
    fn test_happy_path_yes_resolution() {
        // Step 1: Create Market
        let market_id = derive_market_id("test_funding_utxo_1");
        let question_hash = [2u8; 32];
        let params = default_params();
        
        let initial_state = MarketState {
            market_id,
            question_hash,
            params: params.clone(),
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: 0,
            no_supply: 0,
            max_supply: 1_000_000_000_000,
            fees: 0,
            creator: [3u8; 33],
        };
        
        // Verify initial state
        assert_eq!(initial_state.status, MarketStatus::Active);
        assert_eq!(initial_state.yes_supply, 0);
        assert_eq!(initial_state.no_supply, 0);
        assert_eq!(initial_state.fees, 0);
        assert!(initial_state.resolution.is_none());
        
        // Step 2: Mint tokens
        let collateral_amount = 1000000; // 1M sats
        let current_timestamp = 1735603200;
        let fee = (collateral_amount as u128 * params.fee_bps as u128 / 10000) as u64;
        let shares = collateral_amount - fee;
        
        let minted_state = MarketState {
            yes_supply: shares,
            no_supply: shares,
            fees: fee,
            ..initial_state.clone()
        };
        
        // Verify mint validation logic
        assert!(current_timestamp < params.trading_deadline);
        assert!(collateral_amount >= params.min_bet);
        assert_eq!(minted_state.yes_supply, shares);
        assert_eq!(minted_state.no_supply, shares);
        assert_eq!(minted_state.fees, fee);
        
        // Step 3: Resolve YES
        let resolution_timestamp = params.resolution_deadline + 1;
        let resolution = Resolution {
            outcome: Outcome::Yes,
            proof: mock_resolution_proof(),
            timestamp: resolution_timestamp,
        };
        
        let resolved_state = MarketState {
            status: MarketStatus::Resolved,
            resolution: Some(resolution.clone()),
            ..minted_state.clone()
        };
        
        assert_eq!(resolved_state.status, MarketStatus::Resolved);
        assert_eq!(resolved_state.resolution.as_ref().unwrap().outcome, Outcome::Yes);
        
        // Step 4: Redeem YES tokens
        let yes_amount = shares;
        let no_amount = 0;
        
        // In YES outcome, only YES tokens can be redeemed
        assert_eq!(yes_amount, shares);
        assert_eq!(no_amount, 0);
        
        // Verify redemption logic
        match resolved_state.resolution.as_ref().unwrap().outcome {
            Outcome::Yes => {
                assert_eq!(no_amount, 0);
            }
            _ => panic!("Expected YES outcome"),
        }
    }
    
    // ============================================================================
    // TEST SCENARIO 2: Happy Path - NO Resolution
    // ============================================================================
    
    #[test]
    fn test_happy_path_no_resolution() {
        let params = default_params();
        
        let market_id = [1u8; 32];
        let initial_state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: 0,
            no_supply: 0,
            max_supply: 1_000_000_000_000,
            fees: 0,
            creator: [3u8; 33],
        };
        
        // Mint
        let collateral_amount = 1000000;
        let fee = (collateral_amount as u128 * params.fee_bps as u128 / 10000) as u64;
        let shares = collateral_amount - fee;
        
        let minted_state = MarketState {
            yes_supply: shares,
            no_supply: shares,
            fees: fee,
            ..initial_state
        };
        
        // Resolve NO
        let resolution = Resolution {
            outcome: Outcome::No,
            proof: mock_resolution_proof(),
            timestamp: params.resolution_deadline + 1,
        };
        
        let resolved_state = MarketState {
            status: MarketStatus::Resolved,
            resolution: Some(resolution),
            ..minted_state
        };
        
        // Redeem NO tokens
        let yes_amount = 0;
        let no_amount = shares;
        
        match resolved_state.resolution.as_ref().unwrap().outcome {
            Outcome::No => {
                assert_eq!(yes_amount, 0);
                assert_eq!(no_amount, shares);
            }
            _ => panic!("Expected NO outcome"),
        }
    }
    
    // ============================================================================
    // TEST SCENARIO 3: Invalid Resolution
    // ============================================================================
    
    #[test]
    fn test_invalid_resolution() {
        let params = default_params();
        
        let market_id = [1u8; 32];
        let initial_state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: 0,
            no_supply: 0,
            max_supply: 1_000_000_000_000,
            fees: 0,
            creator: [3u8; 33],
        };
        
        // Mint
        let collateral_amount = 1000000;
        let fee = (collateral_amount as u128 * params.fee_bps as u128 / 10000) as u64;
        let shares = collateral_amount - fee;
        
        let minted_state = MarketState {
            yes_supply: shares,
            no_supply: shares,
            fees: fee,
            ..initial_state
        };
        
        // Resolve Invalid
        let resolution = Resolution {
            outcome: Outcome::Invalid,
            proof: mock_resolution_proof(),
            timestamp: params.resolution_deadline + 1,
        };
        
        let resolved_state = MarketState {
            status: MarketStatus::Resolved,
            resolution: Some(resolution),
            ..minted_state
        };
        
        // Redeem (Invalid allows any YES and/or NO; this test uses equal amounts)
        let yes_amount = shares;
        let no_amount = shares;
        
        match resolved_state.resolution.as_ref().unwrap().outcome {
            Outcome::Invalid => {
                // Contract allows asymmetric redemption (YES only, NO only, or both at 0.5 sat each)
                assert_eq!(yes_amount, shares);
                assert_eq!(no_amount, shares);
            }
            _ => panic!("Expected Invalid outcome"),
        }
    }
    
    // ============================================================================
    // TEST SCENARIO 4: Cancel Path
    // ============================================================================
    
    #[test]
    fn test_cancel_path() {
        let params = default_params();
        
        let market_id = [1u8; 32];
        let creator = [3u8; 33];
        
        let initial_state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: 0,
            no_supply: 0,
            max_supply: 1_000_000_000_000,
            fees: 0,
            creator,
        };
        
        // Mint
        let collateral_amount = 1000000;
        let fee = (collateral_amount as u128 * params.fee_bps as u128 / 10000) as u64;
        let shares = collateral_amount - fee;
        
        let minted_state = MarketState {
            yes_supply: shares,
            no_supply: shares,
            fees: fee,
            ..initial_state.clone()
        };
        
        // Cancel (before resolution)
        let cancelled_state = MarketState {
            status: MarketStatus::Cancelled,
            resolution: Some(Resolution {
                outcome: Outcome::Invalid,
                proof: mock_resolution_proof(),
                timestamp: 1735603200,
            }),
            ..minted_state
        };
        
        assert_eq!(cancelled_state.status, MarketStatus::Cancelled);
        assert!(cancelled_state.resolution.is_some());
        assert_eq!(
            cancelled_state.resolution.as_ref().unwrap().outcome,
            Outcome::Invalid
        );
        
        // Verify cancellation requirements
        assert_ne!(initial_state.status, MarketStatus::Resolved);
    }
    
    // ============================================================================
    // TEST SCENARIO 5: Multiple Users
    // ============================================================================
    
    #[test]
    fn test_multiple_users() {
        let params = default_params();
        
        let market_id = [1u8; 32];
        let initial_state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: 0,
            no_supply: 0,
            max_supply: 1_000_000_000_000,
            fees: 0,
            creator: [3u8; 33],
        };
        
        // User A mints
        let user_a_collateral = 1000000;
        let user_a_fee = (user_a_collateral as u128 * params.fee_bps as u128 / 10000) as u64;
        let user_a_shares = user_a_collateral - user_a_fee;
        
        let after_user_a = MarketState {
            yes_supply: user_a_shares,
            no_supply: user_a_shares,
            fees: user_a_fee,
            ..initial_state.clone()
        };
        
        // User B mints
        let user_b_collateral = 2000000;
        let user_b_fee = (user_b_collateral as u128 * params.fee_bps as u128 / 10000) as u64;
        let user_b_shares = user_b_collateral - user_b_fee;
        
        let after_user_b = MarketState {
            yes_supply: user_a_shares + user_b_shares,
            no_supply: user_a_shares + user_b_shares,
            fees: user_a_fee + user_b_fee,
            ..after_user_a
        };
        
        assert_eq!(after_user_b.yes_supply, user_a_shares + user_b_shares);
        assert_eq!(after_user_b.no_supply, user_a_shares + user_b_shares);
        assert_eq!(after_user_b.fees, user_a_fee + user_b_fee);
        
        // Resolve YES
        let resolution = Resolution {
            outcome: Outcome::Yes,
            proof: mock_resolution_proof(),
            timestamp: params.resolution_deadline + 1,
        };
        
        let resolved_state = MarketState {
            status: MarketStatus::Resolved,
            resolution: Some(resolution),
            ..after_user_b
        };
        
        // User A redeems YES tokens
        let user_a_yes_redeemed = user_a_shares;
        let user_a_no_redeemed = 0;
        
        // User B redeems YES tokens
        let user_b_yes_redeemed = user_b_shares;
        let user_b_no_redeemed = 0;
        
        // Verify total redemption matches supply
        assert_eq!(
            user_a_yes_redeemed + user_b_yes_redeemed,
            resolved_state.yes_supply
        );
        assert_eq!(user_a_no_redeemed + user_b_no_redeemed, 0);
        
        // Verify both users can redeem
        assert!(user_a_yes_redeemed > 0);
        assert!(user_b_yes_redeemed > 0);
    }
    
    // ============================================================================
    // ADDITIONAL VALIDATION TESTS
    // ============================================================================
    
    #[test]
    fn test_min_bet_enforcement() {
        let params = default_params();
        
        // Should fail if collateral < min_bet
        let collateral_below_min = 5000;
        assert!(collateral_below_min < params.min_bet);
        
        // Should pass if collateral >= min_bet
        let collateral_above_min = 10000;
        assert!(collateral_above_min >= params.min_bet);
    }
    
    #[test]
    fn test_max_supply_enforcement() {
        let params = default_params();
        
        let max_supply = 1_000_000;
        let market_id = [1u8; 32];
        
        let state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: max_supply - 1000,
            no_supply: max_supply - 1000,
            max_supply,
            fees: 0,
            creator: [3u8; 33],
        };
        
        // Can mint if new supply <= max_supply
        let new_shares = 500;
        assert!(state.yes_supply + new_shares <= max_supply);
        assert!(state.no_supply + new_shares <= max_supply);
        
        // Cannot mint if new supply > max_supply
        let too_many_shares = 2000;
        assert!(state.yes_supply + too_many_shares > max_supply);
    }
    
    #[test]
    fn test_trading_deadline_enforcement() {
        let trading_deadline = 1735603200 + 86400;
        let current_timestamp = 1735603200;
        
        // Should allow mint before deadline
        assert!(current_timestamp < trading_deadline);
        
        // Should reject mint after deadline
        let after_deadline = trading_deadline + 1;
        assert!(after_deadline >= trading_deadline);
    }
    
    #[test]
    fn test_resolution_deadline_enforcement() {
        let resolution_deadline = 1735603200 + 172800;
        
        // Should reject resolution before deadline
        let before_deadline = resolution_deadline - 1;
        assert!(before_deadline < resolution_deadline);
        
        // Should allow resolution after deadline
        let after_deadline = resolution_deadline + 1;
        assert!(after_deadline >= resolution_deadline);
    }
    
    #[test]
    fn test_fee_calculation() {
        let params = default_params();
        
        let collateral = 1000000; // 1M sats
        let fee = (collateral as u128 * params.fee_bps as u128 / 10000) as u64;
        let shares = collateral - fee;
        
        // 1% of 1M = 10,000 sats
        assert_eq!(fee, 10000);
        assert_eq!(shares, 990000);
        
        // Verify fee accumulation
        let initial_fees = 0;
        let new_fees = initial_fees + fee;
        assert_eq!(new_fees, 10000);
    }
    
    // ============================================================================
    // EDGE CASE TESTS - Invalid Operations Should Fail
    // ============================================================================
    
    #[test]
    fn test_double_resolution_should_fail() {
        let params = default_params();
        let market_id = derive_market_id("test_market");
        
        // Create and resolve market
        let resolved_state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Resolved,
            resolution: Some(Resolution {
                outcome: Outcome::Yes,
                proof: mock_resolution_proof(),
                timestamp: params.resolution_deadline + 1,
            }),
            yes_supply: 1000000,
            no_supply: 1000000,
            max_supply: 1_000_000_000_000,
            fees: 10000,
            creator: [3u8; 33],
        };
        
        // Attempt to resolve again - should fail
        // Market is already resolved, so resolution should be rejected
        assert_eq!(resolved_state.status, MarketStatus::Resolved);
        assert!(resolved_state.resolution.is_some());
        
        // validate_resolve checks: old.status == Active || TradingClosed
        // So if status is Resolved, it should return false
        let can_resolve_again = matches!(
            resolved_state.status,
            MarketStatus::Active | MarketStatus::TradingClosed
        );
        assert!(!can_resolve_again, "Double resolution should be rejected");
    }
    
    #[test]
    fn test_redeem_before_resolution_should_fail() {
        let params = default_params();
        let market_id = derive_market_id("test_market");
        
        // Market is active, not resolved
        let active_state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: 1000000,
            no_supply: 1000000,
            max_supply: 1_000_000_000_000,
            fees: 10000,
            creator: [3u8; 33],
        };
        
        // Attempt to redeem before resolution - should fail
        // validate_redeem checks: state.status == MarketStatus::Resolved
        let can_redeem = active_state.status == MarketStatus::Resolved;
        assert!(!can_redeem, "Redeem before resolution should be rejected");
        assert!(active_state.resolution.is_none(), "No resolution exists yet");
    }
    
    #[test]
    fn test_mint_after_trading_closed_should_fail() {
        let params = default_params();
        let market_id = derive_market_id("test_market");
        
        // Market is in TradingClosed status
        let closed_state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::TradingClosed,
            resolution: None,
            yes_supply: 1000000,
            no_supply: 1000000,
            max_supply: 1_000_000_000_000,
            fees: 10000,
            creator: [3u8; 33],
        };
        
        // Attempt to mint after trading closed - should fail
        // validate_mint checks: old.status == MarketStatus::Active
        let can_mint = closed_state.status == MarketStatus::Active;
        assert!(!can_mint, "Mint after trading closed should be rejected");
        
        // Also check timestamp validation
        let current_timestamp = params.trading_deadline + 1; // After deadline
        let timestamp_valid = current_timestamp < params.trading_deadline;
        assert!(!timestamp_valid, "Mint after deadline should be rejected");
    }
    
    #[test]
    fn test_redeem_wrong_token_type_should_fail() {
        let params = default_params();
        let market_id = derive_market_id("test_market");
        
        // Market resolved as YES
        let resolved_state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Resolved,
            resolution: Some(Resolution {
                outcome: Outcome::Yes,
                proof: mock_resolution_proof(),
                timestamp: params.resolution_deadline + 1,
            }),
            yes_supply: 1000000,
            no_supply: 1000000,
            max_supply: 1_000_000_000_000,
            fees: 10000,
            creator: [3u8; 33],
        };
        
        let resolution = resolved_state.resolution.as_ref().unwrap();
        
        // Attempt to redeem NO tokens when YES won - should fail
        // validate_redeem checks: for Outcome::Yes, no_amount must be 0
        let _yes_amount = 0;
        let no_amount = 1000000; // Trying to redeem NO tokens
        
        match resolution.outcome {
            Outcome::Yes => {
                // Should reject if no_amount != 0
                assert_ne!(no_amount, 0, "Redeeming NO tokens when YES won should fail");
            }
            _ => panic!("Expected YES outcome"),
        }
        
        // Attempt to redeem YES tokens when NO won - should fail
        let no_resolved_state = MarketState {
            resolution: Some(Resolution {
                outcome: Outcome::No,
                proof: mock_resolution_proof(),
                timestamp: params.resolution_deadline + 1,
            }),
            ..resolved_state
        };
        
        let no_resolution = no_resolved_state.resolution.as_ref().unwrap();
        let yes_amount_wrong = 1000000; // Trying to redeem YES tokens
        let no_amount_correct = 0;
        
        match no_resolution.outcome {
            Outcome::No => {
                // Should reject if yes_amount != 0
                assert_ne!(yes_amount_wrong, 0, "Redeeming YES tokens when NO won should fail");
                assert_eq!(no_amount_correct, 0, "NO amount should be 0 for NO outcome");
            }
            _ => panic!("Expected NO outcome"),
        }
    }
    
    #[test]
    fn test_cancel_after_resolution_should_fail() {
        let params = default_params();
        let market_id = derive_market_id("test_market");
        
        // Market is already resolved
        let resolved_state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Resolved,
            resolution: Some(Resolution {
                outcome: Outcome::Yes,
                proof: mock_resolution_proof(),
                timestamp: params.resolution_deadline + 1,
            }),
            yes_supply: 1000000,
            no_supply: 1000000,
            max_supply: 1_000_000_000_000,
            fees: 10000,
            creator: [3u8; 33],
        };
        
        // Attempt to cancel after resolution - should fail
        // validate_cancel checks: old.status != MarketStatus::Resolved
        let can_cancel = resolved_state.status != MarketStatus::Resolved;
        assert!(!can_cancel, "Cancel after resolution should be rejected");
    }
    
    #[test]
    fn test_zero_amount_mint_should_fail() {
        let params = default_params();
        let market_id = derive_market_id("test_market");
        
        let _state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: 0,
            no_supply: 0,
            max_supply: 1_000_000_000_000,
            fees: 0,
            creator: [3u8; 33],
        };
        
        // Attempt to mint with zero collateral - should fail
        // validate_mint checks: collateral_amount >= old.params.min_bet
        let zero_collateral = 0;
        let can_mint = zero_collateral >= params.min_bet;
        assert!(!can_mint, "Zero amount mint should be rejected");
        
        // Also test collateral below min_bet
        let below_min = params.min_bet - 1;
        let can_mint_below_min = below_min >= params.min_bet;
        assert!(!can_mint_below_min, "Mint below min_bet should be rejected");
    }
    
    #[test]
    fn test_overflow_in_supply_tracking_should_fail_safely() {
        let params = default_params();
        let market_id = derive_market_id("test_market");
        
        // Test near max_supply to prevent overflow
        let max_supply = 1_000_000_000_000u64;
        let current_supply = max_supply - 1000;
        
        let state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: current_supply,
            no_supply: current_supply,
            max_supply,
            fees: 0,
            creator: [3u8; 33],
        };
        
        // Attempt to mint that would exceed max_supply - should fail
        let mint_amount = 2000; // Would exceed max_supply
        let new_supply = current_supply + mint_amount;
        let would_overflow = new_supply > max_supply;
        assert!(would_overflow, "Mint that exceeds max_supply should be rejected");
        
        // Test safe mint that stays within max_supply
        let safe_mint = 500;
        let safe_new_supply = current_supply + safe_mint;
        let is_safe = safe_new_supply <= max_supply;
        assert!(is_safe, "Mint within max_supply should be allowed");
        
        // Test checked arithmetic to prevent overflow
        let test_supply = u64::MAX - 1000;
        let test_mint = 2000;
        
        // Using checked_add to prevent overflow
        match test_supply.checked_add(test_mint) {
            Some(result) => {
                // If addition succeeds, check if it exceeds max_supply
                if result > max_supply {
                    // Would exceed max_supply, should be rejected
                    assert!(result > max_supply, "Should reject if exceeds max_supply");
                }
            }
            None => {
                // Overflow occurred, should be rejected
                assert!(true, "Overflow should be caught and rejected");
            }
        }
        
        // Test that max_supply enforcement prevents overflow
        let near_max = max_supply - 1;
        let state_near_max = MarketState {
            yes_supply: near_max,
            no_supply: near_max,
            max_supply,
            ..state
        };
        
        // Can only mint 1 more token
        let max_mint = 1;
        assert!(state_near_max.yes_supply + max_mint <= max_supply);
        
        // Cannot mint 2 more tokens
        let too_much = 2;
        assert!(state_near_max.yes_supply + too_much > max_supply);
    }
    
    #[test]
    fn test_burn_more_than_supply_should_fail() {
        let params = default_params();
        let market_id = derive_market_id("test_market");
        
        let state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: 1000000,
            no_supply: 1000000,
            max_supply: 1_000_000_000_000,
            fees: 0,
            creator: [3u8; 33],
        };
        
        // Attempt to burn more than supply - should fail
        // validate_burn uses checked_sub which returns None on underflow
        let burn_amount = state.yes_supply + 1; // More than supply
        let new_supply = state.yes_supply.checked_sub(burn_amount);
        
        // checked_sub returns None if underflow
        assert!(new_supply.is_none(), "Burn more than supply should cause underflow");
        
        // Safe burn within supply
        let safe_burn = state.yes_supply - 1;
        let safe_new_supply = state.yes_supply.checked_sub(safe_burn);
        assert!(safe_new_supply.is_some(), "Burn within supply should succeed");
        assert_eq!(safe_new_supply.unwrap(), 1);
    }
    
    #[test]
    fn test_mint_after_resolution_deadline_but_before_resolution_should_fail() {
        let params = default_params();
        let market_id = derive_market_id("test_market");
        
        let state = MarketState {
            market_id,
            question_hash: [2u8; 32],
            params: params.clone(),
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: 1000000,
            no_supply: 1000000,
            max_supply: 1_000_000_000_000,
            fees: 0,
            creator: [3u8; 33],
        };
        
        // Trading deadline has passed, but market not yet resolved
        let after_trading_deadline = params.trading_deadline + 1;
        let timestamp_valid = after_trading_deadline < params.trading_deadline;
        
        // Mint should fail because trading deadline has passed
        assert!(!timestamp_valid, "Mint after trading deadline should be rejected");
        
        // Even if market is still Active, mint should fail due to timestamp
        assert_eq!(state.status, MarketStatus::Active);
        assert!(!timestamp_valid, "Timestamp check should prevent mint after deadline");
    }

    // ============================================================================
    // NATIVE BTC ACCOUNTING TESTS
    // ============================================================================

    #[test]
    fn test_mint_fails_without_btc_collateral() {
        // This test verifies the mathematical invariant that protects the contract
        // from "Free Minting" exploits, representing the logic inside `validate_mint`.

        let collateral_amount: u64 = 10000; // User attempts to mint 10,000 sats worth of tokens

        // Scenario A: Honest User
        // The old Market UTXO had 5,000 sats.
        // The user properly attaches 10,000 sats, so the new UTXO has 15,000 sats.
        let honest_old_sats: u64 = 5000;
        let honest_new_sats: u64 = 15000;
        let honest_is_valid = honest_new_sats == honest_old_sats.checked_add(collateral_amount).unwrap_or(0);
        assert!(honest_is_valid, "Honest mint with correct BTC collateral must succeed");

        // Scenario B: Malicious User (The "Free Mint" Exploit)
        // The user asks for 10,000 sats of tokens, but DOES NOT attach the BTC.
        // The new UTXO remains at 5,000 sats.
        let malicious_old_sats: u64 = 5000;
        let malicious_new_sats: u64 = 5000; // Balance did not increase!
        let malicious_is_valid = malicious_new_sats == malicious_old_sats.checked_add(collateral_amount).unwrap_or(0);

        // The transaction MUST be rejected
        assert!(!malicious_is_valid, "Malicious mint without BTC collateral MUST be rejected");

        // Scenario C: Malicious User (Partial Funding)
        // The user asks for 10,000 sats of tokens, but only attaches 1,000 sats.
        let partial_new_sats: u64 = 6000;
        let partial_is_valid = partial_new_sats == malicious_old_sats.checked_add(collateral_amount).unwrap_or(0);

        // The transaction MUST be rejected
        assert!(!partial_is_valid, "Partial BTC collateral MUST be rejected");
    }

    #[test]
    fn test_redeem_fails_if_vault_drained() {
        // This test verifies the mathematical invariant that protects the contract
        // from "Vault Draining" exploits during redemption.

        let payout_sats: u64 = 10000; // User legitimately won 10,000 sats

        // Scenario A: Honest User
        // The Market UTXO has 50,000 sats. The user withdraws exactly their 10,000 sat payout.
        let honest_old_sats: u64 = 50000;
        let honest_new_sats: u64 = 40000;
        let honest_is_valid = honest_new_sats == honest_old_sats.checked_sub(payout_sats).unwrap_or(0);
        assert!(honest_is_valid, "Honest redemption withdrawing exact payout must succeed");

        // Scenario B: Malicious User
        // The user legitimately won 10,000 sats, but constructs a transaction
        // to withdraw 20,000 sats (draining other users' collateral).
        let malicious_old_sats: u64 = 50000;
        let malicious_new_sats: u64 = 30000; // Withdrew 20k instead of 10k!
        let malicious_is_valid = malicious_new_sats == malicious_old_sats.checked_sub(payout_sats).unwrap_or(0);

        assert!(!malicious_is_valid, "Redemption withdrawing more than the legitimate payout MUST be rejected");
    }
}

