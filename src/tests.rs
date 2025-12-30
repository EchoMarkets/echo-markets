#[cfg(test)]
mod tests {
    use crate::*;
    
    #[test]
    fn test_market_state_serialization() {
        let state = MarketState {
            market_id: [0u8; 32],
            question_hash: [1u8; 32],
            params: MarketParams {
                trading_deadline: 1735603200,
                resolution_deadline: 1735689600,
                fee_bps: 100,
                min_bet: 10000,
            },
            status: MarketStatus::Active,
            resolution: None,
            yes_supply: 0,
            no_supply: 0,
            max_supply: 1_000_000_000_000,
            fees: 0,
            creator: [2u8; 33],
        };
        
        let encoded = bincode::serialize(&state).unwrap();
        let decoded: MarketState = bincode::deserialize(&encoded).unwrap();
        
        assert_eq!(state.market_id, decoded.market_id);
        assert_eq!(state.status, decoded.status);
    }
    
    #[test]
    fn test_outcome_equality() {
        assert_eq!(Outcome::Yes, Outcome::Yes);
        assert_ne!(Outcome::Yes, Outcome::No);
    }
    
    #[test]
    fn test_operation_serialization() {
        let op = MarketOperation::Mint { 
            collateral_amount: 1000000,
            current_timestamp: 1735603200,
        };
        let encoded = bincode::serialize(&op).unwrap();
        let decoded: MarketOperation = bincode::deserialize(&encoded).unwrap();
        
        match decoded {
            MarketOperation::Mint { collateral_amount, current_timestamp } => {
                assert_eq!(collateral_amount, 1000000);
                assert_eq!(current_timestamp, 1735603200);
            }
            _ => panic!("Wrong operation type"),
        }
    }
}

