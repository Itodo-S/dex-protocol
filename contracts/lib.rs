use ethers::prelude::*;
use std::sync::Arc;

// Contract ABI definitions
abigen!(
    DEXRouter,
    r#"[
        function swapExactTokensForTokens(uint amountIn, uint amountOutMin, address[] calldata path, address to, uint deadline) external returns (uint[] memory amounts)
        function addLiquidity(address tokenA, address tokenB, uint amountADesired, uint amountBDesired, uint amountAMin, uint amountBMin, address to, uint deadline) external returns (uint amountA, uint amountB, uint liquidity)
        function removeLiquidity(address tokenA, address tokenB, uint liquidity, uint amountAMin, uint amountBMin, address to, uint deadline) external returns (uint amountA, uint amountB)
        function getAmountsOut(uint amountIn, address[] calldata path) external view returns (uint[] memory amounts)
    ]"#
);

abigen!(
    DEXFactory,
    r#"[
        function createPair(address tokenA, address tokenB) external returns (address pair)
        function getPair(address tokenA, address tokenB) external view returns (address pair)
        function allPairs(uint) external view returns (address pair)
        function allPairsLength() external view returns (uint)
    ]"#
);

pub struct DEXProtocol {
    pub router: DEXRouter<Provider<Http>>,
    pub factory: DEXFactory<Provider<Http>>,
    pub provider: Arc<Provider<Http>>,
}

impl DEXProtocol {
    pub async fn new(
        provider_url: &str,
        router_address: Address,
        factory_address: Address,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let provider = Provider::<Http>::try_from(provider_url)?;
        let provider = Arc::new(provider);
        
        let router = DEXRouter::new(router_address, provider.clone());
        let factory = DEXFactory::new(factory_address, provider.clone());
        
        Ok(Self {
            router,
            factory,
            provider,
        })
    }

    pub async fn swap_tokens(
        &self,
        wallet: &LocalWallet,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        amount_out_min: U256,
        deadline: U256,
    ) -> Result<TransactionReceipt, Box<dyn std::error::Error>> {
        let client = SignerMiddleware::new(self.provider.clone(), wallet.clone());
        let router = DEXRouter::new(self.router.address(), Arc::new(client));
        
        let path = vec![token_in, token_out];
        let to = wallet.address();
        
        let tx = router
            .swap_exact_tokens_for_tokens(
                amount_in,
                amount_out_min,
                path,
                to,
                deadline,
            )
            .send()
            .await?;
            
        let receipt = tx.await?;
        Ok(receipt.unwrap())
    }

    pub async fn get_amounts_out(
        &self,
        amount_in: U256,
        path: Vec<Address>,
    ) -> Result<Vec<U256>, Box<dyn std::error::Error>> {
        let amounts = self.router.get_amounts_out(amount_in, path).call().await?;
        Ok(amounts)
    }

    pub async fn create_pair(
        &self,
        wallet: &LocalWallet,
        token_a: Address,
        token_b: Address,
    ) -> Result<Address, Box<dyn std::error::Error>> {
        let client = SignerMiddleware::new(self.provider.clone(), wallet.clone());
        let factory = DEXFactory::new(self.factory.address(), Arc::new(client));
        
        let tx = factory.create_pair(token_a, token_b).send().await?;
        let receipt = tx.await?.unwrap();
        
        // Extract pair address from logs
        let pair_address = self.factory.get_pair(token_a, token_b).call().await?;
        Ok(pair_address)
    }
}