use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub total_swaps: u64,
    pub total_volume: HashMap<String, String>, // token -> volume
    pub total_fees_collected: HashMap<String, String>,
    pub active_pools: u64,
    pub total_liquidity: HashMap<String, String>,
    pub average_transaction_time: f64,
}

pub struct MetricsCollector {
    metrics: Arc<RwLock<Metrics>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Metrics {
                total_swaps: 0,
                total_volume: HashMap::new(),
                total_fees_collected: HashMap::new(),
                active_pools: 0,
                total_liquidity: HashMap::new(),
                average_transaction_time: 0.0,
            })),
        }
    }
    
    pub async fn record_swap(&self, input_token: &str, output_token: &str, volume: &str, fee: &str) {
        let mut metrics = self.metrics.write().await;
        
        metrics.total_swaps += 1;
        
        // Update volume
        let current_volume = metrics.total_volume.get(input_token)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let new_volume = current_volume + volume.parse::<u64>().unwrap_or(0);
        metrics.total_volume.insert(input_token.to_string(), new_volume.to_string());
        
        // Update fees
        let current_fees = metrics.total_fees_collected.get(input_token)
            .and_then(|f| f.parse::<u64>().ok())
            .unwrap_or(0);
        let new_fees = current_fees + fee.parse::<u64>().unwrap_or(0);
        metrics.total_fees_collected.insert(input_token.to_string(), new_fees.to_string());
    }
    
    pub async fn get_metrics(&self) -> Metrics {
        self.metrics.read().await.clone()
    }
}