use num_bigint::BigUint;
use num_traits::{Zero, One};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pool {
    pub id: String,
    pub tokens: Vec<Token>,
    pub reserves: HashMap<String, BigUint>,
    pub total_supply: BigUint,
    pub fee_rate: u64, // basis points (100 = 1%)
    pub pool_type: PoolType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoolType {
    ConstantProduct, // x * y = k
    StableSwap,      // For stablecoins
    ConcentratedLiquidity, // Uniswap V3 style
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub address: String,
    pub symbol: String,
    pub decimals: u8,
}

impl Pool {
    pub fn new(
        id: String,
        tokens: Vec<Token>,
        initial_reserves: HashMap<String, BigUint>,
        fee_rate: u64,
        pool_type: PoolType,
    ) -> Self {
        let total_supply = match pool_type {
            PoolType::ConstantProduct => {
                // Calculate initial LP tokens using geometric mean
                let mut product = BigUint::one();
                for reserve in initial_reserves.values() {
                    product *= reserve;
                }
                // Simplified: use square root for 2-token pools
                sqrt(&product)
            }
            _ => BigUint::zero(), // Implement for other pool types
        };

        Pool {
            id,
            tokens,
            reserves: initial_reserves,
            total_supply,
            fee_rate,
            pool_type,
        }
    }

    pub fn calculate_swap_output(
        &self,
        input_token: &str,
        output_token: &str,
        input_amount: &BigUint,
    ) -> Result<BigUint, SwapError> {
        match self.pool_type {
            PoolType::ConstantProduct => {
                self.constant_product_swap(input_token, output_token, input_amount)
            }
            _ => Err(SwapError::UnsupportedPoolType),
        }
    }

    fn constant_product_swap(
        &self,
        input_token: &str,
        output_token: &str,
        input_amount: &BigUint,
    ) -> Result<BigUint, SwapError> {
        let input_reserve = self.reserves.get(input_token)
            .ok_or(SwapError::TokenNotFound)?;
        let output_reserve = self.reserves.get(output_token)
            .ok_or(SwapError::TokenNotFound)?;

        if input_reserve.is_zero() || output_reserve.is_zero() {
            return Err(SwapError::InsufficientLiquidity);
        }

        // Apply fee: input_amount_with_fee = input_amount * (10000 - fee_rate) / 10000
        let fee_multiplier = BigUint::from(10000u64 - self.fee_rate);
        let input_amount_with_fee = (input_amount * &fee_multiplier) / BigUint::from(10000u64);

        // Calculate output: output = (input_with_fee * output_reserve) / (input_reserve + input_with_fee)
        let numerator = &input_amount_with_fee * output_reserve;
        let denominator = input_reserve + &input_amount_with_fee;

        if denominator.is_zero() {
            return Err(SwapError::InsufficientLiquidity);
        }

        let output_amount = numerator / denominator;
        
        if output_amount >= *output_reserve {
            return Err(SwapError::InsufficientLiquidity);
        }

        Ok(output_amount)
    }

    pub fn add_liquidity(
        &mut self,
        token_amounts: HashMap<String, BigUint>,
    ) -> Result<BigUint, LiquidityError> {
        // Calculate LP tokens to mint
        let lp_tokens = self.calculate_lp_tokens_to_mint(&token_amounts)?;
        
        // Update reserves
        for (token, amount) in token_amounts {
            let current_reserve = self.reserves.get_mut(&token)
                .ok_or(LiquidityError::TokenNotFound)?;
            *current_reserve += amount;
        }
        
        // Update total supply
        self.total_supply += &lp_tokens;
        
        Ok(lp_tokens)
    }

    fn calculate_lp_tokens_to_mint(
        &self,
        token_amounts: &HashMap<String, BigUint>,
    ) -> Result<BigUint, LiquidityError> {
        if self.total_supply.is_zero() {
            // Initial liquidity
            let mut product = BigUint::one();
            for amount in token_amounts.values() {
                product *= amount;
            }
            return Ok(sqrt(&product));
        }

        // Calculate based on proportion
        let mut min_ratio = None;
        
        for (token, amount) in token_amounts {
            let current_reserve = self.reserves.get(token)
                .ok_or(LiquidityError::TokenNotFound)?;
            
            if current_reserve.is_zero() {
                return Err(LiquidityError::InsufficientLiquidity);
            }
            
            let ratio = (amount * &self.total_supply) / current_reserve;
            
            min_ratio = match min_ratio {
                None => Some(ratio),
                Some(current_min) => Some(ratio.min(current_min)),
            };
        }
        
        min_ratio.ok_or(LiquidityError::InsufficientLiquidity)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SwapError {
    #[error("Token not found in pool")]
    TokenNotFound,
    #[error("Insufficient liquidity")]
    InsufficientLiquidity,
    #[error("Unsupported pool type")]
    UnsupportedPoolType,
}

#[derive(Debug, thiserror::Error)]
pub enum LiquidityError {
    #[error("Token not found in pool")]
    TokenNotFound,
    #[error("Insufficient liquidity")]
    InsufficientLiquidity,
}

// Helper function for square root calculation
fn sqrt(n: &BigUint) -> BigUint {
    if n.is_zero() {
        return BigUint::zero();
    }
    
    let mut x = n.clone();
    let mut y = (n + BigUint::one()) / BigUint::from(2u32);
    
    while y < x {
        x = y.clone();
        y = (&x + n / &x) / BigUint::from(2u32);
    }
    
    x
}