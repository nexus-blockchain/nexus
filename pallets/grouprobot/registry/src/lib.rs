#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet GroupRobot Registry — Bot 注册 + TEE 节点管理 + 平台身份绑定 + 证明管理
//!
//! 整合现有 `pallet-bot-registry` + `attestation.rs` 相关链上逻辑。
//!
//! ## 功能
//! - Bot 注册 (bot_id_hash, public_key)
//! - 公钥更换 (密钥轮换)
//! - 社区绑定 / 解绑
//! - 用户平台身份绑定
//! - TEE 双证明提交 / 刷新 (TDX+SGX)
//! - MRTD / MRENCLAVE 白名单管理
//! - on_initialize: 过期证明自动降级

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use pallet_grouprobot_primitives::*;
use scale_info::TypeInfo;
use sp_runtime::traits::{Saturating, UniqueSaturatedInto};

/// Bot 注册信息
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct BotInfo<T: Config> {
	pub owner: T::AccountId,
	pub bot_id_hash: BotIdHash,
	pub public_key: [u8; 32],
	pub status: BotStatus,
	pub registered_at: BlockNumberFor<T>,
	pub node_type: NodeType,
	pub community_count: u32,
}

/// 社区绑定记录
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct CommunityBinding<T: Config> {
	pub community_id_hash: CommunityIdHash,
	pub platform: Platform,
	pub bot_id_hash: BotIdHash,
	pub bound_by: T::AccountId,
	pub bound_at: BlockNumberFor<T>,
}

/// TDX Quote v4 结构偏移量 (用于链上解析)
/// Header(48) + TEE_TCB_SVN(16) + MRSEAM(48) + MRSIGNERSEAM(48) +
/// SEAMATTRIBUTES(8) + TDATTRIBUTES(8) + XFAM(8) = 184
pub const TDX_MRTD_OFFSET: usize = 184;
pub const TDX_MRTD_LEN: usize = 48;
/// REPORTDATA 在 TD Quote Body 中的偏移: 184 + MRTD(48) + MRCONFIGID(48) +
/// MROWNER(48) + MROWNERCONFIG(48) + RTMR0(48) + RTMR1(48) + RTMR2(48) + RTMR3(48) = 568
pub const TDX_REPORTDATA_OFFSET: usize = 568;
pub const TDX_REPORTDATA_LEN: usize = 64;
/// 最小 TDX Quote 长度: Header(48) + TD Quote Body(584) = 632
pub const TDX_MIN_QUOTE_LEN: usize = TDX_REPORTDATA_OFFSET + TDX_REPORTDATA_LEN;

/// TEE 证明记录
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct AttestationRecord<T: Config> {
	pub bot_id_hash: BotIdHash,
	pub tdx_quote_hash: [u8; 32],
	pub sgx_quote_hash: Option<[u8; 32]>,
	pub mrtd: [u8; 48],
	pub mrenclave: Option<[u8; 32]>,
	pub attester: T::AccountId,
	pub attested_at: BlockNumberFor<T>,
	pub expires_at: BlockNumberFor<T>,
	pub is_dual_attestation: bool,
	/// 证明是否从原始 Quote 验证 (防 MRTD 伪造)
	pub quote_verified: bool,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// 单个所有者最大 Bot 数
		#[pallet::constant]
		type MaxBotsPerOwner: Get<u32>;
		/// 单个社区最大平台绑定数
		#[pallet::constant]
		type MaxPlatformsPerCommunity: Get<u32>;
		/// 用户最大平台绑定数
		#[pallet::constant]
		type MaxPlatformBindingsPerUser: Get<u32>;
		/// TEE 证明有效期 (区块数)
		#[pallet::constant]
		type AttestationValidityBlocks: Get<BlockNumberFor<Self>>;
		/// 证明过期扫描间隔 (区块数)
		#[pallet::constant]
		type AttestationCheckInterval: Get<BlockNumberFor<Self>>;
		/// TDX Quote 最大字节长度
		#[pallet::constant]
		type MaxQuoteLen: Get<u32>;
	}

	// ========================================================================
	// Storage
	// ========================================================================

	/// Bot 注册表: bot_id_hash → BotInfo
	#[pallet::storage]
	pub type Bots<T: Config> = StorageMap<_, Blake2_128Concat, BotIdHash, BotInfo<T>>;

	/// 所有者的 Bot 列表: owner → Vec<bot_id_hash>
	#[pallet::storage]
	pub type OwnerBots<T: Config> = StorageMap<
		_, Blake2_128Concat, T::AccountId,
		BoundedVec<BotIdHash, T::MaxBotsPerOwner>, ValueQuery,
	>;

	/// 社区绑定: community_id_hash → CommunityBinding
	#[pallet::storage]
	pub type CommunityBindings<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, CommunityBinding<T>>;

	/// 用户平台身份绑定: (account, platform) → platform_user_id_hash
	#[pallet::storage]
	pub type UserPlatformBindings<T: Config> = StorageDoubleMap<
		_, Blake2_128Concat, T::AccountId,
		Blake2_128Concat, Platform,
		[u8; 32],
	>;

	/// TEE 证明记录: bot_id_hash → AttestationRecord
	#[pallet::storage]
	pub type Attestations<T: Config> =
		StorageMap<_, Blake2_128Concat, BotIdHash, AttestationRecord<T>>;

	/// 审批的 MRTD 白名单: mrtd → version
	#[pallet::storage]
	pub type ApprovedMrtd<T: Config> = StorageMap<_, Blake2_128Concat, [u8; 48], u32>;

	/// 审批的 MRENCLAVE 白名单: mrenclave → version
	#[pallet::storage]
	pub type ApprovedMrenclave<T: Config> = StorageMap<_, Blake2_128Concat, [u8; 32], u32>;

	/// Bot 总数
	#[pallet::storage]
	pub type BotCount<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// 证明 Nonce: bot_id_hash → (nonce, issued_at_block)
	/// 用于防止 Quote 重放攻击
	#[pallet::storage]
	pub type AttestationNonces<T: Config> =
		StorageMap<_, Blake2_128Concat, BotIdHash, ([u8; 32], BlockNumberFor<T>)>;

	// ========================================================================
	// Events
	// ========================================================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		BotRegistered { bot_id_hash: BotIdHash, owner: T::AccountId },
		PublicKeyUpdated { bot_id_hash: BotIdHash, new_key: [u8; 32] },
		BotDeactivated { bot_id_hash: BotIdHash },
		BotSuspended { bot_id_hash: BotIdHash },
		BotReactivated { bot_id_hash: BotIdHash },
		CommunityBound { community_id_hash: CommunityIdHash, bot_id_hash: BotIdHash, platform: Platform },
		CommunityUnbound { community_id_hash: CommunityIdHash },
		UserPlatformBound { account: T::AccountId, platform: Platform },
		UserPlatformUnbound { account: T::AccountId, platform: Platform },
		AttestationSubmitted { bot_id_hash: BotIdHash, is_dual: bool },
		AttestationRefreshed { bot_id_hash: BotIdHash },
		AttestationExpired { bot_id_hash: BotIdHash },
		NonceIssued { bot_id_hash: BotIdHash, nonce: [u8; 32] },
		MrtdApproved { mrtd: [u8; 48], version: u32 },
		MrenclaveApproved { mrenclave: [u8; 32], version: u32 },
	}

	// ========================================================================
	// Errors
	// ========================================================================

	#[pallet::error]
	pub enum Error<T> {
		/// Bot 已注册
		BotAlreadyRegistered,
		/// Bot 不存在
		BotNotFound,
		/// 不是 Bot 所有者
		NotBotOwner,
		/// Bot 不活跃
		BotNotActive,
		/// Bot 已停用
		BotAlreadyDeactivated,
		/// 所有者 Bot 数量已满
		MaxBotsReached,
		/// 社区已绑定
		CommunityAlreadyBound,
		/// 社区未绑定
		CommunityNotBound,
		/// MRTD 未在白名单中
		MrtdNotApproved,
		/// MRENCLAVE 未在白名单中
		MrenclaveNotApproved,
		/// 证明不存在
		AttestationNotFound,
		/// 证明已过期
		AttestationExpired,
		/// MRTD 已在白名单中
		MrtdAlreadyApproved,
		/// MRENCLAVE 已在白名单中
		MrenclaveAlreadyApproved,
		/// 公钥不能与旧公钥相同
		SamePublicKey,
		/// Bot 已暂停
		BotAlreadySuspended,
		/// Bot 未暂停
		BotNotSuspended,
		/// TDX Quote 太短 (无法解析)
		QuoteTooShort,
		/// Quote 中 report_data 与 Bot 公钥不匹配 (代码可能被篡改)
		QuoteReportDataMismatch,
		/// 未请求 Nonce (需先调用 request_attestation_nonce)
		NonceMissing,
		/// Nonce 已过期 (超过 AttestationValidityBlocks 未使用)
		NonceExpired,
		/// Quote 中的 nonce 与链上存储的不匹配 (疑似重放攻击)
		NonceMismatch,
	}

	// ========================================================================
	// Hooks
	// ========================================================================

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			let interval = T::AttestationCheckInterval::get();
			if interval == BlockNumberFor::<T>::default() {
				return Weight::zero();
			}
			if (n % interval) != BlockNumberFor::<T>::default() {
				return Weight::zero();
			}

			let mut reads: u64 = 0;
			let mut writes: u64 = 0;

			let mut expired_bots: alloc::vec::Vec<BotIdHash> = alloc::vec::Vec::new();
			for (bot_id_hash, record) in Attestations::<T>::iter() {
				reads += 1;
				if n >= record.expires_at {
					expired_bots.push(bot_id_hash);
				}
			}

			for bot_id_hash in expired_bots {
				Attestations::<T>::remove(&bot_id_hash);
				writes += 1;

				Bots::<T>::mutate(&bot_id_hash, |maybe_bot| {
					if let Some(bot) = maybe_bot {
						bot.node_type = NodeType::StandardNode;
						writes += 1;
					}
				});
				reads += 1;

				Self::deposit_event(Event::AttestationExpired { bot_id_hash });
			}

			Weight::from_parts(
				reads.saturating_mul(25_000_000).saturating_add(writes.saturating_mul(25_000_000)),
				reads.saturating_mul(5_000).saturating_add(writes.saturating_mul(5_000)),
			)
		}
	}

	// ========================================================================
	// Extrinsics
	// ========================================================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// 注册 Bot
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(50_000_000, 10_000))]
		pub fn register_bot(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			public_key: [u8; 32],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!Bots::<T>::contains_key(&bot_id_hash), Error::<T>::BotAlreadyRegistered);

			OwnerBots::<T>::try_mutate(&who, |bots| -> DispatchResult {
				bots.try_push(bot_id_hash).map_err(|_| Error::<T>::MaxBotsReached)?;
				Ok(())
			})?;

			let now = frame_system::Pallet::<T>::block_number();
			let bot = BotInfo::<T> {
				owner: who.clone(),
				bot_id_hash,
				public_key,
				status: BotStatus::Active,
				registered_at: now,
				node_type: NodeType::StandardNode,
				community_count: 0,
			};
			Bots::<T>::insert(&bot_id_hash, bot);
			BotCount::<T>::mutate(|c| *c = c.saturating_add(1));

			Self::deposit_event(Event::BotRegistered { bot_id_hash, owner: who });
			Ok(())
		}

		/// 更换 Bot 公钥 (密钥轮换)
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn update_public_key(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			new_key: [u8; 32],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Bots::<T>::try_mutate(&bot_id_hash, |maybe_bot| -> DispatchResult {
				let bot = maybe_bot.as_mut().ok_or(Error::<T>::BotNotFound)?;
				ensure!(bot.owner == who, Error::<T>::NotBotOwner);
				ensure!(bot.public_key != new_key, Error::<T>::SamePublicKey);
				bot.public_key = new_key;
				Ok(())
			})?;
			Self::deposit_event(Event::PublicKeyUpdated { bot_id_hash, new_key });
			Ok(())
		}

		/// 停用 Bot
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn deactivate_bot(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Bots::<T>::try_mutate(&bot_id_hash, |maybe_bot| -> DispatchResult {
				let bot = maybe_bot.as_mut().ok_or(Error::<T>::BotNotFound)?;
				ensure!(bot.owner == who, Error::<T>::NotBotOwner);
				ensure!(bot.status != BotStatus::Deactivated, Error::<T>::BotAlreadyDeactivated);
				bot.status = BotStatus::Deactivated;
				Ok(())
			})?;
			Self::deposit_event(Event::BotDeactivated { bot_id_hash });
			Ok(())
		}

		/// 绑定社区到 Bot
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::from_parts(45_000_000, 8_000))]
		pub fn bind_community(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			community_id_hash: CommunityIdHash,
			platform: Platform,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);
			ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);
			ensure!(
				!CommunityBindings::<T>::contains_key(&community_id_hash),
				Error::<T>::CommunityAlreadyBound
			);

			let now = frame_system::Pallet::<T>::block_number();
			let binding = CommunityBinding::<T> {
				community_id_hash,
				platform,
				bot_id_hash,
				bound_by: who,
				bound_at: now,
			};
			CommunityBindings::<T>::insert(&community_id_hash, binding);

			Bots::<T>::mutate(&bot_id_hash, |maybe_bot| {
				if let Some(bot) = maybe_bot {
					bot.community_count = bot.community_count.saturating_add(1);
				}
			});

			Self::deposit_event(Event::CommunityBound { community_id_hash, bot_id_hash, platform });
			Ok(())
		}

		/// 解绑社区
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn unbind_community(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let binding = CommunityBindings::<T>::get(&community_id_hash)
				.ok_or(Error::<T>::CommunityNotBound)?;

			let bot = Bots::<T>::get(&binding.bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);

			CommunityBindings::<T>::remove(&community_id_hash);

			Bots::<T>::mutate(&binding.bot_id_hash, |maybe_bot| {
				if let Some(bot) = maybe_bot {
					bot.community_count = bot.community_count.saturating_sub(1);
				}
			});

			Self::deposit_event(Event::CommunityUnbound { community_id_hash });
			Ok(())
		}

		/// 用户绑定平台身份
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn bind_user_platform(
			origin: OriginFor<T>,
			platform: Platform,
			platform_user_id_hash: [u8; 32],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			UserPlatformBindings::<T>::insert(&who, platform, platform_user_id_hash);
			Self::deposit_event(Event::UserPlatformBound { account: who, platform });
			Ok(())
		}

		/// 提交 TEE 双证明 (TDX+SGX Quote)
		#[pallet::call_index(6)]
		#[pallet::weight(Weight::from_parts(60_000_000, 12_000))]
		/// ⚠️ 仅用于软件模式 (is_simulated=true)。硬件节点应使用 submit_verified_attestation。
		pub fn submit_attestation(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			tdx_quote_hash: [u8; 32],
			sgx_quote_hash: Option<[u8; 32]>,
			mrtd: [u8; 48],
			mrenclave: Option<[u8; 32]>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let mut bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);

			// 检查 MRTD 白名单
			ensure!(ApprovedMrtd::<T>::contains_key(&mrtd), Error::<T>::MrtdNotApproved);

			// 如果提供了 MRENCLAVE，检查白名单
			if let Some(ref mre) = mrenclave {
				ensure!(ApprovedMrenclave::<T>::contains_key(mre), Error::<T>::MrenclaveNotApproved);
			}

			let now = frame_system::Pallet::<T>::block_number();
			let expires_at = now.saturating_add(T::AttestationValidityBlocks::get());
			let is_dual = sgx_quote_hash.is_some() && mrenclave.is_some();

			let record = AttestationRecord::<T> {
				bot_id_hash,
				tdx_quote_hash,
				sgx_quote_hash,
				mrtd,
				mrenclave,
				attester: who,
				attested_at: now,
				expires_at,
				is_dual_attestation: is_dual,
				quote_verified: false,
			};

			Attestations::<T>::insert(&bot_id_hash, record);

			// 更新 BotInfo 的 node_type
			let now_u64: u64 = now.unique_saturated_into();
			let expires_u64: u64 = expires_at.unique_saturated_into();
			let sgx_attested_at = if is_dual { Some(now_u64) } else { None };
			bot.node_type = NodeType::TeeNode {
				mrtd,
				mrenclave,
				tdx_attested_at: now_u64,
				sgx_attested_at,
				expires_at: expires_u64,
			};
			Bots::<T>::insert(&bot_id_hash, bot);

			Self::deposit_event(Event::AttestationSubmitted { bot_id_hash, is_dual });
			Ok(())
		}

		/// 刷新 TEE 证明 (24h 周期)
		#[pallet::call_index(7)]
		#[pallet::weight(Weight::from_parts(55_000_000, 10_000))]
		pub fn refresh_attestation(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			tdx_quote_hash: [u8; 32],
			sgx_quote_hash: Option<[u8; 32]>,
			mrtd: [u8; 48],
			mrenclave: Option<[u8; 32]>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);
			ensure!(Attestations::<T>::contains_key(&bot_id_hash), Error::<T>::AttestationNotFound);

			ensure!(ApprovedMrtd::<T>::contains_key(&mrtd), Error::<T>::MrtdNotApproved);
			if let Some(ref mre) = mrenclave {
				ensure!(ApprovedMrenclave::<T>::contains_key(mre), Error::<T>::MrenclaveNotApproved);
			}

			let now = frame_system::Pallet::<T>::block_number();
			let expires_at = now.saturating_add(T::AttestationValidityBlocks::get());
			let is_dual = sgx_quote_hash.is_some() && mrenclave.is_some();

			let record = AttestationRecord::<T> {
				bot_id_hash,
				tdx_quote_hash,
				sgx_quote_hash,
				mrtd,
				mrenclave,
				attester: who,
				attested_at: now,
				expires_at,
				is_dual_attestation: is_dual,
				quote_verified: false,
			};
			Attestations::<T>::insert(&bot_id_hash, record);

			// 刷新 node_type
			let now_u64: u64 = now.unique_saturated_into();
			let expires_u64: u64 = expires_at.unique_saturated_into();
			let sgx_attested_at = if is_dual { Some(now_u64) } else { None };
			Bots::<T>::mutate(&bot_id_hash, |maybe_bot| {
				if let Some(bot) = maybe_bot {
					bot.node_type = NodeType::TeeNode {
						mrtd,
						mrenclave,
						tdx_attested_at: now_u64,
						sgx_attested_at,
						expires_at: expires_u64,
					};
				}
			});

			Self::deposit_event(Event::AttestationRefreshed { bot_id_hash });
			Ok(())
		}

		/// 审批 MRTD 到白名单
		#[pallet::call_index(8)]
		#[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
		pub fn approve_mrtd(
			origin: OriginFor<T>,
			mrtd: [u8; 48],
			version: u32,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(!ApprovedMrtd::<T>::contains_key(&mrtd), Error::<T>::MrtdAlreadyApproved);
			ApprovedMrtd::<T>::insert(&mrtd, version);
			Self::deposit_event(Event::MrtdApproved { mrtd, version });
			Ok(())
		}

		/// 提交经过 Quote 结构验证的 TEE 证明 (硬件节点专用)
		///
		/// MRTD 从原始 TDX Quote 字节中解析, 不由调用者提供。
		/// report_data[0..32] = SHA256(bot.public_key), report_data[32..64] = 链上 nonce
		/// 修改代码 → MRTD 改变 → 不在白名单 → 拒绝。
		/// 重放旧 Quote → nonce 不匹配 → 拒绝。
		#[pallet::call_index(10)]
		#[pallet::weight(Weight::from_parts(80_000_000, 20_000))]
		pub fn submit_verified_attestation(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			tdx_quote_raw: BoundedVec<u8, T::MaxQuoteLen>,
			sgx_quote_hash: Option<[u8; 32]>,
			mrenclave: Option<[u8; 32]>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let mut bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);

			// ── 解析 TDX Quote 结构 ──
			let quote = tdx_quote_raw.as_slice();
			ensure!(quote.len() >= TDX_MIN_QUOTE_LEN, Error::<T>::QuoteTooShort);

			// 提取 MRTD (48 bytes at offset 184)
			let mut mrtd = [0u8; 48];
			mrtd.copy_from_slice(&quote[TDX_MRTD_OFFSET..TDX_MRTD_OFFSET + TDX_MRTD_LEN]);

			// 提取 report_data (64 bytes at offset 568)
			let report_data = &quote[TDX_REPORTDATA_OFFSET..TDX_REPORTDATA_OFFSET + TDX_REPORTDATA_LEN];

			// ── 验证 report_data[0..32] == SHA256(bot.public_key) ──
			let expected_hash = sp_core::hashing::sha2_256(&bot.public_key);
			ensure!(report_data[..32] == expected_hash[..], Error::<T>::QuoteReportDataMismatch);

			// ── P2a: 验证 report_data[32..64] == 链上 nonce (防重放) ──
			let now = frame_system::Pallet::<T>::block_number();
			let (stored_nonce, issued_at) = AttestationNonces::<T>::get(&bot_id_hash)
				.ok_or(Error::<T>::NonceMissing)?;
			// Nonce 有效期 = AttestationValidityBlocks
			let nonce_deadline = issued_at.saturating_add(T::AttestationValidityBlocks::get());
			ensure!(now <= nonce_deadline, Error::<T>::NonceExpired);
			ensure!(report_data[32..64] == stored_nonce[..], Error::<T>::NonceMismatch);
			// 消费 nonce (一次性使用)
			AttestationNonces::<T>::remove(&bot_id_hash);

			// ── 检查 MRTD 白名单 ──
			ensure!(ApprovedMrtd::<T>::contains_key(&mrtd), Error::<T>::MrtdNotApproved);
			if let Some(ref mre) = mrenclave {
				ensure!(ApprovedMrenclave::<T>::contains_key(mre), Error::<T>::MrenclaveNotApproved);
			}

			let tdx_quote_hash = sp_core::hashing::blake2_256(quote);

			let expires_at = now.saturating_add(T::AttestationValidityBlocks::get());
			let is_dual = sgx_quote_hash.is_some() && mrenclave.is_some();

			let record = AttestationRecord::<T> {
				bot_id_hash,
				tdx_quote_hash,
				sgx_quote_hash,
				mrtd,
				mrenclave,
				attester: who,
				attested_at: now,
				expires_at,
				is_dual_attestation: is_dual,
				quote_verified: true,
			};
			Attestations::<T>::insert(&bot_id_hash, record);

			let now_u64: u64 = now.unique_saturated_into();
			let expires_u64: u64 = expires_at.unique_saturated_into();
			let sgx_attested_at = if is_dual { Some(now_u64) } else { None };
			bot.node_type = NodeType::TeeNode {
				mrtd,
				mrenclave,
				tdx_attested_at: now_u64,
				sgx_attested_at,
				expires_at: expires_u64,
			};
			Bots::<T>::insert(&bot_id_hash, bot);

			Self::deposit_event(Event::AttestationSubmitted { bot_id_hash, is_dual });
			Ok(())
		}

		/// 请求证明 Nonce (防重放)
		///
		/// Bot 在生成 TDX Quote 前调用, 获取链上 nonce。
		/// nonce 必须嵌入 TDX report_data[32..64], 随后提交 submit_verified_attestation。
		/// nonce = blake2_256(parent_hash || bot_id_hash || block_number)
		#[pallet::call_index(11)]
		#[pallet::weight(Weight::from_parts(25_000_000, 5_000))]
		pub fn request_attestation_nonce(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);

			let now = frame_system::Pallet::<T>::block_number();
			let parent_hash = <frame_system::Pallet<T>>::parent_hash();
			// nonce = blake2_256(parent_hash || bot_id_hash || block_number_le_bytes)
			let now_bytes: [u8; 8] = {
				let n: u64 = now.unique_saturated_into();
				n.to_le_bytes()
			};
			let mut preimage = alloc::vec::Vec::with_capacity(32 + 32 + 8);
			preimage.extend_from_slice(parent_hash.as_ref());
			preimage.extend_from_slice(&bot_id_hash);
			preimage.extend_from_slice(&now_bytes);
			let nonce = sp_core::hashing::blake2_256(&preimage);

			AttestationNonces::<T>::insert(&bot_id_hash, (nonce, now));
			Self::deposit_event(Event::NonceIssued { bot_id_hash, nonce });
			Ok(())
		}

		/// 审批 MRENCLAVE 到白名单
		#[pallet::call_index(9)]
		#[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
		pub fn approve_mrenclave(
			origin: OriginFor<T>,
			mrenclave: [u8; 32],
			version: u32,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(!ApprovedMrenclave::<T>::contains_key(&mrenclave), Error::<T>::MrenclaveAlreadyApproved);
			ApprovedMrenclave::<T>::insert(&mrenclave, version);
			Self::deposit_event(Event::MrenclaveApproved { mrenclave, version });
			Ok(())
		}
	}

	// ========================================================================
	// Helper Functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// Bot 是否已注册且活跃
		pub fn is_bot_active(bot_id_hash: &BotIdHash) -> bool {
			Bots::<T>::get(bot_id_hash)
				.map(|b| b.status == BotStatus::Active)
				.unwrap_or(false)
		}

		/// Bot 是否为 TEE 节点
		pub fn is_tee_node(bot_id_hash: &BotIdHash) -> bool {
			Bots::<T>::get(bot_id_hash)
				.map(|b| !matches!(b.node_type, NodeType::StandardNode))
				.unwrap_or(false)
		}

		/// Bot 是否有 SGX 双证明
		pub fn has_dual_attestation(bot_id_hash: &BotIdHash) -> bool {
			Attestations::<T>::get(bot_id_hash)
				.map(|a| a.is_dual_attestation)
				.unwrap_or(false)
		}

		/// TEE 证明是否在有效期内
		pub fn is_attestation_fresh(bot_id_hash: &BotIdHash) -> bool {
			let now = frame_system::Pallet::<T>::block_number();
			Attestations::<T>::get(bot_id_hash)
				.map(|a| now < a.expires_at)
				.unwrap_or(false)
		}

		/// 获取 Bot 所有者
		pub fn bot_owner(bot_id_hash: &BotIdHash) -> Option<T::AccountId> {
			Bots::<T>::get(bot_id_hash).map(|b| b.owner)
		}

		/// 获取 Bot 公钥
		pub fn bot_public_key(bot_id_hash: &BotIdHash) -> Option<[u8; 32]> {
			Bots::<T>::get(bot_id_hash).map(|b| b.public_key)
		}
	}

	// ========================================================================
	// BotRegistryProvider 实现
	// ========================================================================

	impl<T: Config> BotRegistryProvider<T::AccountId> for Pallet<T> {
		fn is_bot_active(bot_id_hash: &BotIdHash) -> bool {
			Self::is_bot_active(bot_id_hash)
		}
		fn is_tee_node(bot_id_hash: &BotIdHash) -> bool {
			Self::is_tee_node(bot_id_hash)
		}
		fn has_dual_attestation(bot_id_hash: &BotIdHash) -> bool {
			Self::has_dual_attestation(bot_id_hash)
		}
		fn is_attestation_fresh(bot_id_hash: &BotIdHash) -> bool {
			Self::is_attestation_fresh(bot_id_hash)
		}
		fn bot_owner(bot_id_hash: &BotIdHash) -> Option<T::AccountId> {
			Self::bot_owner(bot_id_hash)
		}
		fn bot_public_key(bot_id_hash: &BotIdHash) -> Option<[u8; 32]> {
			Self::bot_public_key(bot_id_hash)
		}
	}
}
