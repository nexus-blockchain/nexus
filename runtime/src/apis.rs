// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org>

// External crates imports
use alloc::vec::Vec;
use frame_support::{
	genesis_builder_helper::{build_state, get_preset},
	weights::Weight,
};
use pallet_grandpa::AuthorityId as GrandpaId;
use sp_api::impl_runtime_apis;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
	traits::{Block as BlockT, NumberFor},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, ExtrinsicInclusionMode,
};
use sp_version::RuntimeVersion;

// Local module imports
use super::{
	AccountId, Aura, Balance, Block, Executive, Grandpa, InherentDataExt, Nonce, Runtime,
	RuntimeCall, RuntimeGenesisConfig, SessionKeys, StorageService, NexMarket, CommissionPoolReward, System, TransactionPayment, VERSION,
};

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: <Block as BlockT>::LazyBlock) {
			Executive::execute_block(block.into());
		}

		fn initialize_block(header: &<Block as BlockT>::Header) -> ExtrinsicInclusionMode {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}

		fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
			Runtime::metadata_at_version(version)
		}

		fn metadata_versions() -> Vec<u32> {
			Runtime::metadata_versions()
		}
	}

	impl frame_support::view_functions::runtime_api::RuntimeViewFunction<Block> for Runtime {
		fn execute_view_function(id: frame_support::view_functions::ViewFunctionId, input: Vec<u8>) -> Result<Vec<u8>, frame_support::view_functions::ViewFunctionDispatchError> {
			Runtime::execute_view_function(id, input)
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: <Block as BlockT>::LazyBlock,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block.into())
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			pallet_aura::Authorities::<Runtime>::get().into_inner()
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> sp_consensus_grandpa::AuthorityList {
			Grandpa::grandpa_authorities()
		}

		fn current_set_id() -> sp_consensus_grandpa::SetId {
			Grandpa::current_set_id()
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			_equivocation_proof: sp_consensus_grandpa::EquivocationProof<
				<Block as BlockT>::Hash,
				NumberFor<Block>,
			>,
			_key_owner_proof: sp_consensus_grandpa::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			None
		}

		fn generate_key_ownership_proof(
			_set_id: sp_consensus_grandpa::SetId,
			_authority_id: GrandpaId,
		) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> {
			// NOTE: this is the only implementation possible since we've
			// defined our key owner proof type as a bottom type (i.e. a type
			// with no values).
			None
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(account: AccountId) -> Nonce {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
		for Runtime
	{
		fn query_call_info(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_call_info(call, len)
		}
		fn query_call_fee_details(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_call_fee_details(call, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{baseline, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use frame_system_benchmarking::Pallet as SystemBench;
			use frame_system_benchmarking::extensions::Pallet as SystemExtensionsBench;
			use baseline::Pallet as BaselineBench;
			use super::*;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();

			(list, storage_info)
		}

		#[allow(non_local_definitions)]
		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, alloc::string::String> {
			use frame_benchmarking::{baseline, BenchmarkBatch};
			use sp_storage::TrackedStorageKey;
			use frame_system_benchmarking::Pallet as SystemBench;
			use frame_system_benchmarking::extensions::Pallet as SystemExtensionsBench;
			use baseline::Pallet as BaselineBench;
			use super::*;

			impl frame_system_benchmarking::Config for Runtime {}
			impl baseline::Config for Runtime {}

			use frame_support::traits::WhitelistedStorageKeys;
			let whitelist: Vec<TrackedStorageKey> = AllPalletsWithSystem::whitelisted_storage_keys();

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			Ok(batches)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
			// NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
			// have a backtrace here. If any of the pre/post migration checks fail, we shall stop
			// right here and right now.
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, super::configs::RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(
			block: Block,
			state_root_check: bool,
			signature_check: bool,
			select: frame_try_runtime::TryStateSelect
		) -> Weight {
			// NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
			// have a backtrace here.
			Executive::try_execute_block(block, state_root_check, signature_check, select).expect("execute-block failed")
		}
	}

	impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
		fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
			build_state::<RuntimeGenesisConfig>(config)
		}

		fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
			get_preset::<RuntimeGenesisConfig>(id, crate::genesis_config_presets::get_preset)
		}

		fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
			crate::genesis_config_presets::preset_names()
		}
	}

	impl pallet_entity_member::runtime_api::MemberTeamApi<Block, AccountId> for Runtime {
		fn get_member_info(entity_id: u64, account: AccountId) -> Option<pallet_entity_member::runtime_api::MemberDashboardInfo<AccountId>> {
			pallet_entity_member::Pallet::<Runtime>::get_member_info(entity_id, &account)
		}

		fn get_referral_team(entity_id: u64, account: AccountId, depth: u32) -> Vec<pallet_entity_member::runtime_api::TeamMemberInfo<AccountId>> {
			pallet_entity_member::Pallet::<Runtime>::get_referral_team(entity_id, &account, depth)
		}

		fn get_entity_member_overview(entity_id: u64) -> pallet_entity_member::runtime_api::EntityMemberOverview {
			pallet_entity_member::Pallet::<Runtime>::get_entity_member_overview(entity_id)
		}

		fn get_members_paginated(entity_id: u64, page_size: u32, page_index: u32) -> pallet_entity_member::runtime_api::PaginatedMembersResult<AccountId> {
			pallet_entity_member::Pallet::<Runtime>::get_members_paginated(entity_id, page_size, page_index)
		}
	}

	impl pallet_commission_core::runtime_api::CommissionDashboardApi<Block, AccountId, Balance, u128> for Runtime {
		fn get_member_commission_dashboard(
			entity_id: u64,
			account: AccountId,
		) -> Option<pallet_commission_core::runtime_api::MemberCommissionDashboard<Balance, u128>> {
			pallet_commission_core::Pallet::<Runtime>::get_member_commission_dashboard(entity_id, &account)
		}

		fn get_direct_referral_info(
			entity_id: u64,
			account: AccountId,
		) -> pallet_commission_core::runtime_api::DirectReferralInfo<Balance> {
			pallet_commission_core::Pallet::<Runtime>::get_direct_referral_info(entity_id, &account)
		}

		fn get_team_performance_info(
			entity_id: u64,
			account: AccountId,
		) -> pallet_commission_core::runtime_api::TeamPerformanceInfo<Balance> {
			pallet_commission_core::Pallet::<Runtime>::get_team_performance_info(entity_id, &account)
		}

		fn get_entity_commission_overview(
			entity_id: u64,
		) -> pallet_commission_core::runtime_api::EntityCommissionOverview<Balance, u128> {
			pallet_commission_core::Pallet::<Runtime>::get_entity_commission_overview(entity_id)
		}

		fn get_direct_referral_details(
			entity_id: u64,
			account: AccountId,
		) -> pallet_commission_core::runtime_api::DirectReferralDetails<AccountId, Balance> {
			pallet_commission_core::Pallet::<Runtime>::get_direct_referral_details(entity_id, &account)
		}
	}

	impl pallet_ads_core::runtime_api::AdsDiscoveryApi<Block, AccountId, Balance> for Runtime {
		fn available_campaigns_for_placement(
			placement_id: pallet_ads_primitives::PlacementId,
			max_results: u32,
		) -> Vec<pallet_ads_core::runtime_api::CampaignSummary<AccountId, Balance>> {
			pallet_ads_core::Pallet::<Runtime>::available_campaigns_for_placement(&placement_id, max_results)
		}

		fn campaign_details(campaign_id: u64) -> Option<pallet_ads_core::runtime_api::CampaignDetail<AccountId, Balance>> {
			pallet_ads_core::Pallet::<Runtime>::campaign_details(campaign_id)
		}
	}

	impl pallet_nex_market::runtime_api::NexMarketApi<Block, AccountId, Balance> for Runtime {
		fn get_sell_orders() -> Vec<pallet_nex_market::runtime_api::OrderInfo<AccountId, Balance>> {
			NexMarket::api_get_sell_orders()
		}

		fn get_buy_orders() -> Vec<pallet_nex_market::runtime_api::OrderInfo<AccountId, Balance>> {
			NexMarket::api_get_buy_orders()
		}

		fn get_user_orders(user: AccountId) -> Vec<pallet_nex_market::runtime_api::OrderInfo<AccountId, Balance>> {
			NexMarket::api_get_user_orders(&user)
		}

		fn get_user_trades(user: AccountId) -> Vec<pallet_nex_market::runtime_api::TradeInfo<AccountId, Balance>> {
			NexMarket::api_get_user_trades(&user)
		}

		fn get_order_trades(order_id: u64) -> Vec<pallet_nex_market::runtime_api::TradeInfo<AccountId, Balance>> {
			NexMarket::api_get_order_trades(order_id)
		}

		fn get_active_trades(user: AccountId) -> Vec<pallet_nex_market::runtime_api::TradeInfo<AccountId, Balance>> {
			NexMarket::api_get_active_trades(&user)
		}

		fn get_order_depth() -> (
			Vec<pallet_nex_market::runtime_api::DepthEntry<Balance>>,
			Vec<pallet_nex_market::runtime_api::DepthEntry<Balance>>,
		) {
			NexMarket::api_get_order_depth()
		}

		fn get_best_prices() -> (Option<u64>, Option<u64>) {
			NexMarket::get_best_prices()
		}

		fn get_market_summary() -> pallet_nex_market::runtime_api::MarketSummary {
			NexMarket::api_get_market_summary()
		}
	}

	impl pallet_commission_pool_reward::runtime_api::PoolRewardDetailApi<Block, AccountId, Balance, u128> for Runtime {
		fn get_pool_reward_member_view(
			entity_id: u64,
			account: AccountId,
		) -> Option<pallet_commission_pool_reward::runtime_api::PoolRewardMemberView<Balance, u128>> {
			CommissionPoolReward::get_pool_reward_member_view(entity_id, &account)
		}

		fn get_pool_reward_admin_view(
			entity_id: u64,
		) -> Option<pallet_commission_pool_reward::runtime_api::PoolRewardAdminView<Balance, u128>> {
			CommissionPoolReward::get_pool_reward_admin_view(entity_id)
		}
	}

	impl pallet_storage_service::runtime_api::StorageServiceApi<Block, AccountId, Balance> for Runtime {
		fn get_user_funding_account(user: AccountId) -> AccountId {
			StorageService::derive_user_funding_account(&user)
		}

		fn get_user_funding_balance(user: AccountId) -> Balance {
			let funding_account = StorageService::derive_user_funding_account(&user);
			pallet_balances::Pallet::<Runtime>::free_balance(&funding_account)
		}

		fn get_subject_usage(user: AccountId, domain: u8, subject_id: u64) -> Balance {
			pallet_storage_service::SubjectUsage::<Runtime>::get((user, domain, subject_id))
		}

		fn get_user_all_usage(user: AccountId) -> Vec<(u8, u64, Balance)> {
			// 遍历 SubjectUsage 存储，筛选出该用户的所有记录
			pallet_storage_service::SubjectUsage::<Runtime>::iter()
				.filter_map(|((u, domain, subject_id), amount)| {
					if u == user {
						Some((domain, subject_id, amount))
					} else {
						None
					}
				})
				.collect()
		}
	}
}
