#![cfg_attr(not(feature = "std"), no_std)]

#[openbrush::contract]
pub mod uniswap_v2_pair {
    use ink_prelude::vec::Vec;
    use ink_storage::traits::SpreadAllocate;

    use crate::uniswap_v2_psp22::uniswap_v2_psp22::UniswapV2Psp22;
    use openbrush::contracts::psp22::psp22_external;
    use openbrush::contracts::psp22::PSP22;

    // todo: const SELECTOR;
    // #[ink(selector = 0xCAFEBABE)]
    // bytes4 private constant SELECTOR = bytes4(keccak256(bytes('transfer(address,uint256)')));

    pub const MINIMUM_LIQUIDITY: Balance = 1_000;

    // todo:
    // use openbrush's reentrancy_guard / modifiers for uniswap's `uint private unlocked = 1;`
    // https://github.com/Supercolony-net/openbrush-contracts/blob/main/contracts/src/security/reentrancy_guard/mod.rs

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum PairError {
        InsufficientInputAmount,
        InsufficientOoutputAmount,
        InsufficientLiquidity,
        InvalidTo,
        UniswapV2KError,
    }

    pub type Result<T> = core::result::Result<T, PairError>;

    #[openbrush::wrapper]
    type PSP22Ref = dyn PSP22;

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct UniswapV2Pair {
        pub factory: AccountId,
        pub token_0: AccountId,
        pub token_1: AccountId,
        pub price_cumulative_last_0: u128, // uint (uint is an alias of uint256 in solidity)
        pub price_cumulative_last_1: u128,
        pub k_last: u128, // uint. reserve0 * reserve1, as of immediately after the most recent liquidity event
        reserve_0: Balance,
        reserve_1: Balance,
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

    #[ink(event)]
    pub struct Swap {
        #[ink(topic)]
        sender: AccountId, // address indexed sender
        #[ink(topic)]
        to: AccountId, // address indexed to
        amount_in_0: Balance,
        amount_in_1: Balance,
        amount_out_0: Balance,
        amount_out_1: Balance,
    }

    impl UniswapV2Pair {
        #[ink(constructor)]
        pub fn new(token_0: AccountId, token_1: AccountId) -> Self {
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
        pub fn get_reserves(&self) -> Result<(Balance, Balance, u64)> {
            Ok((self.reserve_0, self.reserve_1, self.block_timestamp_last))
        }

        fn swap(
            &mut self,
            amount_out_0: Balance,
            amount_out_1: Balance,
            to: AccountId,
            data: Vec<u8>,
        ) -> Result<()> {
            if amount_out_0 <= 0 && amount_out_1 <= 0 {
                return Err(PairError::InsufficientOoutputAmount);
            }

            let (reserve_0, reserve_1, _) = self.get_reserves()?;
            if amount_out_0 >= reserve_0 || amount_out_1 >= reserve_1 {
                return Err(PairError::InsufficientLiquidity);
            }

            if to == self.token_0 || to == self.token_1 {
                return Err(PairError::InvalidTo);
            }

            if amount_out_0 > 0 {
                self.safe_transfer(self.token_0, to, amount_out_0)?;
            }

            if amount_out_1 > 0 {
                self.safe_transfer(self.token_1, to, amount_out_1)?;
            }

            // todo
            // if (data.length > 0) IUniswapV2Callee(to).uniswapV2Call(msg.sender, amount0Out, amount1Out, data);

            let balance_0 = PSP22Ref::balance_of(&self.token_0, Self::env().account_id());
            let balance_1 = PSP22Ref::balance_of(&self.token_1, Self::env().account_id());

            let amount_in_0 = if balance_0 > reserve_0 - amount_out_0 {
                balance_0 - (reserve_0 - amount_out_0)
            } else {
                0
            };
            let amount_in_1 = if balance_1 > reserve_1 - amount_out_1 {
                balance_1 - (reserve_1 - amount_out_1)
            } else {
                0
            };

            if amount_in_0 <= 0 && amount_in_1 <= 0 {
                return Err(PairError::InsufficientInputAmount);
            }

            let adjusted_balance_0 = balance_0.checked_mul(1000).expect("overflow");
            let adjusted_balance_0 = adjusted_balance_0
                .checked_sub(amount_in_0.checked_mul(3).expect("overflow"))
                .expect("underflow");

            let adjusted_balance_1 = balance_1.checked_mul(1000).expect("overflow");
            let adjusted_balance_1 = adjusted_balance_1
                .checked_sub(amount_in_1.checked_mul(3).expect("overflow"))
                .expect("underflow");

            let k_balance = adjusted_balance_0
                .checked_mul(adjusted_balance_1)
                .expect("overflow");
            let k_reserve = reserve_0.checked_mul(reserve_1).expect("overflow");
            let k_reserve = k_reserve.checked_mul(1_000_000).expect("overflow");

            if k_balance < k_reserve {
                return Err(PairError::UniswapV2KError);
            } else {
                self.update(balance_0, balance_1, reserve_0, reserve_1)?;
            }

            Self::env().emit_event(Swap {
                sender: Self::env().caller(),
                to,
                amount_in_0,
                amount_in_1,
                amount_out_0,
                amount_out_1,
            });

            Ok(())
        }

        fn safe_transfer(&self, token: AccountId, to: AccountId, value: Balance) -> Result<()> {
            todo!()
        }

        fn update(
            &self,
            balance_0: Balance,
            balance_1: Balance,
            reserve_0: Balance,
            reserve_1: Balance,
        ) -> Result<()> {
            todo!()
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
