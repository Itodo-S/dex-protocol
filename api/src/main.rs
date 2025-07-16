use warp::Filter;
use serde::{Deserialize, Serialize};
use dex_protocol_core::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Serialize, Deserialize)]
struct SwapRequest {
    input_token: String,
    output_token: String,
    input_amount: String,
    slippage_tolerance: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct SwapResponse {
    output_amount: String,
    price_impact: f64,
    fee: String,
    route: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AddLiquidityRequest {
    pool_id: String,
    token_amounts: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PoolInfo {
    id: String,
    tokens: Vec<Token>,
    reserves: HashMap<String, String>,
    total_supply: String,
    fee_rate: u64,
    apy: f64,
    volume_24h: String,
}

type PoolStorage = Arc<RwLock<HashMap<String, Pool>>>;

#[tokio::main]
async fn main() {
    let pools: PoolStorage = Arc::new(RwLock::new(HashMap::new()));
    
    // Initialize some sample pools
    initialize_sample_pools(&pools).await;
    
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE"]);
    
    // Routes
    let quote_route = warp::path("quote")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_pools(pools.clone()))
        .and_then(handle_quote);
    
    let swap_route = warp::path("swap")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_pools(pools.clone()))
        .and_then(handle_swap);
    
    let pools_route = warp::path("pools")
        .and(warp::get())
        .and(with_pools(pools.clone()))
        .and_then(handle_get_pools);
    
    let add_liquidity_route = warp::path("liquidity")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_pools(pools.clone()))
        .and_then(handle_add_liquidity);
    
    let routes = quote_route
        .or(swap_route)
        .or(pools_route)
        .or(add_liquidity_route)
        .with(cors);
    
    println!("DEX API server starting on http://localhost:3030");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

fn with_pools(pools: PoolStorage) -> impl Filter<Extract = (PoolStorage,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || pools.clone())
}

async fn handle_quote(
    request: SwapRequest,
    pools: PoolStorage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let pools_read = pools.read().await;
    
    // Find appropriate pool (simplified - in reality you'd have routing logic)
    let pool = pools_read.values().find(|p| {
        p.tokens.iter().any(|t| t.address == request.input_token) &&
        p.tokens.iter().any(|t| t.address == request.output_token)
    });
    
    if let Some(pool) = pool {
        let input_amount = request.input_amount.parse::<num_bigint::BigUint>()
            .map_err(|_| warp::reject::reject())?;
        
        match pool.calculate_swap_output(&request.input_token, &request.output_token, &input_amount) {
            Ok(output_amount) => {
                let response = SwapResponse {
                    output_amount: output_amount.to_string(),
                    price_impact: calculate_price_impact(&pool, &request.input_token, &input_amount),
                    fee: (input_amount.clone() * pool.fee_rate / 10000u64).to_string(),
                    route: vec![request.input_token, request.output_token],
                };
                Ok(warp::reply::json(&response))
            }
            Err(_) => Err(warp::reject::reject()),
        }
    } else {
        Err(warp::reject::reject())
    }
}

async fn handle_swap(
    request: SwapRequest,
    pools: PoolStorage,
) -> Result<impl warp::Reply, warp::Rejection> {
    // This would integrate with the smart contract layer
    // For now, we'll return a mock response
    handle_quote(request, pools).await
}

async fn handle_get_pools(pools: PoolStorage) -> Result<impl warp::Reply, warp::Rejection> {
    let pools_read = pools.read().await;
    let pool_infos: Vec<PoolInfo> = pools_read.values().map(|pool| {
        PoolInfo {
            id: pool.id.clone(),
            tokens: pool.tokens.clone(),
            reserves: pool.reserves.iter().map(|(k, v)| (k.clone(), v.to_string())).collect(),
            total_supply: pool.total_supply.to_string(),
            fee_rate: pool.fee_rate,
            apy: calculate_apy(&pool),
            volume_24h: "1000000".to_string(), // Mock data
        }
    }).collect();
    
    Ok(warp::reply::json(&pool_infos))
}

async fn handle_add_liquidity(
    request: AddLiquidityRequest,
    pools: PoolStorage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut pools_write = pools.write().await;
    
    if let Some(pool) = pools_write.get_mut(&request.pool_id) {
        let mut token_amounts = HashMap::new();
        
        for (token, amount_str) in request.token_amounts {
            let amount = amount_str.parse::<num_bigint::BigUint>()
                .map_err(|_| warp::reject::reject())?;
            token_amounts.insert(token, amount);
        }
        
        match pool.add_liquidity(token_amounts) {
            Ok(lp_tokens) => {
                let response = serde_json::json!({
                    "lp_tokens": lp_tokens.to_string(),
                    "success": true
                });
                Ok(warp::reply::json(&response))
            }
            Err(_) => Err(warp::reject::reject()),
        }
    } else {
        Err(warp::reject::reject())
    }
}

async fn initialize_sample_pools(pools: &PoolStorage) {
    let mut pools_write = pools.write().await;
    
    // ETH/USDC pool
    let eth_token = Token {
        address: "0x0000000000000000000000000000000000000000".to_string(),
        symbol: "ETH".to_string(),
        decimals: 18,
    };
    
    let usdc_token = Token {
        address: "0xA0b86a33E6441B8C5c4EA1E18AA41bE2d5E27ad2".to_string(),
        symbol: "USDC".to_string(),
        decimals: 6,
    };
    
    let mut reserves = HashMap::new();
    reserves.insert(eth_token.address.clone(), num_bigint::BigUint::from(1000000000000000000u64)); // 1 ETH
    reserves.insert(usdc_token.address.clone(), num_bigint::BigUint::from(2000000000u64)); // 2000 USDC
    
    let pool = Pool::new(
        "ETH-USDC".to_string(),
        vec![eth_token, usdc_token],
        reserves,
        300, // 3% fee
        PoolType::ConstantProduct,
    );
    
    pools_write.insert("ETH-USDC".to_string(), pool);
}

fn calculate_price_impact(pool: &Pool, input_token: &str, input_amount: &num_bigint::BigUint) -> f64 {
    // Simplified price impact calculation
    if let Some(input_reserve) = pool.reserves.get(input_token) {
        let impact = input_amount.clone() * 100u64 / input_reserve;
        impact.to_string().parse::<f64>().unwrap_or(0.0)
    } else {
        0.0
    }
}

fn calculate_apy(pool: &Pool) -> f64 {
    // Mock APY calculation - in reality this would use historical data
    match pool.pool_type {
        PoolType::ConstantProduct => 12.5,
        PoolType::StableSwap => 8.2,
        PoolType::ConcentratedLiquidity => 25.7,
    }
}