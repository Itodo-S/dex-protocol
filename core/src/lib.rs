use num_bigint::BigUint;
use num_traits::{One, Zero};
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
    ConstantProduct,       // x * y = k
    StableSwap,            // For stablecoins
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
        let input_reserve = self
            .reserves
            .get(input_token)
            .ok_or(SwapError::TokenNotFound)?;
        let output_reserve = self
            .reserves
            .get(output_token)
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
            let current_reserve = self
                .reserves
                .get_mut(&token)
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
            let current_reserve = self
                .reserves
                .get(token)
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

impl Pool {
    pub fn update_dynamic_fee(&mut self, volume_24h: &BigUint, volatility: f64) {
        // Dynamic fee based on volume and volatility
        let base_fee = 300u64; // 3% base fee
        let volume_factor = if *volume_24h > BigUint::from(1000000u64) {
            50
        } else {
            0
        };
        let volatility_factor = (volatility * 100.0) as u64;

        self.fee_rate = base_fee + volume_factor + volatility_factor;
        self.fee_rate = self.fee_rate.min(1000); // Cap at 10%
    }

    pub fn calculate_concentrated_liquidity_swap(
        &self,
        input_token: &str,
        output_token: &str,
        input_amount: &BigUint,
        price_range: (f64, f64),
    ) -> Result<BigUint, SwapError> {
        // Uniswap V3 style concentrated liquidity logic
        // This is a simplified version - real implementation would be more complex

        let current_price = self.get_current_price(input_token, output_token)?;

        if current_price < price_range.0 || current_price > price_range.1 {
            return Err(SwapError::PriceOutOfRange);
        }

        // Calculate output based on concentrated liquidity curve
        let liquidity_in_range = self.calculate_active_liquidity(price_range)?;
        let output_amount =
            self.calculate_output_from_liquidity(input_amount, &liquidity_in_range, current_price)?;

        Ok(output_amount)
    }

    fn get_current_price(&self, token_a: &str, token_b: &str) -> Result<f64, SwapError> {
        let reserve_a = self.reserves.get(token_a).ok_or(SwapError::TokenNotFound)?;
        let reserve_b = self.reserves.get(token_b).ok_or(SwapError::TokenNotFound)?;

        if reserve_a.is_zero() || reserve_b.is_zero() {
            return Err(SwapError::InsufficientLiquidity);
        }

        let price = reserve_b.clone() as f64 / reserve_a.clone() as f64;
        Ok(price)
    }

    fn calculate_active_liquidity(&self, price_range: (f64, f64)) -> Result<BigUint, SwapError> {
        // Simplified - in reality this would track liquidity positions
        let total_liquidity = &self.total_supply;
        let range_factor = 1.0 / (price_range.1 - price_range.0);
        let active_liquidity = total_liquidity * BigUint::from(range_factor as u64);

        Ok(active_liquidity)
    }

    fn calculate_output_from_liquidity(
        &self,
        input_amount: &BigUint,
        liquidity: &BigUint,
        current_price: f64,
    ) -> Result<BigUint, SwapError> {
        // Simplified concentrated liquidity calculation
        let price_impact = input_amount.clone() / liquidity;
        let new_price =
            current_price * (1.0 + price_impact.to_string().parse::<f64>().unwrap_or(0.0));
        let output_amount = input_amount * BigUint::from(new_price as u64);

        Ok(output_amount)
    }
}

// Add new error type
#[derive(Debug, thiserror::Error)]
pub enum SwapError {
    #[error("Token not found in pool")]
    TokenNotFound,
    #[error("Insufficient liquidity")]
    InsufficientLiquidity,
    #[error("Unsupported pool type")]
    UnsupportedPoolType,
    #[error("Price out of range")]
    PriceOutOfRange,
}

// Extended Pool implementation for multi-asset pools
impl Pool {
    pub fn calculate_multi_asset_swap(
        &self,
        input_token: &str,
        output_token: &str,
        input_amount: &BigUint,
    ) -> Result<BigUint, SwapError> {
        match self.pool_type {
            PoolType::ConstantProduct => {
                // Standard 2-token AMM
                self.constant_product_swap(input_token, output_token, input_amount)
            }
            PoolType::StableSwap => {
                // Curve-style stable swap for correlated assets
                self.stable_swap(input_token, output_token, input_amount)
            }
            PoolType::ConcentratedLiquidity => {
                // Uniswap V3 style with price ranges
                self.concentrated_liquidity_swap(input_token, output_token, input_amount)
            }
        }
    }

    fn stable_swap(
        &self,
        input_token: &str,
        output_token: &str,
        input_amount: &BigUint,
    ) -> Result<BigUint, SwapError> {
        // Curve StableSwap invariant: A * n^n * sum(x_i) + D = A * D * n^n + D^(n+1) / (n^n * prod(x_i))
        let n = self.tokens.len();
        let a = BigUint::from(100u64); // Amplification parameter

        let mut balances: Vec<BigUint> = Vec::new();
        let mut total_balance = BigUint::zero();

        for token in &self.tokens {
            let balance = self
                .reserves
                .get(&token.address)
                .unwrap_or(&BigUint::zero())
                .clone();
            balances.push(balance.clone());
            total_balance += balance;
        }

        let d = self.calculate_d(&balances, &a)?;

        // Find input and output token indices
        let input_idx = self.find_token_index(input_token)?;
        let output_idx = self.find_token_index(output_token)?;

        // Calculate new balance after input
        let mut new_balances = balances.clone();
        new_balances[input_idx] += input_amount;

        // Calculate what the output balance should be
        let new_output_balance = self.calculate_y(&new_balances, output_idx, &d, &a)?;
        let output_amount = &balances[output_idx] - &new_output_balance;

        // Apply fee
        let fee_amount = (&output_amount * self.fee_rate) / BigUint::from(10000u64);
        let output_after_fee = output_amount - fee_amount;

        Ok(output_after_fee)
    }

    fn calculate_d(&self, balances: &[BigUint], a: &BigUint) -> Result<BigUint, SwapError> {
        let n = BigUint::from(balances.len());
        let mut s = BigUint::zero();

        for balance in balances {
            s += balance;
        }

        if s.is_zero() {
            return Ok(BigUint::zero());
        }

        let mut d = s.clone();
        let ann = a * &n.pow(balances.len() as u32);

        // Newton's method to solve for D
        for _ in 0..255 {
            let mut dp = d.clone();
            for balance in balances {
                dp = (&dp * &d) / (&n * balance);
            }

            let d_prev = d.clone();
            d = ((&ann * &s + &dp * &n) * &d)
                / ((&ann - BigUint::one()) * &d + (&n + BigUint::one()) * &dp);

            if d > d_prev {
                if &d - &d_prev <= BigUint::one() {
                    break;
                }
            } else if &d_prev - &d <= BigUint::one() {
                break;
            }
        }

        Ok(d)
    }

    fn calculate_y(
        &self,
        balances: &[BigUint],
        token_index: usize,
        d: &BigUint,
        a: &BigUint,
    ) -> Result<BigUint, SwapError> {
        let n = BigUint::from(balances.len());
        let ann = a * &n.pow(balances.len() as u32);

        let mut c = d.clone();
        let mut s = BigUint::zero();

        for (i, balance) in balances.iter().enumerate() {
            if i != token_index {
                s += balance;
                c = (&c * d) / (&n * balance);
            }
        }

        c = (&c * d) / (&ann * &n);
        let b = &s + d / &ann;

        let mut y = d.clone();
        for _ in 0..255 {
            let y_prev = y.clone();
            y = (&y * &y + &c) / (&y * BigUint::from(2u32) + &b - d);

            if y > y_prev {
                if &y - &y_prev <= BigUint::one() {
                    break;
                }
            } else if &y_prev - &y <= BigUint::one() {
                break;
            }
        }

        Ok(y)
    }

    fn find_token_index(&self, token_address: &str) -> Result<usize, SwapError> {
        self.tokens
            .iter()
            .position(|t| t.address == token_address)
            .ok_or(SwapError::TokenNotFound)
    }

    fn concentrated_liquidity_swap(
        &self,
        input_token: &str,
        output_token: &str,
        input_amount: &BigUint,
    ) -> Result<BigUint, SwapError> {
        // Simplified Uniswap V3 style calculation
        // In reality, this would involve complex tick calculations

        let current_price = self.get_current_price(input_token, output_token)?;
        let sqrt_price = (current_price.sqrt() * 2f64.powi(96)) as u128;

        // Calculate price impact based on concentrated liquidity
        let liquidity = &self.total_supply;
        let price_impact = input_amount / liquidity;

        // Calculate new price after swap
        let new_sqrt_price = sqrt_price + price_impact.to_string().parse::<u128>().unwrap_or(0);
        let new_price = (new_sqrt_price as f64 / 2f64.powi(96)).powi(2);

        // Calculate output amount
        let price_ratio = new_price / current_price;
        let output_amount = input_amount * BigUint::from((1.0 / price_ratio) as u64);

        // Apply fee
        let fee_amount = (&output_amount * self.fee_rate) / BigUint::from(10000u64);
        let output_after_fee = output_amount - fee_amount;

        Ok(output_after_fee)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use std::collections::HashMap;

    #[test]
    fn test_constant_product_swap() {
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

        let mut reserves = HashMap::new();
        reserves.insert("ETH".to_string(), BigUint::from(1000u64));
        reserves.insert("USDC".to_string(), BigUint::from(2000u64));

        let pool = Pool::new(
            "ETH-USDC".to_string(),
            vec![eth_token, usdc_token],
            reserves,
            300, // 3% fee
            PoolType::ConstantProduct,
        );

        let input_amount = BigUint::from(100u64);
        let output = pool.calculate_swap_output("ETH", "USDC", &input_amount);

        assert!(output.is_ok());
        let output_amount = output.unwrap();
        assert!(output_amount > BigUint::zero());
        assert!(output_amount < BigUint::from(2000u64)); // Should be less than total reserve
    }

    #[test]
    fn test_add_liquidity() {
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

        let mut reserves = HashMap::new();
        reserves.insert("ETH".to_string(), BigUint::from(1000u64));
        reserves.insert("USDC".to_string(), BigUint::from(2000u64));

        let mut pool = Pool::new(
            "ETH-USDC".to_string(),
            vec![eth_token, usdc_token],
            reserves,
            300,
            PoolType::ConstantProduct,
        );

        let mut liquidity_amounts = HashMap::new();
        liquidity_amounts.insert("ETH".to_string(), BigUint::from(100u64));
        liquidity_amounts.insert("USDC".to_string(), BigUint::from(200u64));

        let initial_supply = pool.total_supply.clone();
        let lp_tokens = pool.add_liquidity(liquidity_amounts).unwrap();

        assert!(lp_tokens > BigUint::zero());
        assert!(pool.total_supply > initial_supply);
    }

    #[test]
    fn test_dynamic_fee_calculation() {
        let mut pool = create_sample_pool();

        let high_volume = BigUint::from(10000000u64);
        let high_volatility = 0.5;

        pool.update_dynamic_fee(&high_volume, high_volatility);

        assert!(pool.fee_rate > 300); // Should be higher than base fee
        assert!(pool.fee_rate <= 1000); // Should be capped at 10%
    }

    #[test]
    fn test_price_calculation() {
        let pool = create_sample_pool();
        let price = pool.get_current_price("ETH", "USDC").unwrap();

        assert!(price > 0.0);
        assert_eq!(price, 2.0); // 2000 USDC / 1000 ETH = 2.0
    }

    fn create_sample_pool() -> Pool {
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

        let mut reserves = HashMap::new();
        reserves.insert("ETH".to_string(), BigUint::from(1000u64));
        reserves.insert("USDC".to_string(), BigUint::from(2000u64));

        Pool::new(
            "ETH-USDC".to_string(),
            vec![eth_token, usdc_token],
            reserves,
            300,
            PoolType::ConstantProduct,
        )
    }
}
