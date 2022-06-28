#![cfg_attr(not(feature = "std"), no_std)]

#[openbrush::contract]
pub mod uniswap_v2_psp22 {
    use ink_prelude::string::String;
    use ink_prelude::vec::Vec;
    use ink_storage::traits::SpreadAllocate;

    use openbrush::contracts::psp22::extensions::burnable::*;
    use openbrush::contracts::psp22::extensions::metadata::*;
    use openbrush::contracts::psp22::extensions::mintable::*;
    use openbrush::contracts::psp22::*;

    #[ink(storage)]
    #[derive(Default, SpreadAllocate, PSP22Storage, PSP22MetadataStorage)]
    pub struct UniswapV2Psp22 {
        #[PSP22StorageField]
        psp22: PSP22Data,
        #[PSP22MetadataStorageField]
        metadata: PSP22MetadataData,
    }

    impl PSP22 for UniswapV2Psp22 {}
    impl PSP22Metadata for UniswapV2Psp22 {}
    impl PSP22Mintable for UniswapV2Psp22 {}
    impl PSP22Burnable for UniswapV2Psp22 {}

    impl UniswapV2Psp22 {
        #[ink(constructor)]
        pub fn new(
            total_supply: Balance,
            name: Option<String>,
            symbol: Option<String>,
            decimal: u8,
        ) -> Self {
            ink_lang::codegen::initialize_contract(|instance: &mut Self| {
                instance.metadata.name = name;
                instance.metadata.symbol = symbol;
                instance.metadata.decimals = decimal;
                instance
                    ._mint(instance.env().caller(), total_supply)
                    .expect("Should mint total_supply");
            })
        }

        #[ink(message)]
        pub fn mint_to(&mut self, account: AccountId, amount: Balance) -> Result<(), PSP22Error> {
            self.mint(account, amount)
        }

        #[ink(message)]
        pub fn burn_from_many(
            &mut self,
            accounts: Vec<(AccountId, Balance)>,
        ) -> Result<(), PSP22Error> {
            for account in accounts.iter() {
                self.burn(account.0, account.1)?;
            }
            Ok(())
        }
    }

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.
    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        /// Imports `ink_lang` so we can use `#[ink::test]`.
        use ink_lang as ink;

        /// We test if the default constructor does its job.
        #[ink::test]
        fn default_works() {
            let uniswap_v2_psp22 = UniswapV2Psp22::default();
            assert_eq!(uniswap_v2_psp22.get(), false);
        }

        /// We test a simple use case of our contract.
        #[ink::test]
        fn it_works() {
            todo!();
        }
    }
}
