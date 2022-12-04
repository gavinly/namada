//! Proof-of-Stake integration as a native validity predicate

pub mod vp;

pub use namada_proof_of_stake;
pub use namada_proof_of_stake::parameters::PosParams;
pub use namada_proof_of_stake::storage::*;
pub use namada_proof_of_stake::types;
use namada_proof_of_stake::PosBase;
use rust_decimal::Decimal;
pub use vp::PosVP;

use crate::ledger::storage::{self as ledger_storage, Storage, StorageHasher};
use crate::types::address::{Address, InternalAddress};
use crate::types::storage::Epoch;

/// Address of the PoS account implemented as a native VP
pub const ADDRESS: Address = Address::Internal(InternalAddress::PoS);

/// Address of the PoS slash pool account
pub const SLASH_POOL_ADDRESS: Address =
    Address::Internal(InternalAddress::PosSlashPool);

/// Calculate voting power in the tendermint context (which is stored as i64)
/// from the number of tokens
pub fn into_tm_voting_power(
    votes_per_token: Decimal,
    tokens: impl Into<u64>,
) -> i64 {
    let prod = decimal_mult_u64(votes_per_token, tokens.into());
    i64::try_from(prod).expect("Invalid validator voting power (i64)")
}

/// Initialize storage in the genesis block.
pub fn init_genesis_storage<'a, DB, H>(
    storage: &mut Storage<DB, H>,
    params: &'a PosParams,
    validators: impl Iterator<Item = &'a GenesisValidator> + Clone + 'a,
    current_epoch: Epoch,
) where
    DB: ledger_storage::DB + for<'iter> ledger_storage::DBIter<'iter>,
    H: StorageHasher,
{
    storage
        .init_genesis(params, validators, current_epoch)
        .expect("Initialize PoS genesis storage")
}


/// Alias for a PoS type with the same name with concrete type parameters
pub type ValidatorConsensusKeys =
    namada_proof_of_stake::types::ValidatorConsensusKeys<
        key::common::PublicKey,
    >;

/// Alias for a PoS type with the same name with concrete type parameters
pub type ValidatorTotalDeltas =
    namada_proof_of_stake::types::ValidatorTotalDeltas<token::Change>;

/// Alias for a PoS type with the same name with concrete type parameters
pub type Bonds = namada_proof_of_stake::types::Bonds<token::Amount>;

/// Alias for a PoS type with the same name with concrete type parameters
pub type Unbonds = namada_proof_of_stake::types::Unbonds<token::Amount>;

/// Alias for a PoS type with the same name with concrete type parameters
pub type ValidatorSets = namada_proof_of_stake::types::ValidatorSets<Address>;

/// Alias for a PoS type with the same name with concrete type parameters
pub type BondId = namada_proof_of_stake::types::BondId<Address>;

/// Alias for a PoS type with the same name with concrete type parameters
pub type GenesisValidator = namada_proof_of_stake::types::GenesisValidator<
    Address,
    token::Amount,
    key::common::PublicKey,
>;

// TODO pos will depend on core, so it will be able to `Epoch` type directly
// impl From<Epoch> for namada_proof_of_stake::types::Epoch {
//     fn from(epoch: Epoch) -> Self {
//         let epoch: u64 = epoch.into();
//         namada_proof_of_stake::types::Epoch::from(epoch)
//     }
// }

// impl From<namada_proof_of_stake::types::Epoch> for Epoch {
//     fn from(epoch: namada_proof_of_stake::types::Epoch) -> Self {
//         let epoch: u64 = epoch.into();
//         Epoch(epoch)
//     }
// }

// The error conversions are needed to implement `PosActions` in
// `tx_prelude/src/proof_of_stake.rs`
// impl From<namada_proof_of_stake::BecomeValidatorError<Address>>
//     for storage_api::Error
// {
//     fn from(err: namada_proof_of_stake::BecomeValidatorError<Address>) ->
// Self {         Self::new(err)
//     }
// }

// impl From<namada_proof_of_stake::BondError<Address>> for storage_api::Error {
//     fn from(err: namada_proof_of_stake::BondError<Address>) -> Self {
//         Self::new(err)
//     }
// }

// impl From<namada_proof_of_stake::UnbondError<Address, token::Amount>>
//     for storage_api::Error
// {
//     fn from(
//         err: namada_proof_of_stake::UnbondError<Address, token::Amount>,
//     ) -> Self {
//         Self::new(err)
//     }
// }

// impl From<namada_proof_of_stake::WithdrawError<Address>>
//     for storage_api::Error
// {
//     fn from(err: namada_proof_of_stake::WithdrawError<Address>) -> Self {
//         Self::new(err)
//     }
// }

#[macro_use]
mod macros {
    /// Implement `PosReadOnly` for a type that implements
    /// [`trait@crate::ledger::storage_api::StorageRead`].
    ///
    /// Excuse the horrible syntax - we haven't found a better way to use this
    /// for native_vp `CtxPreStorageRead`/`CtxPostStorageRead`, which have
    /// generics and explicit lifetimes.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// impl_pos_read_only! { impl PosReadOnly for X }
    /// ```
    #[macro_export]
    macro_rules! impl_pos_read_only {
    (
        // Type error type has to be declared before the impl.
        // This error type must `impl From<storage_api::Error> for $error`.
        type $error:tt = $err_ty:ty ;
        // Matches anything, so that we can use lifetimes and generic types.
        // This expects `impl(<.*>)? PoSReadOnly for $ty(<.*>)?`.
        $( $any:tt )* )
    => {
        $( $any )*
        {
            type Address = $crate::types::address::Address;
            type $error = $err_ty;
            type PublicKey = $crate::types::key::common::PublicKey;
            type TokenAmount = $crate::types::token::Amount;
            type TokenChange = $crate::types::token::Change;

            const POS_ADDRESS: Self::Address = $crate::ledger::pos::ADDRESS;

            fn staking_token_address() -> Self::Address {
                $crate::ledger::pos::staking_token_address()
            }

            fn read_pos_params(&self) -> std::result::Result<PosParams, Self::Error> {
                let value = $crate::ledger::storage_api::StorageRead::read_bytes(self, &params_key())?.unwrap();
                Ok($crate::ledger::storage::types::decode(value).unwrap())
            }

            fn read_validator_consensus_key(
                &self,
                key: &Self::Address,
            ) -> std::result::Result<Option<ValidatorConsensusKeys>, Self::Error> {
                let value =
                    $crate::ledger::storage_api::StorageRead::read_bytes(self, &validator_consensus_key_key(key))?;
                Ok(value.map(|value| $crate::ledger::storage::types::decode(value).unwrap()))
            }

            fn read_validator_state(
                &self,
                key: &Self::Address,
            ) -> std::result::Result<Option<ValidatorStates>, Self::Error> {
                let value = $crate::ledger::storage_api::StorageRead::read_bytes(self, &validator_state_key(key))?;
                Ok(value.map(|value| $crate::ledger::storage::types::decode(value).unwrap()))
            }

            fn read_validator_total_deltas(
                &self,
                key: &Self::Address,
            ) -> std::result::Result<Option<ValidatorTotalDeltas>, Self::Error> {
                let value =
                    $crate::ledger::storage_api::StorageRead::read_bytes(self, &validator_total_deltas_key(key))?;
                Ok(value.map(|value| $crate::ledger::storage::types::decode(value).unwrap()))
            }

            fn read_validator_voting_power(
                &self,
                key: &Self::Address,
            ) -> std::result::Result<Option<ValidatorVotingPowers>, Self::Error> {
                let value =
                    $crate::ledger::storage_api::StorageRead::read_bytes(self, &validator_voting_power_key(key))?;
                Ok(value.map(|value| $crate::ledger::storage::types::decode(value).unwrap()))
            }

            fn read_validator_slashes(
                &self,
                key: &Self::Address,
            ) -> std::result::Result<Vec<types::Slash>, Self::Error> {
                let value = $crate::ledger::storage_api::StorageRead::read_bytes(self, &validator_slashes_key(key))?;
                Ok(value
                    .map(|value| $crate::ledger::storage::types::decode(value).unwrap())
                    .unwrap_or_default())
            }

            fn read_bond(
                &self,
                key: &BondId,
            ) -> std::result::Result<Option<Bonds>, Self::Error> {
                let value = $crate::ledger::storage_api::StorageRead::read_bytes(self, &bond_key(key))?;
                Ok(value.map(|value| $crate::ledger::storage::types::decode(value).unwrap()))
            }

            fn read_unbond(
                &self,
                key: &BondId,
            ) -> std::result::Result<Option<Unbonds>, Self::Error> {
                let value = $crate::ledger::storage_api::StorageRead::read_bytes(self, &unbond_key(key))?;
                Ok(value.map(|value| $crate::ledger::storage::types::decode(value).unwrap()))
            }

            fn read_validator_set(
                &self,
            ) -> std::result::Result<ValidatorSets, Self::Error> {
                let value =
                    $crate::ledger::storage_api::StorageRead::read_bytes(self, &validator_set_key())?.unwrap();
                Ok($crate::ledger::storage::types::decode(value).unwrap())
            }

            fn read_total_voting_power(
                &self,
            ) -> std::result::Result<TotalVotingPowers, Self::Error> {
                let value =
                    $crate::ledger::storage_api::StorageRead::read_bytes(self, &total_voting_power_key())?.unwrap();
                Ok($crate::ledger::storage::types::decode(value).unwrap())
            }
        }
    }
}
}

impl_pos_read_only! {
    type Error = storage_api::Error;
    impl<DB, H> PosReadOnly for Storage<DB, H>
        where
            DB: ledger_storage::DB + for<'iter> ledger_storage::DBIter<'iter> +'static,
            H: StorageHasher +'static,
}
