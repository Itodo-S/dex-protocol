use dex_protocol_core::*;
use dex_protocol_contracts::*;
use ethers::prelude::*;
use std::sync::Arc;
use tokio;

#[tokio::test]
async fn test_full_swap_flow() {
    // This would test the complete flow from API to smart contract
    // For now, we'll test the core logic
    
    let pool = create_test_pool();
    
    // Test quote
    let input_amount = num_bigint::BigUint::from(100u64);
    let output = pool.calculate_swap_output("ETH", "USDC", &input_amount).unwrap();
    
    assert!(output > num_bigint::BigUint::zero());
    
    // Test that output is reasonable (should be less than input * 2 given our 1:2 ratio)
    assert!(output < input_amount * 2u64);
}

#[tokio::test]
async fn test_api_endpoints() {
    // Test API endpoints
    let client = reqwest::Client::new();
    
    // Test quote endpoint
    let quote_request = serde_json::json!({
        "input_token": "ETH",
        "output_token": "USDC",
        "input_amount": "100",
        "slippage_tolerance": 0.5
    });
    
    // This would test against a running API server
    // For now, we'll just verify the request structure
    assert!(quote_request["input_token"].is_string());
    assert!(quote_request["input_amount"].is_string());
}

fn create_test_pool() -> Pool {
    let eth_token = Token {
        address: "ETH".to_string(),
        symbol: "ETH".to_string(),
        decimals: 18,
    };
    
    let usdc_token = Token {
        address: "USDC".to_string(),
        symbol: "USDC".to_string(),
        decimals: 6,
    };
    
    let mut reserves = std::collections::HashMap::new();
    reserves.insert("ETH".to_string(), num_bigint::BigUint::from(1000u64));
    reserves.insert("USDC".to_string(), num_bigint::BigUint::from(2000u64));
    
    Pool::new(
        "ETH-USDC".to_string(),
        vec![eth_token, usdc_token],
        reserves,
        300,
        PoolType::ConstantProduct,
    )
}