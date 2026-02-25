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

pub mod dcap;

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

/// Peer 端点信息 (用于节点发现)
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct PeerEndpoint<T: Config> {
	/// Peer 公钥 (Ed25519, 32 bytes)
	pub public_key: [u8; 32],
	/// 端点 URL (例如 https://node-b:8443)
	pub endpoint: BoundedVec<u8, T::MaxEndpointLen>,
	/// 注册区块
	pub registered_at: BlockNumberFor<T>,
	/// 最后心跳区块
	pub last_seen: BlockNumberFor<T>,
}

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
	/// DCAP 验证级别: 0=无, 2=Body签名+AK绑定, 3=+QE Report签名
	pub dcap_level: u8,
	/// API Server MRTD (双 Quote 证明, 可选)
	pub api_server_mrtd: Option<[u8; 48]>,
	/// API Server Quote 哈希 (双 Quote 证明, 可选)
	pub api_server_quote_hash: Option<[u8; 32]>,
}

/// TEE 证明记录 V2 (三模式统一)
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct AttestationRecordV2<T: Config> {
	pub bot_id_hash: BotIdHash,
	/// 主 Quote hash (TDX v4 或 SGX v3)
	pub primary_quote_hash: [u8; 32],
	/// 补充 Quote hash (TDX+SGX 双证明时的第二个 Quote)
	pub secondary_quote_hash: Option<[u8; 32]>,
	/// 统一度量值: MRTD(48B) 或 MRENCLAVE(32B + 16B zero-pad)
	pub primary_measurement: [u8; 48],
	/// SGX MRENCLAVE (原始 32B)
	pub mrenclave: Option<[u8; 32]>,
	/// TEE 类型
	pub tee_type: TeeType,
	pub attester: T::AccountId,
	pub attested_at: BlockNumberFor<T>,
	pub expires_at: BlockNumberFor<T>,
	pub is_dual_attestation: bool,
	pub quote_verified: bool,
	pub dcap_level: u8,
	pub api_server_mrtd: Option<[u8; 48]>,
	pub api_server_quote_hash: Option<[u8; 32]>,
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
		/// 单个 Bot 最大 Peer 数
		#[pallet::constant]
		type MaxPeersPerBot: Get<u32>;
		/// Peer 端点 URL 最大长度
		#[pallet::constant]
		type MaxEndpointLen: Get<u32>;
		/// Peer 心跳过期阈值 (区块数), 超过此时间未心跳则自动移除
		#[pallet::constant]
		type PeerHeartbeatTimeout: Get<BlockNumberFor<Self>>;
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

	/// 审批的 API Server MRTD 白名单 (双 Quote 证明)
	#[pallet::storage]
	pub type ApprovedApiServerMrtd<T: Config> =
		StorageMap<_, Blake2_128Concat, [u8; 48], u32>;

	/// 注册的 PCK 公钥: platform_id → (pck_pubkey_64bytes, registered_at_block)
	/// PCK (Provisioning Certification Key) 用于 DCAP Level 3 验证
	#[pallet::storage]
	pub type RegisteredPckKeys<T: Config> =
		StorageMap<_, Blake2_128Concat, [u8; 32], ([u8; 64], BlockNumberFor<T>)>;

	/// Peer 注册表: bot_id_hash → Vec<PeerEndpoint>
	/// 同一 Bot 的所有 TEE 节点端点, 用于节点发现和 share recovery
	#[pallet::storage]
	pub type PeerRegistry<T: Config> = StorageMap<
		_, Blake2_128Concat, BotIdHash,
		BoundedVec<PeerEndpoint<T>, T::MaxPeersPerBot>, ValueQuery,
	>;

	/// TEE 证明记录 V2 (三模式统一): bot_id_hash → AttestationRecordV2
	#[pallet::storage]
	pub type AttestationsV2<T: Config> =
		StorageMap<_, Blake2_128Concat, BotIdHash, AttestationRecordV2<T>>;

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
		ApiServerMrtdApproved { mrtd: [u8; 48], version: u32 },
		PckKeyRegistered { platform_id: [u8; 32] },
		DcapAttestationSubmitted { bot_id_hash: BotIdHash, dcap_level: u8, has_api_server: bool },
		PeerRegistered { bot_id_hash: BotIdHash, public_key: [u8; 32], peer_count: u32 },
		PeerDeregistered { bot_id_hash: BotIdHash, public_key: [u8; 32], peer_count: u32 },
		PeerHeartbeat { bot_id_hash: BotIdHash, public_key: [u8; 32] },
		PeerExpired { bot_id_hash: BotIdHash, public_key: [u8; 32], peer_count: u32 },
		SgxAttestationSubmitted { bot_id_hash: BotIdHash, sgx_dcap_level: u8 },
		TeeAttestationSubmitted { bot_id_hash: BotIdHash, tee_type: TeeType, dcap_level: u8 },
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
		// R1-fix: BotAlreadySuspended, BotNotSuspended 已移除 (无 extrinsic 使用)
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
		/// DCAP 验证失败: Quote 结构无效
		DcapQuoteInvalid,
		/// DCAP 验证失败: Body ECDSA 签名无效 (Quote 被篡改或伪造)
		DcapBodySignatureInvalid,
		/// DCAP 验证失败: Attestation Key 未绑定到 QE Report
		DcapAkBindingFailed,
		/// DCAP 验证失败: QE Report 签名无效
		DcapQeSignatureInvalid,
		/// PCK 公钥未注册 (DCAP Level 3 需要)
		PckKeyNotRegistered,
		/// API Server MRTD 未在白名单中
		ApiServerMrtdNotApproved,
		/// API Server MRTD 已在白名单中
		ApiServerMrtdAlreadyApproved,
		/// API Server Quote report_data 与 Bot 公钥不匹配
		ApiServerReportDataMismatch,
		/// DCAP 证书链验证失败 (DER 解析或签名验证)
		DcapCertChainInvalid,
		/// Intel Root CA 签名 Intermediate CA 验证失败
		DcapRootCaVerificationFailed,
		/// Intermediate CA 签名 PCK 证书验证失败
		DcapIntermediateCaVerificationFailed,
		/// Peer 已注册 (相同公钥)
		PeerAlreadyRegistered,
		/// Peer 不存在
		PeerNotFound,
		/// Peer 数量已满
		MaxPeersReached,
		/// 端点 URL 为空
		EndpointEmpty,
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

			// ── AttestationsV2 过期清理 ──
			let mut expired_v2: alloc::vec::Vec<BotIdHash> = alloc::vec::Vec::new();
			for (bot_id_hash, record) in AttestationsV2::<T>::iter() {
				reads += 1;
				if n >= record.expires_at {
					expired_v2.push(bot_id_hash);
				}
			}
			for bot_id_hash in expired_v2 {
				AttestationsV2::<T>::remove(&bot_id_hash);
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

			// ── Peer 心跳过期清理 ──
			let peer_timeout = T::PeerHeartbeatTimeout::get();
			if peer_timeout > BlockNumberFor::<T>::default() {
				let mut stale: alloc::vec::Vec<(BotIdHash, alloc::vec::Vec<[u8; 32]>)> = alloc::vec::Vec::new();
				for (bot_id_hash, peers) in PeerRegistry::<T>::iter() {
					reads += 1;
					let expired_pks: alloc::vec::Vec<[u8; 32]> = peers.iter()
						.filter(|p| n.saturating_sub(p.last_seen) > peer_timeout)
						.map(|p| p.public_key)
						.collect();
					if !expired_pks.is_empty() {
						stale.push((bot_id_hash, expired_pks));
					}
				}

				for (bot_id_hash, expired_pks) in stale {
					PeerRegistry::<T>::mutate(&bot_id_hash, |peers| {
						for pk in &expired_pks {
							if let Some(idx) = peers.iter().position(|p| &p.public_key == pk) {
								peers.swap_remove(idx);
								writes += 1;
								Self::deposit_event(Event::PeerExpired {
									bot_id_hash,
									public_key: *pk,
									peer_count: peers.len() as u32,
								});
							}
						}
					});
					reads += 1;
				}
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
				// H1-fix: 公钥变更后, 旧 attestation 的 report_data 绑定已失效
				// 必须重置为 StandardNode, 要求重新证明
				bot.node_type = NodeType::StandardNode;
				Ok(())
			})?;
			// H1-fix: 移除旧的 attestation 记录
			Attestations::<T>::remove(&bot_id_hash);
			AttestationNonces::<T>::remove(&bot_id_hash);
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
			ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);

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
				dcap_level: 0,
				api_server_mrtd: None,
				api_server_quote_hash: None,
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
			ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);
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
				dcap_level: 0,
				api_server_mrtd: None,
				api_server_quote_hash: None,
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

		/// 提交经过 Quote 结构验证的 TEE 证明
		///
		/// ⚠️ **安全级别低**: 此 extrinsic 仅解析 Quote 结构 + nonce 绑定,
		/// **不验证任何 ECDSA 签名**。攻击者可在非 TEE 环境下构造通过验证的 Quote。
		/// 生产环境应使用 `submit_dcap_attestation` (Level 3+) 或 `submit_dcap_full_attestation` (Level 4)。
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
			ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);

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
				quote_verified: false, // C1-fix: 无 ECDSA 签名验证, 不可信
				dcap_level: 1, // C1-fix: 仅结构解析, 非 DCAP 密码学验证
				api_server_mrtd: None,
				api_server_quote_hash: None,
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
			ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);

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

		/// 审批 API Server MRTD 到白名单 (双 Quote 证明)
		#[pallet::call_index(12)]
		#[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
		pub fn approve_api_server_mrtd(
			origin: OriginFor<T>,
			mrtd: [u8; 48],
			version: u32,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(
				!ApprovedApiServerMrtd::<T>::contains_key(&mrtd),
				Error::<T>::ApiServerMrtdAlreadyApproved
			);
			ApprovedApiServerMrtd::<T>::insert(&mrtd, version);
			Self::deposit_event(Event::ApiServerMrtdApproved { mrtd, version });
			Ok(())
		}

		/// 注册 PCK 公钥 (用于 DCAP Level 3 验证)
		///
		/// PCK (Provisioning Certification Key) 由 Intel 证书链认证。
		/// 通过治理注册到链上后，用于验证 QE Report 签名。
		#[pallet::call_index(13)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn register_pck_key(
			origin: OriginFor<T>,
			platform_id: [u8; 32],
			pck_public_key: [u8; 64],
		) -> DispatchResult {
			ensure_root(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			RegisteredPckKeys::<T>::insert(&platform_id, (pck_public_key, now));
			Self::deposit_event(Event::PckKeyRegistered { platform_id });
			Ok(())
		}

		/// 提交 DCAP 验证的 TEE 证明 (单 Quote, Level 2 或 Level 3)
		///
		/// 与 `submit_verified_attestation` 的区别:
		/// - 验证 ECDSA P-256 签名 (防止手工构造假 Quote)
		/// - 验证 Attestation Key 绑定到 QE Report
		/// - 如果提供 platform_id, 额外验证 QE Report 签名 (Level 3)
		///
		/// 修改代码 → MRTD 改变 → 签名不匹配 → 拒绝
		/// 手工构造 Quote → AK 私钥不在 CPU 外 → 签名无效 → 拒绝
		#[pallet::call_index(14)]
		#[pallet::weight(Weight::from_parts(200_000_000, 30_000))]
		pub fn submit_dcap_attestation(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			tdx_quote_raw: BoundedVec<u8, T::MaxQuoteLen>,
			mrenclave: Option<[u8; 32]>,
			platform_id: Option<[u8; 32]>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let mut bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);
			ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);

			let quote = tdx_quote_raw.as_slice();

			// ── DCAP 验证 ──
			let (dcap_result, dcap_level) = if let Some(ref pid) = platform_id {
				// Level 3: Body sig + AK binding + QE Report sig
				let (pck_key, _) = RegisteredPckKeys::<T>::get(pid)
					.ok_or(Error::<T>::PckKeyNotRegistered)?;
				let result = dcap::verify_quote_level3(quote, &pck_key)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 3u8)
			} else {
				// Level 2: Body sig + AK binding
				let result = dcap::verify_quote_level2(quote)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 2u8)
			};

			let mrtd = dcap_result.mrtd;
			let report_data = dcap_result.report_data;

			// ── report_data[0..32] == SHA256(bot.public_key) ──
			let expected_hash = sp_core::hashing::sha2_256(&bot.public_key);
			ensure!(report_data[..32] == expected_hash[..], Error::<T>::QuoteReportDataMismatch);

			// ── Nonce 验证 (防重放) ──
			let now = frame_system::Pallet::<T>::block_number();
			let (stored_nonce, issued_at) = AttestationNonces::<T>::get(&bot_id_hash)
				.ok_or(Error::<T>::NonceMissing)?;
			let nonce_deadline = issued_at.saturating_add(T::AttestationValidityBlocks::get());
			ensure!(now <= nonce_deadline, Error::<T>::NonceExpired);
			ensure!(report_data[32..64] == stored_nonce[..], Error::<T>::NonceMismatch);
			AttestationNonces::<T>::remove(&bot_id_hash);

			// ── MRTD 白名单 ──
			ensure!(ApprovedMrtd::<T>::contains_key(&mrtd), Error::<T>::MrtdNotApproved);
			if let Some(ref mre) = mrenclave {
				ensure!(ApprovedMrenclave::<T>::contains_key(mre), Error::<T>::MrenclaveNotApproved);
			}

			let tdx_quote_hash = sp_core::hashing::blake2_256(quote);
			let expires_at = now.saturating_add(T::AttestationValidityBlocks::get());

			// X1-fix: Level 2 攻击者控制 AK 私钥, ECDSA 验证无意义 → quote_verified=false
			// Level 3+ PCK 私钥不可获取 → quote_verified=true
			let quote_verified = dcap_level >= 3;
			let record = AttestationRecord::<T> {
				bot_id_hash,
				tdx_quote_hash,
				sgx_quote_hash: None,
				mrtd,
				mrenclave,
				attester: who,
				attested_at: now,
				expires_at,
				is_dual_attestation: false,
				quote_verified,
				dcap_level,
				api_server_mrtd: None,
				api_server_quote_hash: None,
			};
			Attestations::<T>::insert(&bot_id_hash, record);

			let now_u64: u64 = now.unique_saturated_into();
			let expires_u64: u64 = expires_at.unique_saturated_into();
			bot.node_type = NodeType::TeeNode {
				mrtd,
				mrenclave,
				tdx_attested_at: now_u64,
				sgx_attested_at: None,
				expires_at: expires_u64,
			};
			Bots::<T>::insert(&bot_id_hash, bot);

			Self::deposit_event(Event::DcapAttestationSubmitted {
				bot_id_hash, dcap_level, has_api_server: false,
			});
			Ok(())
		}

		/// 提交 DCAP 双 Quote 证明 (GroupRobot + API Server)
		///
		/// 同时验证两个 TDX Quote:
		/// 1. Bot Quote: MRTD 在 ApprovedMrtd 白名单
		/// 2. API Server Quote: MRTD 在 ApprovedApiServerMrtd 白名单
		///
		/// 两个 Quote 的 report_data[0..32] 必须都绑定到同一个 bot.public_key,
		/// 证明两个进程运行在同一节点上。
		#[pallet::call_index(15)]
		#[pallet::weight(Weight::from_parts(350_000_000, 50_000))]
		pub fn submit_dcap_dual_attestation(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			bot_quote_raw: BoundedVec<u8, T::MaxQuoteLen>,
			api_server_quote_raw: BoundedVec<u8, T::MaxQuoteLen>,
			mrenclave: Option<[u8; 32]>,
			platform_id: Option<[u8; 32]>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let mut bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);
			ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);

			let bot_quote = bot_quote_raw.as_slice();
			let api_quote = api_server_quote_raw.as_slice();

			// ── DCAP 验证 Bot Quote ──
			let (bot_result, dcap_level) = if let Some(ref pid) = platform_id {
				let (pck_key, _) = RegisteredPckKeys::<T>::get(pid)
					.ok_or(Error::<T>::PckKeyNotRegistered)?;
				let result = dcap::verify_quote_level3(bot_quote, &pck_key)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 3u8)
			} else {
				let result = dcap::verify_quote_level2(bot_quote)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 2u8)
			};

			// ── DCAP 验证 API Server Quote ──
			let api_result = if let Some(ref pid) = platform_id {
				let (pck_key, _) = RegisteredPckKeys::<T>::get(pid)
					.ok_or(Error::<T>::PckKeyNotRegistered)?;
				dcap::verify_quote_level3(api_quote, &pck_key)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?
			} else {
				dcap::verify_quote_level2(api_quote)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?
			};

			let mrtd = bot_result.mrtd;
			let report_data = bot_result.report_data;
			let api_server_mrtd = api_result.mrtd;

			// ── Bot Quote: report_data 绑定 ──
			let expected_hash = sp_core::hashing::sha2_256(&bot.public_key);
			ensure!(report_data[..32] == expected_hash[..], Error::<T>::QuoteReportDataMismatch);

			// ── API Server Quote: report_data 也必须绑定到同一 public_key ──
			ensure!(
				api_result.report_data[..32] == expected_hash[..],
				Error::<T>::ApiServerReportDataMismatch
			);

			// ── Nonce 验证 (Bot Quote) ──
			let now = frame_system::Pallet::<T>::block_number();
			let (stored_nonce, issued_at) = AttestationNonces::<T>::get(&bot_id_hash)
				.ok_or(Error::<T>::NonceMissing)?;
			let nonce_deadline = issued_at.saturating_add(T::AttestationValidityBlocks::get());
			ensure!(now <= nonce_deadline, Error::<T>::NonceExpired);
			ensure!(report_data[32..64] == stored_nonce[..], Error::<T>::NonceMismatch);
			AttestationNonces::<T>::remove(&bot_id_hash);

			// ── MRTD 白名单 ──
			ensure!(ApprovedMrtd::<T>::contains_key(&mrtd), Error::<T>::MrtdNotApproved);
			ensure!(
				ApprovedApiServerMrtd::<T>::contains_key(&api_server_mrtd),
				Error::<T>::ApiServerMrtdNotApproved
			);
			if let Some(ref mre) = mrenclave {
				ensure!(ApprovedMrenclave::<T>::contains_key(mre), Error::<T>::MrenclaveNotApproved);
			}

			let tdx_quote_hash = sp_core::hashing::blake2_256(bot_quote);
			let api_server_quote_hash = sp_core::hashing::blake2_256(api_quote);
			let expires_at = now.saturating_add(T::AttestationValidityBlocks::get());

			// X1-fix: 同单 Quote 逻辑, Level 2 不可信
			let quote_verified = dcap_level >= 3;
			let record = AttestationRecord::<T> {
				bot_id_hash,
				tdx_quote_hash,
				sgx_quote_hash: None,
				mrtd,
				mrenclave,
				attester: who,
				attested_at: now,
				expires_at,
				is_dual_attestation: true,
				quote_verified,
				dcap_level,
				api_server_mrtd: Some(api_server_mrtd),
				api_server_quote_hash: Some(api_server_quote_hash),
			};
			Attestations::<T>::insert(&bot_id_hash, record);

			let now_u64: u64 = now.unique_saturated_into();
			let expires_u64: u64 = expires_at.unique_saturated_into();
			bot.node_type = NodeType::TeeNode {
				mrtd,
				mrenclave,
				tdx_attested_at: now_u64,
				sgx_attested_at: None,
				expires_at: expires_u64,
			};
			Bots::<T>::insert(&bot_id_hash, bot);

			Self::deposit_event(Event::DcapAttestationSubmitted {
				bot_id_hash, dcap_level, has_api_server: true,
			});
			Ok(())
		}

		/// 提交 DCAP Level 4 证明 (完整证书链验证)
		///
		/// 与 Level 3 的区别:
		/// - Level 3: 信任治理注册的 PCK 公钥
		/// - Level 4: 通过 Intel Root CA 证书链验证 PCK 公钥的合法性
		///
		/// 提交者需提供:
		/// - TDX Quote 原始字节
		/// - PCK 证书 (DER 编码)
		/// - Intermediate CA 证书 (DER 编码)
		///
		/// 链上验证:
		/// ```text
		/// Intel Root CA (硬编码) → Intermediate CA → PCK → QE Report → AK → Body
		/// ```
		#[pallet::call_index(16)]
		#[pallet::weight(Weight::from_parts(300_000_000, 40_000))]
		pub fn submit_dcap_full_attestation(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			tdx_quote_raw: BoundedVec<u8, T::MaxQuoteLen>,
			pck_cert_der: BoundedVec<u8, T::MaxQuoteLen>,
			intermediate_cert_der: BoundedVec<u8, T::MaxQuoteLen>,
			mrenclave: Option<[u8; 32]>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let mut bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);
			ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);

			let quote = tdx_quote_raw.as_slice();

			// ── DCAP Level 4 验证: 证书链 + Quote ──
			let dcap_result = dcap::verify_quote_with_cert_chain(
				quote,
				pck_cert_der.as_slice(),
				intermediate_cert_der.as_slice(),
			)
			.map_err(|e| Self::dcap_error_to_dispatch(e))?;

			let dcap_level = 4u8;
			let mrtd = dcap_result.mrtd;
			let report_data = dcap_result.report_data;

			// ── report_data[0..32] == SHA256(bot.public_key) ──
			let expected_hash = sp_core::hashing::sha2_256(&bot.public_key);
			ensure!(report_data[..32] == expected_hash[..], Error::<T>::QuoteReportDataMismatch);

			// ── Nonce 验证 (防重放) ──
			let now = frame_system::Pallet::<T>::block_number();
			let (stored_nonce, issued_at) = AttestationNonces::<T>::get(&bot_id_hash)
				.ok_or(Error::<T>::NonceMissing)?;
			let nonce_deadline = issued_at.saturating_add(T::AttestationValidityBlocks::get());
			ensure!(now <= nonce_deadline, Error::<T>::NonceExpired);
			ensure!(report_data[32..64] == stored_nonce[..], Error::<T>::NonceMismatch);
			AttestationNonces::<T>::remove(&bot_id_hash);

			// ── MRTD 白名单 ──
			ensure!(ApprovedMrtd::<T>::contains_key(&mrtd), Error::<T>::MrtdNotApproved);
			if let Some(ref mre) = mrenclave {
				ensure!(ApprovedMrenclave::<T>::contains_key(mre), Error::<T>::MrenclaveNotApproved);
			}

			let tdx_quote_hash = sp_core::hashing::blake2_256(quote);
			let expires_at = now.saturating_add(T::AttestationValidityBlocks::get());

			let record = AttestationRecord::<T> {
				bot_id_hash,
				tdx_quote_hash,
				sgx_quote_hash: None,
				mrtd,
				mrenclave,
				attester: who,
				attested_at: now,
				expires_at,
				is_dual_attestation: false,
				quote_verified: true,
				dcap_level,
				api_server_mrtd: None,
				api_server_quote_hash: None,
			};
			Attestations::<T>::insert(&bot_id_hash, record);

			let now_u64: u64 = now.unique_saturated_into();
			let expires_u64: u64 = expires_at.unique_saturated_into();
			bot.node_type = NodeType::TeeNode {
				mrtd,
				mrenclave,
				tdx_attested_at: now_u64,
				sgx_attested_at: None,
				expires_at: expires_u64,
			};
			Bots::<T>::insert(&bot_id_hash, bot);

			Self::deposit_event(Event::DcapAttestationSubmitted {
				bot_id_hash, dcap_level, has_api_server: false,
			});
			Ok(())
		}

		/// 提交 SGX Enclave DCAP 证明 (补充现有 TDX 证明)
		///
		/// 要求: Bot 必须已有有效的 TDX 证明 (AttestationRecord)。
		/// 此 extrinsic 验证 SGX Quote v3 并将 MRENCLAVE 写入现有证明记录,
		/// 使 `is_dual_attestation = true`, 从而获得 `SgxEnclaveBonus` 奖励。
		///
		/// 验证内容:
		/// 1. SGX Quote v3 结构解析 + ECDSA 签名验证 (Level 2+)
		/// 2. report_data[0..32] == SHA256(bot.public_key)
		/// 3. MRENCLAVE 在 ApprovedMrenclave 白名单中
		/// 4. 可选: PCK 签名验证 (Level 3) 或证书链验证 (Level 4)
		#[pallet::call_index(20)]
		#[pallet::weight(Weight::from_parts(250_000_000, 35_000))]
		pub fn submit_sgx_attestation(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			sgx_quote_raw: BoundedVec<u8, T::MaxQuoteLen>,
			platform_id: Option<[u8; 32]>,
			pck_cert_der: Option<BoundedVec<u8, T::MaxQuoteLen>>,
			intermediate_cert_der: Option<BoundedVec<u8, T::MaxQuoteLen>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);
			ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);

			// 必须已有 TDX 证明
			let mut record = Attestations::<T>::get(&bot_id_hash)
				.ok_or(Error::<T>::AttestationNotFound)?;

			let quote = sgx_quote_raw.as_slice();

			// ── SGX DCAP 验证 ──
			let (sgx_result, sgx_dcap_level) = if let (Some(ref pck_der), Some(ref inter_der)) =
				(&pck_cert_der, &intermediate_cert_der)
			{
				// Level 4: 证书链验证
				let result = dcap::verify_sgx_quote_with_cert_chain(
					quote,
					pck_der.as_slice(),
					inter_der.as_slice(),
				)
				.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 4u8)
			} else if let Some(ref pid) = platform_id {
				// Level 3: PCK 公钥验证
				let (pck_key, _) = RegisteredPckKeys::<T>::get(pid)
					.ok_or(Error::<T>::PckKeyNotRegistered)?;
				let result = dcap::verify_sgx_quote_level3(quote, &pck_key)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 3u8)
			} else {
				// Level 2: Body 签名 + AK 绑定
				let result = dcap::verify_sgx_quote_level2(quote)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 2u8)
			};

			let mrenclave = sgx_result.mrenclave;
			let sgx_report_data = sgx_result.report_data;

			// ── report_data[0..32] == SHA256(bot.public_key) ──
			let expected_hash = sp_core::hashing::sha2_256(&bot.public_key);
			ensure!(sgx_report_data[..32] == expected_hash[..], Error::<T>::QuoteReportDataMismatch);

			// ── MRENCLAVE 白名单检查 ──
			ensure!(ApprovedMrenclave::<T>::contains_key(&mrenclave), Error::<T>::MrenclaveNotApproved);

			// ── 更新现有 AttestationRecord ──
			let sgx_quote_hash = sp_core::hashing::blake2_256(quote);
			record.mrenclave = Some(mrenclave);
			record.sgx_quote_hash = Some(sgx_quote_hash);
			record.is_dual_attestation = true;
			Attestations::<T>::insert(&bot_id_hash, record);

			// ── 更新 BotInfo.node_type 的 mrenclave + sgx_attested_at ──
			let now = frame_system::Pallet::<T>::block_number();
			let now_u64: u64 = now.unique_saturated_into();
			Bots::<T>::mutate(&bot_id_hash, |maybe_bot| {
				if let Some(bot) = maybe_bot {
					if let NodeType::TeeNode { ref mrtd, mrenclave: _, tdx_attested_at, sgx_attested_at: _, expires_at } = bot.node_type {
						bot.node_type = NodeType::TeeNode {
							mrtd: *mrtd,
							mrenclave: Some(mrenclave),
							tdx_attested_at,
							sgx_attested_at: Some(now_u64),
							expires_at,
						};
					}
				}
			});

			Self::deposit_event(Event::SgxAttestationSubmitted { bot_id_hash, sgx_dcap_level });
			Ok(())
		}

		/// 提交 TEE 证明 (SGX v3 / TDX v4 自动检测, 三模式统一入口)
		///
		/// 自动检测 Quote 类型 (version 字段):
		/// - version=3 → SGX Quote v3 → 提取 MRENCLAVE
		/// - version=4 → TDX Quote v4 → 提取 MRTD
		///
		/// DCAP 验证级别:
		/// - 无 platform_id, 无 certs → Level 2 (Body sig + AK binding)
		/// - 有 platform_id          → Level 3 (+ QE Report sig via PCK)
		/// - 有 certs                → Level 4 (+ Intel Root CA cert chain)
		#[pallet::call_index(21)]
		#[pallet::weight(Weight::from_parts(250_000_000, 40_000))]
		pub fn submit_tee_attestation(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			quote_raw: BoundedVec<u8, T::MaxQuoteLen>,
			platform_id: Option<[u8; 32]>,
			pck_cert_der: Option<BoundedVec<u8, T::MaxQuoteLen>>,
			intermediate_cert_der: Option<BoundedVec<u8, T::MaxQuoteLen>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let mut bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);
			ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);

			let quote = quote_raw.as_slice();
			ensure!(quote.len() >= 48, Error::<T>::QuoteTooShort);

			// ── Step 1: 自动检测 Quote 类型 ──
			let version = u16::from_le_bytes([quote[0], quote[1]]);
			let (tee_type, primary_measurement, mrenclave, report_data, dcap_level, quote_verified)
				= match version {
				4 => Self::verify_tdx_quote_unified(quote, &platform_id, &pck_cert_der, &intermediate_cert_der)?,
				3 => Self::verify_sgx_quote_unified(quote, &platform_id, &pck_cert_der, &intermediate_cert_der)?,
				_ => return Err(Error::<T>::DcapQuoteInvalid.into()),
			};

			// ── Step 2: report_data binding ──
			let expected_hash = sp_core::hashing::sha2_256(&bot.public_key);
			ensure!(report_data[..32] == expected_hash[..], Error::<T>::QuoteReportDataMismatch);

			// ── Step 3: Nonce 验证 (防重放) ──
			let now = frame_system::Pallet::<T>::block_number();
			let (stored_nonce, issued_at) = AttestationNonces::<T>::get(&bot_id_hash)
				.ok_or(Error::<T>::NonceMissing)?;
			let nonce_deadline = issued_at.saturating_add(T::AttestationValidityBlocks::get());
			ensure!(now <= nonce_deadline, Error::<T>::NonceExpired);
			ensure!(report_data[32..64] == stored_nonce[..], Error::<T>::NonceMismatch);
			AttestationNonces::<T>::remove(&bot_id_hash);

			// ── Step 4: 白名单检查 ──
			Self::check_measurement_approved(&tee_type, &primary_measurement, &mrenclave)?;

			// ── Step 5: 写入 AttestationRecordV2 ──
			let quote_hash = sp_core::hashing::blake2_256(quote);
			let expires_at = now.saturating_add(T::AttestationValidityBlocks::get());

			let record = AttestationRecordV2::<T> {
				bot_id_hash,
				primary_quote_hash: quote_hash,
				secondary_quote_hash: None,
				primary_measurement,
				mrenclave,
				tee_type,
				attester: who,
				attested_at: now,
				expires_at,
				is_dual_attestation: matches!(tee_type, TeeType::TdxPlusSgx),
				quote_verified,
				dcap_level,
				api_server_mrtd: None,
				api_server_quote_hash: None,
			};
			AttestationsV2::<T>::insert(&bot_id_hash, record);

			// ── Step 6: 更新 NodeType → TeeNodeV2 ──
			let now_u64: u64 = now.unique_saturated_into();
			let expires_u64: u64 = expires_at.unique_saturated_into();
			bot.node_type = NodeType::TeeNodeV2 {
				primary_measurement,
				tee_type,
				mrenclave,
				attested_at: now_u64,
				sgx_attested_at: None,
				expires_at: expires_u64,
			};
			Bots::<T>::insert(&bot_id_hash, bot);

			Self::deposit_event(Event::TeeAttestationSubmitted {
				bot_id_hash,
				tee_type,
				dcap_level,
			});
			Ok(())
		}

		/// 注册 Peer 端点 (TEE 节点启动时调用)
		///
		/// 将本节点的公钥和端点 URL 注册到链上, 供其他节点发现。
		/// 调用者必须是 Bot 所有者, Bot 必须已有 TEE 证明。
		#[pallet::call_index(17)]
		#[pallet::weight(Weight::from_parts(40_000_000, 8_000))]
		pub fn register_peer(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			peer_public_key: [u8; 32],
			endpoint: BoundedVec<u8, T::MaxEndpointLen>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);
			ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);
			ensure!(!endpoint.is_empty(), Error::<T>::EndpointEmpty);

			PeerRegistry::<T>::try_mutate(&bot_id_hash, |peers| -> DispatchResult {
				// 检查是否已注册 (相同公钥)
				ensure!(
					!peers.iter().any(|p| p.public_key == peer_public_key),
					Error::<T>::PeerAlreadyRegistered
				);

				let now = frame_system::Pallet::<T>::block_number();
				let peer = PeerEndpoint::<T> {
					public_key: peer_public_key,
					endpoint,
					registered_at: now,
					last_seen: now,
				};
				peers.try_push(peer).map_err(|_| Error::<T>::MaxPeersReached)?;

				Self::deposit_event(Event::PeerRegistered {
					bot_id_hash,
					public_key: peer_public_key,
					peer_count: peers.len() as u32,
				});
				Ok(())
			})
		}

		/// 注销 Peer 端点 (节点下线时调用)
		#[pallet::call_index(18)]
		#[pallet::weight(Weight::from_parts(35_000_000, 6_000))]
		pub fn deregister_peer(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			peer_public_key: [u8; 32],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);

			PeerRegistry::<T>::try_mutate(&bot_id_hash, |peers| -> DispatchResult {
				let idx = peers.iter().position(|p| p.public_key == peer_public_key)
					.ok_or(Error::<T>::PeerNotFound)?;
				peers.swap_remove(idx);

				Self::deposit_event(Event::PeerDeregistered {
					bot_id_hash,
					public_key: peer_public_key,
					peer_count: peers.len() as u32,
				});
				Ok(())
			})
		}

		/// Peer 心跳 (定期调用, 更新 last_seen)
		#[pallet::call_index(19)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn heartbeat_peer(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			peer_public_key: [u8; 32],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
			ensure!(bot.owner == who, Error::<T>::NotBotOwner);

			PeerRegistry::<T>::try_mutate(&bot_id_hash, |peers| -> DispatchResult {
				let peer = peers.iter_mut().find(|p| p.public_key == peer_public_key)
					.ok_or(Error::<T>::PeerNotFound)?;
				peer.last_seen = frame_system::Pallet::<T>::block_number();

				Self::deposit_event(Event::PeerHeartbeat {
					bot_id_hash,
					public_key: peer_public_key,
				});
				Ok(())
			})
		}
	}

	// ========================================================================
	// Helper Functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// 将 DCAP 错误转换为 DispatchError
		fn dcap_error_to_dispatch(e: dcap::DcapError) -> DispatchError {
			match e {
				dcap::DcapError::QuoteTooShort
				| dcap::DcapError::InvalidVersion
				| dcap::DcapError::InvalidAttKeyType
				| dcap::DcapError::InvalidTeeType
				| dcap::DcapError::InvalidVendorId
				| dcap::DcapError::InvalidSigDataLen
				| dcap::DcapError::InvalidPublicKey
				| dcap::DcapError::InvalidSignature => Error::<T>::DcapQuoteInvalid.into(),
				dcap::DcapError::BodySignatureInvalid => Error::<T>::DcapBodySignatureInvalid.into(),
				dcap::DcapError::AttestationKeyBindingFailed => Error::<T>::DcapAkBindingFailed.into(),
					dcap::DcapError::QeReportSignatureInvalid => Error::<T>::DcapQeSignatureInvalid.into(),
				dcap::DcapError::CertParsingFailed
				| dcap::DcapError::CertSignatureInvalid
				| dcap::DcapError::CertChainInvalid => Error::<T>::DcapCertChainInvalid.into(),
				dcap::DcapError::RootCaVerificationFailed => Error::<T>::DcapRootCaVerificationFailed.into(),
				dcap::DcapError::IntermediateCaVerificationFailed => Error::<T>::DcapIntermediateCaVerificationFailed.into(),
			}
		}

		/// TDX Quote v4 DCAP 验证 (统一入口用)
		fn verify_tdx_quote_unified(
			quote: &[u8],
			platform_id: &Option<[u8; 32]>,
			pck_cert_der: &Option<BoundedVec<u8, T::MaxQuoteLen>>,
			intermediate_cert_der: &Option<BoundedVec<u8, T::MaxQuoteLen>>,
		) -> Result<(TeeType, [u8; 48], Option<[u8; 32]>, [u8; 64], u8, bool), DispatchError> {
			let (dcap_result, dcap_level) = if let (Some(pck), Some(inter)) =
				(pck_cert_der, intermediate_cert_der)
			{
				let result = dcap::verify_quote_with_cert_chain(quote, pck.as_slice(), inter.as_slice())
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 4u8)
			} else if let Some(ref pid) = platform_id {
				let (pck_key, _) = RegisteredPckKeys::<T>::get(pid)
					.ok_or(Error::<T>::PckKeyNotRegistered)?;
				let result = dcap::verify_quote_level3(quote, &pck_key)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 3u8)
			} else {
				let result = dcap::verify_quote_level2(quote)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 2u8)
			};
			let quote_verified = dcap_level >= 3;
			Ok((TeeType::Tdx, dcap_result.mrtd, None, dcap_result.report_data, dcap_level, quote_verified))
		}

		/// SGX Quote v3 DCAP 验证 (统一入口用)
		fn verify_sgx_quote_unified(
			quote: &[u8],
			platform_id: &Option<[u8; 32]>,
			pck_cert_der: &Option<BoundedVec<u8, T::MaxQuoteLen>>,
			intermediate_cert_der: &Option<BoundedVec<u8, T::MaxQuoteLen>>,
		) -> Result<(TeeType, [u8; 48], Option<[u8; 32]>, [u8; 64], u8, bool), DispatchError> {
			let (sgx_result, dcap_level) = if let (Some(pck), Some(inter)) =
				(pck_cert_der, intermediate_cert_der)
			{
				let result = dcap::verify_sgx_quote_with_cert_chain(quote, pck.as_slice(), inter.as_slice())
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 4u8)
			} else if let Some(ref pid) = platform_id {
				let (pck_key, _) = RegisteredPckKeys::<T>::get(pid)
					.ok_or(Error::<T>::PckKeyNotRegistered)?;
				let result = dcap::verify_sgx_quote_level3(quote, &pck_key)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 3u8)
			} else {
				let result = dcap::verify_sgx_quote_level2(quote)
					.map_err(|e| Self::dcap_error_to_dispatch(e))?;
				(result, 2u8)
			};
			// MRENCLAVE → padded to 48B as primary_measurement
			let mut primary = [0u8; 48];
			primary[..32].copy_from_slice(&sgx_result.mrenclave);
			let mrenclave = Some(sgx_result.mrenclave);
			let quote_verified = dcap_level >= 3;
			Ok((TeeType::Sgx, primary, mrenclave, sgx_result.report_data, dcap_level, quote_verified))
		}

		/// 根据 TEE 类型检查度量值白名单
		fn check_measurement_approved(
			tee_type: &TeeType,
			primary_measurement: &[u8; 48],
			mrenclave: &Option<[u8; 32]>,
		) -> DispatchResult {
			match tee_type {
				TeeType::Tdx | TeeType::TdxPlusSgx => {
					ensure!(
						ApprovedMrtd::<T>::contains_key(primary_measurement),
						Error::<T>::MrtdNotApproved
					);
				}
				TeeType::Sgx => {
					let mut mre = [0u8; 32];
					mre.copy_from_slice(&primary_measurement[..32]);
					ensure!(
						ApprovedMrenclave::<T>::contains_key(&mre),
						Error::<T>::MrenclaveNotApproved
					);
				}
			}
			if let Some(ref mre) = mrenclave {
				if matches!(tee_type, TeeType::TdxPlusSgx) {
					ensure!(
						ApprovedMrenclave::<T>::contains_key(mre),
						Error::<T>::MrenclaveNotApproved
					);
				}
			}
			Ok(())
		}

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
			if let Some(v2) = AttestationsV2::<T>::get(bot_id_hash) {
				return v2.is_dual_attestation;
			}
			Attestations::<T>::get(bot_id_hash)
				.map(|a| a.is_dual_attestation)
				.unwrap_or(false)
		}

		/// TEE 证明是否在有效期内
		pub fn is_attestation_fresh(bot_id_hash: &BotIdHash) -> bool {
			let now = frame_system::Pallet::<T>::block_number();
			if let Some(v2) = AttestationsV2::<T>::get(bot_id_hash) {
				return now < v2.expires_at;
			}
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

		/// 获取 Bot 的 Peer 数量
		pub fn peer_count(bot_id_hash: &BotIdHash) -> u32 {
			PeerRegistry::<T>::get(bot_id_hash).len() as u32
		}

		/// 获取 Bot 的所有 Peer 端点
		pub fn get_peers(bot_id_hash: &BotIdHash) -> BoundedVec<PeerEndpoint<T>, T::MaxPeersPerBot> {
			PeerRegistry::<T>::get(bot_id_hash)
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
		fn peer_count(bot_id_hash: &BotIdHash) -> u32 {
			Self::peer_count(bot_id_hash)
		}
	}
}
