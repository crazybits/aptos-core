// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use crate::StateComputeResult;
use anyhow::{ensure, Result};
use aptos_crypto::HashValue;
use aptos_storage_interface::cached_state_view::ShardedStateCache;
use aptos_types::{
    contract_event::ContractEvent,
    epoch_state::EpochState,
    proof::accumulator::InMemoryTransactionAccumulator,
    state_store::ShardedStateUpdates,
    transaction::{
        block_epilogue::BlockEndInfo, TransactionInfo, TransactionStatus, TransactionToCommit,
        Version,
    },
};
use itertools::zip_eq;
use std::{ops::Deref, sync::Arc};

#[derive(Clone, Debug, Default)]
pub struct LedgerUpdateOutput {
    inner: Arc<Inner>,
}

impl Deref for LedgerUpdateOutput {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl LedgerUpdateOutput {
    pub fn new_empty(transaction_accumulator: Arc<InMemoryTransactionAccumulator>) -> Self {
        Self::new_impl(Inner::new_empty(transaction_accumulator))
    }

    pub fn new_dummy_with_compute_status(statuses: Vec<TransactionStatus>) -> Self {
        Self::new_impl(Inner::new_dummy_with_compute_status(statuses))
    }

    pub fn new_dummy_with_root_hash(root_hash: HashValue) -> Self {
        Self::new_impl(Inner::new_dummy_with_root_hash(root_hash))
    }

    pub fn reconfig_suffix(&self) -> Self {
        Self::new_impl(Inner::new_empty(self.transaction_accumulator.clone()))
    }

    pub fn new(
        statuses_for_input_txns: Vec<TransactionStatus>,
        to_commit: Vec<TransactionToCommit>,
        subscribable_events: Vec<ContractEvent>,
        transaction_info_hashes: Vec<HashValue>,
        state_updates_until_last_checkpoint: Option<ShardedStateUpdates>,
        sharded_state_cache: ShardedStateCache,
        transaction_accumulator: Arc<InMemoryTransactionAccumulator>,
        parent_accumulator: Arc<InMemoryTransactionAccumulator>,
        block_end_info: Option<BlockEndInfo>,
    ) -> Self {
        Self::new_impl(Inner {
            statuses_for_input_txns,
            to_commit,
            subscribable_events,
            transaction_info_hashes,
            state_updates_until_last_checkpoint,
            sharded_state_cache,
            transaction_accumulator,
            parent_accumulator,
            block_end_info,
        })
    }

    fn new_impl(inner: Inner) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn as_state_compute_result(
        &self,
        next_epoch_state: Option<EpochState>,
    ) -> StateComputeResult {
        StateComputeResult::new(self.clone(), next_epoch_state)
    }
}

#[derive(Default, Debug)]
pub struct Inner {
    pub statuses_for_input_txns: Vec<TransactionStatus>,
    pub to_commit: Vec<TransactionToCommit>,
    pub subscribable_events: Vec<ContractEvent>,
    pub transaction_info_hashes: Vec<HashValue>,
    pub state_updates_until_last_checkpoint: Option<ShardedStateUpdates>,
    pub sharded_state_cache: ShardedStateCache,
    /// The in-memory Merkle Accumulator representing a blockchain state consistent with the
    /// `state_tree`.
    pub transaction_accumulator: Arc<InMemoryTransactionAccumulator>,
    pub parent_accumulator: Arc<InMemoryTransactionAccumulator>,
    pub block_end_info: Option<BlockEndInfo>,
}

impl Inner {
    pub fn new_empty(transaction_accumulator: Arc<InMemoryTransactionAccumulator>) -> Self {
        Self {
            parent_accumulator: transaction_accumulator.clone(),
            transaction_accumulator,
            ..Default::default()
        }
    }

    pub fn new_dummy_with_compute_status(statuses: Vec<TransactionStatus>) -> Self {
        Self {
            statuses_for_input_txns: statuses,
            ..Default::default()
        }
    }

    pub fn new_dummy_with_root_hash(root_hash: HashValue) -> Self {
        let transaction_accumulator = Arc::new(
            InMemoryTransactionAccumulator::new_empty_with_root_hash(root_hash),
        );
        Self {
            parent_accumulator: transaction_accumulator.clone(),
            transaction_accumulator,
            ..Default::default()
        }
    }

    pub fn txn_accumulator(&self) -> &Arc<InMemoryTransactionAccumulator> {
        &self.transaction_accumulator
    }

    pub fn transactions_to_commit(&self) -> &Vec<TransactionToCommit> {
        &self.to_commit
    }

    /// Ensure that every block committed by consensus ends with a state checkpoint. That can be
    /// one of the two cases: 1. a reconfiguration (txns in the proposed block after the txn caused
    /// the reconfiguration will be retried) 2. a Transaction::StateCheckpoint at the end of the
    /// block.
    pub fn ensure_ends_with_state_checkpoint(&self) -> Result<()> {
        ensure!(
            self.to_commit
                .last()
                .map_or(true, |txn| txn.transaction().is_non_reconfig_block_ending()),
            "Block not ending with a state checkpoint.",
        );
        Ok(())
    }

    pub fn ensure_transaction_infos_match(
        &self,
        transaction_infos: &[TransactionInfo],
    ) -> Result<()> {
        let first_version =
            self.transaction_accumulator.version() + 1 - self.to_commit.len() as Version;
        ensure!(
            self.transactions_to_commit().len() == transaction_infos.len(),
            "Lengths don't match. {} vs {}",
            self.transactions_to_commit().len(),
            transaction_infos.len(),
        );

        let mut version = first_version;
        for (txn_to_commit, expected_txn_info) in
            zip_eq(self.to_commit.iter(), transaction_infos.iter())
        {
            let txn_info = txn_to_commit.transaction_info();
            ensure!(
                txn_info == expected_txn_info,
                "Transaction infos don't match. version:{version}, txn_info:{txn_info}, expected_txn_info:{expected_txn_info}",
            );
            version += 1;
        }
        Ok(())
    }

    pub fn next_version(&self) -> Version {
        self.transaction_accumulator.num_leaves() as Version
    }

    pub fn last_version(&self) -> Version {
        self.next_version()
            .checked_sub(1)
            .expect("Empty block before genesis.")
    }

    pub fn first_version(&self) -> Version {
        self.transaction_accumulator.num_leaves() - self.to_commit.len() as Version
    }

    pub fn num_txns(&self) -> usize {
        self.to_commit.len()
    }
}
