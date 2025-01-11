#![allow(clippy::not_unsafe_ptr_arg_deref)]
use std::mem::transmute;
use std::sync::Arc;
use std::sync::Once;

use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::SUCCESS;
use solana_sdk::instruction::Instruction;
use solana_sdk::instruction::InstructionError;
use solana_sdk::program_error::UNSUPPORTED_SYSVAR;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::stable_layout::stable_instruction::StableInstruction;
use solana_sdk::sysvar::Sysvar;

use solana_program::program_error::ProgramError;
use solana_program::program_stubs;
use solana_program::program_stubs::set_syscall_stubs;

use solana_program_runtime::stable_log;
use solana_program_runtime::timings::ExecuteTimings;

use crate::get_invoke_context;
use crate::TridentTryFrom;

static ONCE: Once = Once::new();

pub fn set_stubs_v1() {
    ONCE.call_once(|| {
        set_syscall_stubs(Box::new(TridentSyscallStubs {}));
    });
}

fn get_sysvar<T: Default + Sysvar + Sized + serde::de::DeserializeOwned + Clone>(
    sysvar: Result<Arc<T>, InstructionError>,
    var_addr: *mut u8,
) -> u64 {
    match sysvar {
        Ok(sysvar_data) => unsafe {
            *(var_addr as *mut _ as *mut T) = T::clone(&sysvar_data);
            SUCCESS
        },
        Err(_) => UNSUPPORTED_SYSVAR,
    }
}

pub struct TridentSyscallStubs;

impl program_stubs::SyscallStubs for TridentSyscallStubs {
    fn sol_log(&self, message: &str) {
        let invoke_context = get_invoke_context();
        let log_collector = invoke_context.get_log_collector();

        stable_log::program_log(&log_collector, message);
    }

    fn sol_get_rent_sysvar(&self, var_addr: *mut u8) -> u64 {
        get_sysvar(get_invoke_context().get_sysvar_cache().get_rent(), var_addr)
    }
    fn sol_get_clock_sysvar(&self, var_addr: *mut u8) -> u64 {
        get_sysvar(
            get_invoke_context().get_sysvar_cache().get_clock(),
            var_addr,
        )
    }

    fn sol_get_epoch_schedule_sysvar(&self, var_addr: *mut u8) -> u64 {
        get_sysvar(
            get_invoke_context().get_sysvar_cache().get_epoch_schedule(),
            var_addr,
        )
    }

    fn sol_get_epoch_rewards_sysvar(&self, var_addr: *mut u8) -> u64 {
        get_sysvar(
            get_invoke_context().get_sysvar_cache().get_epoch_rewards(),
            var_addr,
        )
    }
    #[allow(deprecated)]
    fn sol_get_fees_sysvar(&self, var_addr: *mut u8) -> u64 {
        get_sysvar(get_invoke_context().get_sysvar_cache().get_fees(), var_addr)
    }

    fn sol_get_last_restart_slot(&self, var_addr: *mut u8) -> u64 {
        get_sysvar(
            get_invoke_context()
                .get_sysvar_cache()
                .get_last_restart_slot(),
            var_addr,
        )
    }
    fn sol_invoke_signed(
        &self,
        instruction: &solana_program::instruction::Instruction,
        account_infos: &[solana_program::account_info::AccountInfo<'_>],
        signers_seeds: &[&[&[u8]]],
    ) -> std::result::Result<(), solana_program::program_error::ProgramError> {
        let instruction = unsafe {
            transmute::<&solana_program::instruction::Instruction, &Instruction>(instruction)
        };

        let account_infos = unsafe {
            transmute::<&[solana_program::account_info::AccountInfo<'_>], &[AccountInfo<'_>]>(
                account_infos,
            )
        };

        let instruction = StableInstruction::from(instruction.clone());
        let invoke_context = get_invoke_context();
        let log_collector = invoke_context.get_log_collector();
        let transaction_context = &invoke_context.transaction_context;

        let instruction_context = transaction_context
            .get_current_instruction_context()
            .map_err(|err| {
                ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
            })?;

        let caller = instruction_context
            .get_last_program_key(transaction_context)
            .map_err(|err| {
                ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
            })?;

        let signers = signers_seeds
            .iter()
            .map(|seeds| Pubkey::create_program_address(seeds, caller).unwrap())
            .collect::<Vec<_>>();

        let (instruction_accounts, program_indices) = invoke_context
            .prepare_instruction(&instruction, &signers)
            .map_err(|err| {
                ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
            })?;

        // Copy caller's account_info modifications into invoke_context accounts
        let transaction_context = &invoke_context.transaction_context;

        let instruction_context = transaction_context
            .get_current_instruction_context()
            .map_err(|err| {
                ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
            })?;

        let mut account_indices = Vec::with_capacity(instruction_accounts.len());

        for instruction_account in instruction_accounts.iter() {
            let account_key = transaction_context
                .get_key_of_account_at_index(instruction_account.index_in_transaction)
                .map_err(|err| {
                    ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
                })?;
            let account_info_index = account_infos
                .iter()
                .position(|account_info| account_info.unsigned_key() == account_key)
                .ok_or(InstructionError::MissingAccount)
                .map_err(|err| {
                    ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
                })?;
            let account_info = &account_infos[account_info_index];
            let mut borrowed_account = instruction_context
                .try_borrow_instruction_account(
                    transaction_context,
                    instruction_account.index_in_caller,
                )
                .map_err(|err| {
                    ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
                })?;
            if borrowed_account.get_lamports() != account_info.lamports() {
                borrowed_account
                    .set_lamports(account_info.lamports())
                    .map_err(|err| {
                        ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
                    })?;
            }
            // Beware the program it retursn if from different crate version
            let account_info_data = account_info.try_borrow_data().unwrap();
            // The redundant check helps to avoid the expensive data comparison if we can
            match borrowed_account
                .can_data_be_resized(account_info_data.len())
                .and_then(|_| borrowed_account.can_data_be_changed())
            {
                Ok(()) => borrowed_account
                    .set_data_from_slice(&account_info_data)
                    .map_err(|err| {
                        ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
                    })?,
                Err(err) if borrowed_account.get_data() != *account_info_data => {
                    panic!("{err:?}");
                }
                _ => {}
            }
            // Change the owner at the end so that we are allowed to change the lamports and data before
            if borrowed_account.get_owner() != account_info.owner {
                borrowed_account
                    .set_owner(account_info.owner.as_ref())
                    .map_err(|err| {
                        ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
                    })?;
            }
            if instruction_account.is_writable {
                account_indices.push((instruction_account.index_in_caller, account_info_index));
            }
        }

        let mut compute_units_consumed = 0;

        stable_log::program_invoke(
            &log_collector,
            &instruction.program_id,
            invoke_context.get_stack_height(),
        );

        invoke_context
            .process_instruction(
                &instruction.data,
                &instruction_accounts,
                &program_indices,
                &mut compute_units_consumed,
                &mut ExecuteTimings::default(),
            )
            .map_err(|err| {
                ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
            })?;

        // Copy invoke_context accounts modifications into caller's account_info
        let transaction_context = &invoke_context.transaction_context;

        let instruction_context = transaction_context
            .get_current_instruction_context()
            .map_err(|err| {
                ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
            })?;

        for (index_in_caller, account_info_index) in account_indices.into_iter() {
            let borrowed_account = instruction_context
                .try_borrow_instruction_account(transaction_context, index_in_caller)
                .map_err(|err| {
                    ProgramError::try_from_custom(err).unwrap_or_else(|err| panic!("{}", err))
                })?;
            let account_info = &account_infos[account_info_index];
            **account_info.try_borrow_mut_lamports().unwrap() = borrowed_account.get_lamports();
            if account_info.owner != borrowed_account.get_owner() {
                // TODO Figure out a better way to allow the System Program to set the account owner
                #[allow(clippy::transmute_ptr_to_ptr)]
                #[allow(mutable_transmutes)]
                let account_info_mut =
                    unsafe { transmute::<&Pubkey, &mut Pubkey>(account_info.owner) };
                *account_info_mut = *borrowed_account.get_owner();
            }

            let new_data = borrowed_account.get_data();
            let new_len = new_data.len();

            // Resize account_info data
            if account_info.data_len() != new_len {
                account_info.realloc(new_len, false).unwrap();
            }

            // Clone the data
            let mut data = account_info.try_borrow_mut_data().unwrap();

            data.clone_from_slice(new_data);
        }

        stable_log::program_success(&log_collector, &instruction.program_id);

        Ok(())
    }
    fn sol_get_return_data(
        &self,
    ) -> std::option::Option<(solana_program::pubkey::Pubkey, std::vec::Vec<u8>)> {
        let (program_id, data) = get_invoke_context().transaction_context.get_return_data();

        let program_id =
            unsafe { transmute::<&Pubkey, &solana_program::pubkey::Pubkey>(program_id) };
        Some((*program_id, data.to_vec()))
    }
    fn sol_set_return_data(&self, data: &[u8]) {
        let invoke_context = get_invoke_context();
        let transaction_context = &mut invoke_context.transaction_context;
        let instruction_context = transaction_context
            .get_current_instruction_context()
            .unwrap();
        let caller = *instruction_context
            .get_last_program_key(transaction_context)
            .unwrap();
        transaction_context
            .set_return_data(caller, data.to_vec())
            .unwrap();
    }

    fn sol_get_stack_height(&self) -> u64 {
        let invoke_context = get_invoke_context();
        invoke_context.get_stack_height().try_into().unwrap()
    }
}

/// The V1 implementation is meant for solana crate version 1.17 and higher. As 1.17 is considered as
/// completely different type compared to 2.0, we need to implement this manually.
impl TridentTryFrom<InstructionError> for ProgramError {
    type Error = InstructionError;

    fn try_from_custom(error: InstructionError) -> Result<Self, Self::Error> {
        match error {
            Self::Error::Custom(err) => Ok(Self::Custom(err)),
            Self::Error::InvalidArgument => Ok(Self::InvalidArgument),
            Self::Error::InvalidInstructionData => Ok(Self::InvalidInstructionData),
            Self::Error::InvalidAccountData => Ok(Self::InvalidAccountData),
            Self::Error::AccountDataTooSmall => Ok(Self::AccountDataTooSmall),
            Self::Error::InsufficientFunds => Ok(Self::InsufficientFunds),
            Self::Error::IncorrectProgramId => Ok(Self::IncorrectProgramId),
            Self::Error::MissingRequiredSignature => Ok(Self::MissingRequiredSignature),
            Self::Error::AccountAlreadyInitialized => Ok(Self::AccountAlreadyInitialized),
            Self::Error::UninitializedAccount => Ok(Self::UninitializedAccount),
            Self::Error::NotEnoughAccountKeys => Ok(Self::NotEnoughAccountKeys),
            Self::Error::AccountBorrowFailed => Ok(Self::AccountBorrowFailed),
            Self::Error::MaxSeedLengthExceeded => Ok(Self::MaxSeedLengthExceeded),
            Self::Error::InvalidSeeds => Ok(Self::InvalidSeeds),
            Self::Error::BorshIoError(err) => Ok(Self::BorshIoError(err)),
            Self::Error::AccountNotRentExempt => Ok(Self::AccountNotRentExempt),
            Self::Error::UnsupportedSysvar => Ok(Self::UnsupportedSysvar),
            Self::Error::IllegalOwner => Ok(Self::IllegalOwner),
            Self::Error::MaxAccountsDataAllocationsExceeded => {
                Ok(Self::MaxAccountsDataAllocationsExceeded)
            }
            Self::Error::InvalidRealloc => Ok(Self::InvalidRealloc),
            Self::Error::MaxInstructionTraceLengthExceeded => {
                Ok(Self::MaxInstructionTraceLengthExceeded)
            }
            Self::Error::BuiltinProgramsMustConsumeComputeUnits => {
                Ok(Self::BuiltinProgramsMustConsumeComputeUnits)
            }
            Self::Error::InvalidAccountOwner => Ok(Self::InvalidAccountOwner),
            Self::Error::ArithmeticOverflow => Ok(Self::ArithmeticOverflow),
            _ => Err(error),
        }
    }
}
