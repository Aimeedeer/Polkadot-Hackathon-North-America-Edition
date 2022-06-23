#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod uniswap_v2_pair {
    use ink_storage::traits::SpreadAllocate;
    // uniswap_v2_pair contract: https://github.com/Uniswap/v2-core/blob/master/contracts/UniswapV2Pair.sol

    // todo: check all types in ink!

    // todo: const SELECTOR;
    // bytes4 private constant SELECTOR = bytes4(keccak256(bytes('transfer(address,uint256)')));
    pub const MINIMUM_LIQUIDITY: Balance = 100_000_000; // uint 10**3

    // todo:
    // use openbrush's reentrancy_guard for uniswap's `uint private unlocked = 1;`
    // https://github.com/Supercolony-net/openbrush-contracts/blob/main/contracts/src/security/reentrancy_guard/mod.rs

    
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum PairError {
        InsufficientBalance,
    }

    pub type Result<T> = core::result::Result<T, PairError>;
    
    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct UniswapV2Pair {
        pub factory: AccountId,
        pub token_0: AccountId,
        pub token_1: AccountId,
        pub price_cumulative_last_0: u128, // uint (uint is an alias of uint256 in solidity)
        pub price_cumulative_last_1: u128,
        pub k_last: u128, // uint. reserve0 * reserve1, as of immediately after the most recent liquidity event
        reserve_0: u128,  // uint112 private reserve0;
        reserve_1: u128,
        block_timestamp_last: u64, //uinit32
    }

    #[ink(event)]
    pub struct Approval {
        #[ink(topic)]
        owner: AccountId,
        #[ink(topic)]
        spender: AccountId,
        value: Balance,
    }

    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        from: Option<AccountId>,
        #[ink(topic)]
        to: Option<AccountId>,
        value: Balance,
    }

    impl UniswapV2Pair {
        #[ink(constructor)]
        pub fn new(
            token_0: AccountId,
            token_1: AccountId,
        ) -> Self {
            ink_lang::utils::initialize_contract(|instance: &mut Self| {
                instance.factory = Self::env().caller();
                instance.token_0 = token_0;
                instance.token_1 = token_1;
                instance.price_cumulative_last_0 = 0;
                instance.price_cumulative_last_1 = 0;
                instance.k_last = 0;
                instance.reserve_0 = 0;
                instance.reserve_1 = 0;
                instance.block_timestamp_last = Self::env().block_timestamp();
            })
        }

        #[ink(constructor)]
        pub fn default() -> Self {
            ink_lang::utils::initialize_contract(|_| {})
        }

        #[ink(message)]
        pub fn get_reserves(&self) -> Result<(u128, u128, u64)> {
            Ok((self.reserve_0, self.reserve_1, self.block_timestamp_last))
        }

        fn update(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ink_lang as ink;

        #[ink::test]
        fn default_works() {
            todo!();
        }
    }
}
