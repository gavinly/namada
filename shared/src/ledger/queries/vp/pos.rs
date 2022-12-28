use std::collections::HashSet;

use namada_proof_of_stake::{self, PosReadOnly};

use crate::ledger::pos::{self, BondId};
use crate::ledger::queries::types::RequestCtx;
use crate::ledger::storage::{DBIter, StorageHasher, DB};
use crate::ledger::storage_api;
use crate::types::address::Address;
use crate::types::storage::Epoch;
use crate::types::token;

// PoS validity predicate queries
router! {POS,
    ( "validator" ) = {
        ( "is_validator" / [addr: Address] ) -> bool = is_validator,

        ( "addresses" / [epoch: opt Epoch] )
        -> HashSet<Address> = validator_addresses,

        ( "stake" / [validator: Address] / [epoch: opt Epoch] )
        -> token::Amount = validator_stake,
    },

    ( "total_stake" / [epoch: opt Epoch] )
    -> token::Amount = total_stake,

    ( "delegations" / [owner: Address] )
    -> HashSet<Address> = delegations,

    ( "bond_amount" / [owner: Address] / [validator: Address] / [epoch: opt Epoch] )
    -> token::Amount = bond_amount,
}

// Handlers that implement the functions via `trait StorageRead`:

/// Find if the given address belongs to a validator account.
fn is_validator<D, H>(
    ctx: RequestCtx<'_, D, H>,
    addr: Address,
) -> storage_api::Result<bool>
where
    D: 'static + DB + for<'iter> DBIter<'iter> + Sync,
    H: 'static + StorageHasher + Sync,
{
    let params = namada_proof_of_stake::read_pos_params(ctx.storage)?;
    namada_proof_of_stake::is_validator(
        ctx.storage,
        &addr,
        &params,
        ctx.storage.block.epoch,
    )
}

/// Get all the validator known addresses. These validators may be in any state,
/// e.g. active, inactive or jailed.
fn validator_addresses<D, H>(
    ctx: RequestCtx<'_, D, H>,
    epoch: Option<Epoch>,
) -> storage_api::Result<HashSet<Address>>
where
    D: 'static + DB + for<'iter> DBIter<'iter> + Sync,
    H: 'static + StorageHasher + Sync,
{
    let epoch = epoch.unwrap_or(ctx.storage.last_epoch);
    ctx.storage.validator_addresses(epoch)
}

/// Get the total stake of a validator at the given epoch or current when
/// `None`. The total stake is a sum of validator's self-bonds and delegations
/// to their address.
fn validator_stake<D, H>(
    ctx: RequestCtx<'_, D, H>,
    validator: Address,
    epoch: Option<Epoch>,
) -> storage_api::Result<token::Amount>
where
    D: 'static + DB + for<'iter> DBIter<'iter> + Sync,
    H: 'static + StorageHasher + Sync,
{
    let epoch = epoch.unwrap_or(ctx.storage.last_epoch);
    ctx.storage.validator_stake(&validator, epoch)
}

/// Get the total stake in PoS system at the given epoch or current when `None`.
fn total_stake<D, H>(
    ctx: RequestCtx<'_, D, H>,
    epoch: Option<Epoch>,
) -> storage_api::Result<token::Amount>
where
    D: 'static + DB + for<'iter> DBIter<'iter> + Sync,
    H: 'static + StorageHasher + Sync,
{
    let epoch = epoch.unwrap_or(ctx.storage.last_epoch);
    ctx.storage.total_stake(epoch)
}

/// Get the total bond amount for the given bond ID (this may be delegation or
/// self-bond when `owner == validator`) at the given epoch, or the current
/// epoch when `None`.
fn bond_amount<D, H>(
    ctx: RequestCtx<'_, D, H>,
    owner: Address,
    validator: Address,
    epoch: Option<Epoch>,
) -> storage_api::Result<token::Amount>
where
    D: 'static + DB + for<'iter> DBIter<'iter> + Sync,
    H: 'static + StorageHasher + Sync,
{
    let epoch = epoch.unwrap_or(ctx.storage.last_epoch);

    let bond_id = BondId {
        source: owner,
        validator,
    };
    ctx.storage.bond_amount(&bond_id, epoch)
}

/// Find all the validator addresses to whom the given `owner` address has
/// some delegation in any epoch
fn delegations<D, H>(
    ctx: RequestCtx<'_, D, H>,
    owner: Address,
) -> storage_api::Result<HashSet<Address>>
where
    D: 'static + DB + for<'iter> DBIter<'iter> + Sync,
    H: 'static + StorageHasher + Sync,
{
    let bonds_prefix = pos::bonds_for_source_prefix(&owner);

    let mut delegations: HashSet<Address> = HashSet::new();
    for iter_result in
        storage_api::iter_prefix_bytes(ctx.storage, &bonds_prefix)?
    {
        let (key, _bonds_bytes) = iter_result?;
        let validator_address = pos::get_validator_address_from_bond(&key)
            .ok_or_else(|| {
                storage_api::Error::new_const(
                    "Delegation key should contain validator address.",
                )
            })?;
        delegations.insert(validator_address);
    }
    Ok(delegations)
}
