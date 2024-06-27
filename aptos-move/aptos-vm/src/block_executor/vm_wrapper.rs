// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{aptos_vm::AptosVM, block_executor::AptosTransactionOutput};
use aptos_block_executor::task::{ExecutionStatus, ExecutorTask};
use aptos_logger::{enabled, Level};
use aptos_mvhashmap::types::TxnIndex;
use aptos_types::{
    state_store::{StateView, StateViewId},
    transaction::signature_verified_transaction::SignatureVerifiedTransaction,
};
use aptos_vm_logging::{log_schema::AdapterLogSchema, prelude::*};
use aptos_vm_types::{
    environment::Environment,
    resolver::{ExecutorView, ResourceGroupView},
};
use fail::fail_point;
use move_core_types::vm_status::{StatusCode, VMStatus};
use std::sync::Arc;

pub(crate) struct AptosExecutorTask {
    vm: AptosVM,
    id: StateViewId,
}

impl ExecutorTask for AptosExecutorTask {
    type Environment = Arc<Environment>;
    type Error = VMStatus;
    type Output = AptosTransactionOutput;
    type Txn = SignatureVerifiedTransaction;

    fn init(env: Self::Environment, state_view: &impl StateView) -> Self {
        let vm = AptosVM::new_with_environment(env, state_view);
        let id = state_view.id();
        Self { vm, id }
    }

    // This function is called by the BlockExecutor for each transaction it intends
    // to execute (via the ExecutorTask trait). It can be as a part of sequential
    // execution, or speculatively as a part of a parallel execution.
    fn execute_transaction(
        &self,
        base_view: &impl StateView,
        executor_with_group_view: &(impl ExecutorView + ResourceGroupView),
        txn: &SignatureVerifiedTransaction,
        txn_idx: TxnIndex,
    ) -> ExecutionStatus<AptosTransactionOutput, VMStatus> {
        fail_point!("aptos_vm::vm_wrapper::execute_transaction", |_| {
            ExecutionStatus::DelayedFieldsCodeInvariantError("fail points error".into())
        });

        let log_context = AdapterLogSchema::new(self.id, txn_idx as usize);

        // We process direct write set payload here, to ensure that VM does not have to lift
        // storage-level abstractions into more fine-grained types used by the VM.
        if let Some(change_set) = txn.as_valid_direct_write_set_payload() {
            let execution_result =
                self.vm
                    .execute_direct_write_set_payload(base_view, change_set, &log_context);
            return match execution_result {
                Ok(output) => {
                    // Direct payload triggers reconfiguration, and subsequent transactions are skipped.
                    // Note that here we read all state keys in the change set from the base state view,
                    // which is fine: we do not use the results of these reads anyway. Changes made by
                    // this transaction are applied directly and also do not break any other outputs,
                    // because of reconfiguration.
                    speculative_info!(
                        &log_context,
                        "Reconfiguration occurred: restart required".into()
                    );
                    ExecutionStatus::MaterializedSkipRest(AptosTransactionOutput::new_committed(output))
                },
                Err(vm_status) => ExecutionStatus::Abort(vm_status),
            };
        }

        let resolver = self
            .vm
            .as_move_resolver_with_group_view(executor_with_group_view);
        match self
            .vm
            .execute_single_transaction(txn, &resolver, &log_context)
        {
            Ok((vm_status, vm_output)) => {
                if vm_output.status().is_discarded() {
                    speculative_trace!(
                        &log_context,
                        format!("Transaction discarded, status: {:?}", vm_status),
                    );
                }
                if vm_status.status_code() == StatusCode::SPECULATIVE_EXECUTION_ABORT_ERROR {
                    ExecutionStatus::SpeculativeExecutionAbortError(
                        vm_status.message().cloned().unwrap_or_default(),
                    )
                } else if vm_status.status_code()
                    == StatusCode::DELAYED_MATERIALIZATION_CODE_INVARIANT_ERROR
                {
                    ExecutionStatus::DelayedFieldsCodeInvariantError(
                        vm_status.message().cloned().unwrap_or_default(),
                    )
                } else if AptosVM::should_restart_execution(vm_output.change_set()) {
                    speculative_info!(
                        &log_context,
                        "Reconfiguration occurred: restart required".into()
                    );
                    ExecutionStatus::SkipRest(AptosTransactionOutput::new(vm_output))
                } else {
                    ExecutionStatus::Success(AptosTransactionOutput::new(vm_output))
                }
            },
            // execute_single_transaction only returns an error when transactions that should never fail
            // (BlockMetadataTransaction and GenesisTransaction) return an error themselves.
            Err(err) => {
                if err.status_code() == StatusCode::SPECULATIVE_EXECUTION_ABORT_ERROR {
                    ExecutionStatus::SpeculativeExecutionAbortError(
                        err.message().cloned().unwrap_or_default(),
                    )
                } else if err.status_code()
                    == StatusCode::DELAYED_MATERIALIZATION_CODE_INVARIANT_ERROR
                {
                    ExecutionStatus::DelayedFieldsCodeInvariantError(
                        err.message().cloned().unwrap_or_default(),
                    )
                } else {
                    ExecutionStatus::Abort(err)
                }
            },
        }
    }
}
