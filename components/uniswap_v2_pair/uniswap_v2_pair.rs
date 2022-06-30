#![cfg_attr(not(feature = "std"), no_std)]

#[openbrush::contract]
pub mod uniswap_v2_pair {
    use ink_prelude::string::String;
    use ink_prelude::vec::Vec;
    use ink_storage::traits::SpreadAllocate;

    use crate::math;
    use openbrush::contracts::psp22::extensions::{
        burnable::*, flashmint::*, metadata::*, mintable::*,
    };
    use openbrush::contracts::psp22::*;

    use swap_traits::uniswap_v2_callee::IUniswapV2Callee;

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
        InsufficientLiquidityMinted,
        InsufficientLiquidityBurned,
        InvalidTo,
        UniswapV2KError,
    }

    pub type Result<T> = core::result::Result<T, PairError>;

    #[openbrush::wrapper]
    type PSP22Ref = dyn PSP22 + PSP22Mintable + PSP22Burnable;

    #[ink(storage)]
    #[derive(Default, SpreadAllocate, PSP22Storage, PSP22MetadataStorage)]
    pub struct UniswapV2Pair {
        #[PSP22StorageField]
        psp22: PSP22Data,
        #[PSP22MetadataStorageField]
        metadata: PSP22MetadataData,
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

    impl PSP22 for UniswapV2Pair {}
    impl PSP22Metadata for UniswapV2Pair {}
    impl PSP22Mintable for UniswapV2Pair {}
    impl PSP22Burnable for UniswapV2Pair {}
    impl FlashLender for UniswapV2Pair {}

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
    pub struct Mint {
        #[ink(topic)]
        sender: AccountId, // address indexed sender
        amount_0: Balance,
        amount_1: Balance,
    }

    #[ink(event)]
    pub struct Burn {
        #[ink(topic)]
        sender: AccountId, // address indexed sender
        #[ink(topic)]
        to: AccountId, // address indexed to
        amount_0: Balance,
        amount_1: Balance,
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

    #[ink(event)]
    pub struct Sync {
        reserve_0: Balance,
        reserve_1: Balance,
    }

    impl UniswapV2Pair {
        #[ink(constructor)]
        pub fn new(
            name: Option<String>,
            symbol: Option<String>,
            decimal: u8,
            total_supply: Balance,
            token_0: AccountId,
            token_1: AccountId,
        ) -> Self {
            ink_lang::utils::initialize_contract(|instance: &mut Self| {
                instance.metadata.name = name;
                instance.metadata.symbol = symbol;
                instance.metadata.decimals = decimal;
                instance
                    ._mint(instance.env().caller(), total_supply)
                    .expect("Should mint total_supply");
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

        // todo: lock check?
        // function mint(address to) external lock returns (uint liquidity) {
        pub fn mint(&mut self, to: AccountId) -> Result<Balance> {
            let (reserve_0, reserve_1, _) = self.get_reserves()?;

            let balance_0 = PSP22Ref::balance_of(&self.token_0, Self::env().account_id());
            let balance_1 = PSP22Ref::balance_of(&self.token_1, Self::env().account_id());

            let amount_0 = balance_0.checked_sub(reserve_0).expect("underflow");
            let amount_1 = balance_1.checked_sub(reserve_1).expect("underflow");

            let is_fee_on = self.mint_fee(reserve_0, reserve_1);

            let total_supply = PSP22Ref::total_supply(&to);

            let liquidity: Balance;
            if total_supply == 0 {
                let _liquidity = math::sqrt(amount_0.checked_mul(amount_1).expect("overflow"));
                let _liquidity = _liquidity
                    .checked_sub(MINIMUM_LIQUIDITY)
                    .expect("underflow");

                liquidity = _liquidity;

                // todo: set up address(0)
                // _mint(address(0), MINIMUM_LIQUIDITY); // permanently lock the first MINIMUM_LIQUIDITY tokens
                PSP22Ref::mint(&Self::env().account_id(), to, MINIMUM_LIQUIDITY)
                    .expect("PSP22 mint error");
            } else {
                let _liquidity = math::min(
                    amount_0
                        .checked_mul(total_supply)
                        .expect("overflow")
                        .checked_div(reserve_0)
                        .expect("overflow"),
                    amount_1
                        .checked_mul(total_supply)
                        .expect("overflow")
                        .checked_div(reserve_1)
                        .expect("overflow"),
                );

                liquidity = _liquidity
            }

            if liquidity <= 0 {
                return Err(PairError::InsufficientLiquidityMinted);
            }

            PSP22Ref::mint(&Self::env().account_id(), to, liquidity).expect("PSP22 mint error");

            self.update(balance_0, balance_1, reserve_0, reserve_1)?;

            // todo
            // reserve_0 and reserve_1 are up-to-date??
            if is_fee_on {
                self.k_last = reserve_0.checked_mul(reserve_1).expect("overflow");
            }

            Self::env().emit_event(Mint {
                sender: Self::env().caller(),
                amount_0,
                amount_1,
            });

            Ok(liquidity)
        }

        // todo: lock check?
        // function burn(address to) external lock returns (uint amount0, uint amount1) {
        pub fn burn(&mut self, to: AccountId) -> Result<(Balance, Balance)> {
            let (reserve_0, reserve_1, _) = self.get_reserves()?;

            let balance_0 = PSP22Ref::balance_of(&self.token_0, Self::env().account_id());
            let balance_1 = PSP22Ref::balance_of(&self.token_1, Self::env().account_id());

            // get liquidity right?
            let liquidity =
                PSP22Ref::balance_of(&Self::env().account_id(), Self::env().account_id());

            let is_fee_on = self.mint_fee(reserve_0, reserve_1);
            let total_supply = PSP22Ref::total_supply(&to);

            let amount_0 = liquidity
                .checked_mul(balance_0)
                .expect("overflow")
                .checked_div(total_supply)
                .expect("overflow");
            let amount_1 = liquidity
                .checked_mul(balance_1)
                .expect("overflow")
                .checked_div(total_supply)
                .expect("overflow");

            if amount_0 <= 0 || amount_1 <= 0 {
                return Err(PairError::InsufficientLiquidityBurned);
            }

            PSP22Ref::burn(&Self::env().account_id(), to, liquidity).expect("PSP22 burn error");

            self.safe_transfer(self.token_0, to, amount_0);
            self.safe_transfer(self.token_1, to, amount_1);

            let balance_0 = PSP22Ref::balance_of(&self.token_0, Self::env().account_id());
            let balance_1 = PSP22Ref::balance_of(&self.token_1, Self::env().account_id());

            self.update(balance_0, balance_1, reserve_0, reserve_1)?;

            if is_fee_on {
                self.k_last = reserve_0.checked_mul(reserve_1).expect("overflow");
            }

            Self::env().emit_event(Burn {
                sender: Self::env().caller(),
                to,
                amount_0,
                amount_1,
            });

            Ok((amount_0, amount_1))
        }

        // todo: lock check?
        // function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external lock {
        pub fn swap(
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

            // todo: CalleeRef::uniswap_v2_call(&Self::env().account_id(), Self::env().caller(), amount_out_0, amount_out_1, data);
            // if (data.length > 0) IUniswapV2Callee(to).uniswapV2Call(msg.sender, amount0Out, amount1Out, data);

            let balance_0 = PSP22Ref::balance_of(&self.token_0, Self::env().account_id());
            let balance_1 = PSP22Ref::balance_of(&self.token_1, Self::env().account_id());

            let amount_in_0 = if balance_0 > reserve_0.checked_sub(amount_out_0).expect("underflow")
            {
                balance_0
                    .checked_sub(reserve_0.checked_sub(amount_out_0).expect("underflow"))
                    .expect("underflow")
            } else {
                0
            };
            let amount_in_1 = if balance_1 > reserve_1.checked_sub(amount_out_1).expect("underflow")
            {
                balance_1
                    .checked_sub(reserve_1.checked_sub(amount_out_1).expect("underflow"))
                    .expect("underflow")
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

        // todo:
        // function skim(address to) external lock {
        pub fn skim(&self, to: AccountId) -> Result<()> {
            let balance_0 = PSP22Ref::balance_of(&self.token_0, Self::env().account_id());
            let balance_1 = PSP22Ref::balance_of(&self.token_1, Self::env().account_id());

            let amount_0 = balance_0.checked_sub(self.reserve_0).expect("underflow");
            let amount_1 = balance_1.checked_sub(self.reserve_1).expect("underflow");

            self.safe_transfer(self.token_0, to, amount_0)?;
            self.safe_transfer(self.token_1, to, amount_1)?;

            Ok(())
        }

        // todo:
        // function sync() external lock {
        pub fn sync(&self) -> Result<()> {
            let balance_0 = PSP22Ref::balance_of(&self.token_0, Self::env().account_id());
            let balance_1 = PSP22Ref::balance_of(&self.token_1, Self::env().account_id());

            self.update(balance_0, balance_1, self.reserve_0, self.reserve_1)?;

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

        fn mint_fee(&self, reserve_0: Balance, reserve_1: Balance) -> bool {
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
