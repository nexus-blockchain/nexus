#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet GroupRobot Ceremony — RA-TLS 仪式审计 + Enclave 白名单 + 自动风险检测
//!
//! 整合现有 `pallet-ceremony-audit` + `ceremony.rs` 链上部分。
//!
//! ## 功能
//! - 记录仪式 (验证 Enclave 白名单 + Shamir 参数)
//! - 撤销仪式
//! - 强制 re-ceremony
//! - Ceremony Enclave 白名单管理
//! - on_initialize: 仪式过期 + 风险检测

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

/// 仪式记录
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct CeremonyRecord<T: Config> {
	pub ceremony_mrenclave: [u8; 32],
	pub k: u8,
	pub n: u8,
	pub bot_public_key: [u8; 32],
	pub participant_count: u8,
	pub participant_enclaves: BoundedVec<[u8; 32], T::MaxParticipants>,
	pub initiator: T::AccountId,
	pub created_at: BlockNumberFor<T>,
	pub status: CeremonyStatus,
	pub expires_at: BlockNumberFor<T>,
	/// 是否为 Re-ceremony (而非首次 Ceremony)
	pub is_re_ceremony: bool,
	/// 替代的旧仪式哈希 (Re-ceremony 时填写)
	pub supersedes: Option<[u8; 32]>,
	/// C1-fix: 存储 bot_id_hash 以供 on_initialize 使用 (避免哈希函数不一致)
	pub bot_id_hash: [u8; 32],
}

/// Ceremony Enclave 信息
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct CeremonyEnclaveInfo {
	pub version: u32,
	pub approved_at: u64,
	pub description: BoundedVec<u8, ConstU32<128>>,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// 最大参与节点数
		#[pallet::constant]
		type MaxParticipants: Get<u32>;
		/// 仪式历史最大数
		#[pallet::constant]
		type MaxCeremonyHistory: Get<u32>;
		/// 仪式有效期 (区块数)
		#[pallet::constant]
		type CeremonyValidityBlocks: Get<BlockNumberFor<Self>>;
		/// 仪式检查间隔 (区块数)
		#[pallet::constant]
		type CeremonyCheckInterval: Get<BlockNumberFor<Self>>;
		/// L2-R3: 每次 on_initialize 最多处理的仪式数
		#[pallet::constant]
		type MaxProcessPerBlock: Get<u32>;
		/// Bot 注册查询
		type BotRegistry: BotRegistryProvider<Self::AccountId>;
		/// 订阅层级查询 (tier gate)
		type Subscription: SubscriptionProvider;
	}

	// ========================================================================
	// Storage
	// ========================================================================

	/// 仪式记录: ceremony_hash → CeremonyRecord
	#[pallet::storage]
	pub type Ceremonies<T: Config> =
		StorageMap<_, Blake2_128Concat, [u8; 32], CeremonyRecord<T>>;

	/// Bot 公钥 → 当前活跃仪式哈希
	#[pallet::storage]
	pub type ActiveCeremony<T: Config> =
		StorageMap<_, Blake2_128Concat, [u8; 32], [u8; 32]>;

	/// 仪式历史: bot_public_key → Vec<ceremony_hash>
	#[pallet::storage]
	pub type CeremonyHistory<T: Config> = StorageMap<
		_, Blake2_128Concat, [u8; 32],
		BoundedVec<[u8; 32], T::MaxCeremonyHistory>, ValueQuery,
	>;

	/// 审批的 Ceremony Enclave: mrenclave → CeremonyEnclaveInfo
	#[pallet::storage]
	pub type ApprovedEnclaves<T: Config> =
		StorageMap<_, Blake2_128Concat, [u8; 32], CeremonyEnclaveInfo>;

	/// 仪式总数
	#[pallet::storage]
	pub type CeremonyCount<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// L2-R3: 按 expires_at 排序的过期队列 (expires_at, bot_pk, ceremony_hash)
	/// record_ceremony 时插入，过期/撤销/替代时移除
	#[pallet::storage]
	pub type ExpiryQueue<T: Config> = StorageValue<
		_,
		BoundedVec<(BlockNumberFor<T>, [u8; 32], [u8; 32]), ConstU32<1000>>,
		ValueQuery,
	>;

	/// L2-R3: AtRisk 检测游标 — 上次处理到的 ActiveCeremony 位置
	/// 存储 last processed bot_pk，下次从此处继续
	#[pallet::storage]
	pub type AtRiskCursor<T: Config> = StorageValue<_, [u8; 32]>;

	// ========================================================================
	// Events
	// ========================================================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CeremonyRecorded {
			ceremony_hash: [u8; 32],
			bot_public_key: [u8; 32],
			k: u8,
			n: u8,
		},
		CeremonyRevoked {
			ceremony_hash: [u8; 32],
		},
		CeremonySuperseded {
			old_hash: [u8; 32],
			new_hash: [u8; 32],
		},
		CeremonyExpired {
			ceremony_hash: [u8; 32],
		},
		CeremonyAtRisk {
			ceremony_hash: [u8; 32],
			required_k: u8,
		},
		EnclaveApproved {
			mrenclave: [u8; 32],
			version: u32,
		},
		EnclaveRemoved {
			mrenclave: [u8; 32],
		},
		ForcedReCeremony {
			ceremony_hash: [u8; 32],
		},
		/// M3-R6: 终态仪式已清理
		CeremonyCleaned {
			ceremony_hash: [u8; 32],
		},
	}

	// ========================================================================
	// Errors
	// ========================================================================

	#[pallet::error]
	pub enum Error<T> {
		/// Enclave 未在白名单中
		EnclaveNotApproved,
		/// 仪式不存在
		CeremonyNotFound,
		/// 仪式已存在
		CeremonyAlreadyExists,
		/// Shamir 参数无效 (k=0, k>n, n>254)
		InvalidShamirParams,
		/// Enclave 已在白名单中
		EnclaveAlreadyApproved,
		/// Enclave 不在白名单中
		EnclaveNotFound,
		/// 参与者为空
		EmptyParticipants,
		/// 参与者过多
		TooManyParticipants,
		/// Free 层级不允许使用此功能
		FreeTierNotAllowed,
		/// 不是 Bot 所有者
		NotBotOwner,
		/// Bot 不存在
		BotNotFound,
		/// Bot 公钥不匹配
		BotPublicKeyMismatch,
		/// H1-fix: 参与者数量不足以恢复 secret (participant_count < k)
		InsufficientParticipants,
		/// H2-fix: 仪式不是活跃状态 (已撤销/已过期/已替代)
		CeremonyNotActive,
		/// M1-fix: 描述过长 (超过 128 bytes)
		DescriptionTooLong,
		/// M2-audit: 参与者 Enclave 列表含重复项
		DuplicateParticipant,
		/// M1-R4: ExpiryQueue 已满 (1000 条上限)
		ExpiryQueueFull,
		/// M1-R6: Bot 未激活（停用/banned）
		BotNotActive,
		/// M2-R6: 参与者数量超过 Shamir n 参数
		ParticipantCountExceedsN,
		/// M3-R6: 仪式不是终态（无法清理）
		CeremonyNotTerminal,
	}

	// ========================================================================
	// Hooks
	// ========================================================================

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			let interval = T::CeremonyCheckInterval::get();
			if interval == BlockNumberFor::<T>::default() {
				return Weight::zero();
			}
			if (n % interval) != BlockNumberFor::<T>::default() {
				return Weight::zero();
			}

			let max_process = T::MaxProcessPerBlock::get() as usize;
			let mut reads: u64 = 1; // ExpiryQueue read
			let mut writes: u64 = 0;

			// ── Phase 1: 按 expires_at 排序的优先队列处理过期 ──
			let mut queue = ExpiryQueue::<T>::get();
			let mut expired_count: usize = 0;

			// 队列按 expires_at 升序排列，从头部取出已过期的条目
			while expired_count < max_process {
				if queue.is_empty() {
					break;
				}
				let (exp_at, _bot_pk, _hash) = &queue[0];
				if n < *exp_at {
					break; // 队列有序，后续都未过期
				}
				let (_, bot_pk, ceremony_hash) = queue.remove(0);

				reads += 1;
				if let Some(record) = Ceremonies::<T>::get(&ceremony_hash) {
					// 仅处理仍然活跃的仪式（可能已被 revoke/supersede）
					if matches!(record.status, CeremonyStatus::Active) {
						Ceremonies::<T>::mutate(&ceremony_hash, |maybe_record| {
							if let Some(r) = maybe_record {
								r.status = CeremonyStatus::Expired;
							}
						});
						writes += 1;

						// 仅当 ActiveCeremony 仍指向此仪式时才移除
						if ActiveCeremony::<T>::get(&bot_pk) == Some(ceremony_hash) {
							ActiveCeremony::<T>::remove(&bot_pk);
							writes += 1;
						}
						reads += 1;

						Self::deposit_event(Event::CeremonyExpired { ceremony_hash });
					}
				}
				expired_count += 1;
			}

			ExpiryQueue::<T>::put(queue);
			writes += 1;

			// ── Phase 2: 游标分批 AtRisk 检测 ──
			let remaining_budget = max_process.saturating_sub(expired_count);
			if remaining_budget > 0 {
				reads += 1; // AtRiskCursor read
				let cursor = AtRiskCursor::<T>::get();
				let mut processed: usize = 0;
				let mut new_cursor: Option<[u8; 32]> = None;

				// 使用 iter_from 如果有游标，否则从头开始
				let iter: alloc::boxed::Box<dyn Iterator<Item = ([u8; 32], [u8; 32])>> = match cursor {
					Some(ref start_key) => {
						// iter_from 不可用于 StorageMap，使用 iter 并跳过
						// M1-R5: 使用 < 而非 <=，否则游标位置的仪式会被永久跳过
						alloc::boxed::Box::new(
							ActiveCeremony::<T>::iter()
								.skip_while(move |(k, _)| k < start_key)
						)
					},
					None => alloc::boxed::Box::new(ActiveCeremony::<T>::iter()),
				};

				for (bot_pk, ceremony_hash) in iter {
					if processed >= remaining_budget {
						new_cursor = Some(bot_pk);
						break;
					}
					reads += 1;
					if let Some(record) = Ceremonies::<T>::get(&ceremony_hash) {
						reads += 1;
						if matches!(record.status, CeremonyStatus::Active) && n < record.expires_at {
							let peer_count = T::BotRegistry::peer_count(&record.bot_id_hash);
							reads += 1;
							if peer_count <= record.k as u32 {
								Self::deposit_event(Event::CeremonyAtRisk {
									ceremony_hash,
									required_k: record.k,
								});
							}
						}
					}
					processed += 1;
					new_cursor = None; // 全部处理完，清除游标
				}

				// 更新游标
				if let Some(c) = new_cursor {
					AtRiskCursor::<T>::put(c);
				} else {
					AtRiskCursor::<T>::kill();
				}
				writes += 1;
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
		/// 记录仪式 (验证 Enclave 白名单 + Shamir 参数 + 调用者身份)
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(60_000_000, 12_000))]
		pub fn record_ceremony(
			origin: OriginFor<T>,
			ceremony_hash: [u8; 32],
			ceremony_mrenclave: [u8; 32],
			k: u8,
			n: u8,
			bot_public_key: [u8; 32],
			participant_enclaves: alloc::vec::Vec<[u8; 32]>,
			bot_id_hash: [u8; 32],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// G1: 验证调用者是 Bot 所有者
			let owner = T::BotRegistry::bot_owner(&bot_id_hash)
				.ok_or(Error::<T>::BotNotFound)?;
			ensure!(owner == who, Error::<T>::NotBotOwner);

			// M1-R6: 验证 Bot 处于激活状态（停用/banned 的 Bot 不允许发起仪式）
			ensure!(T::BotRegistry::is_bot_active(&bot_id_hash), Error::<T>::BotNotActive);

			// Tier gate: Free 层级不允许发起 Ceremony
			ensure!(
				T::Subscription::effective_tier(&bot_id_hash).is_paid(),
				Error::<T>::FreeTierNotAllowed
			);
			// 验证 bot_public_key 匹配
			if let Some(registered_pk) = T::BotRegistry::bot_public_key(&bot_id_hash) {
				ensure!(registered_pk == bot_public_key, Error::<T>::BotPublicKeyMismatch);
			}

			ensure!(!Ceremonies::<T>::contains_key(&ceremony_hash), Error::<T>::CeremonyAlreadyExists);
			ensure!(k > 0 && k <= n && n <= 254, Error::<T>::InvalidShamirParams);
			ensure!(!participant_enclaves.is_empty(), Error::<T>::EmptyParticipants);
			// H1-fix: 参与者数量必须 >= k (否则无法恢复 secret)
			ensure!(participant_enclaves.len() >= k as usize, Error::<T>::InsufficientParticipants);
			// M2-R6: 参与者数量不能超过 Shamir n 参数（n 份 share 分配给 n 个参与者）
			ensure!(participant_enclaves.len() <= n as usize, Error::<T>::ParticipantCountExceedsN);

			// M2-R5: 先检查上限，再做 O(n²) 去重检测，防止无界输入 DoS
			ensure!(
				participant_enclaves.len() <= T::MaxParticipants::get() as usize,
				Error::<T>::TooManyParticipants
			);

			// M2-audit: 参与者 Enclave 不允许重复（重复会膨胀 participant_count，
			// 使 Shamir 门限看似满足但实际唯一参与者不足）
			for i in 0..participant_enclaves.len() {
				for j in (i + 1)..participant_enclaves.len() {
					ensure!(participant_enclaves[i] != participant_enclaves[j], Error::<T>::DuplicateParticipant);
				}
			}

			// 验证 ceremony enclave 白名单
			ensure!(
				ApprovedEnclaves::<T>::contains_key(&ceremony_mrenclave),
				Error::<T>::EnclaveNotApproved
			);

			let bounded_participants: BoundedVec<[u8; 32], T::MaxParticipants> =
				participant_enclaves.try_into().map_err(|_| Error::<T>::TooManyParticipants)?;

			let participant_count = bounded_participants.len() as u8;

			let now = frame_system::Pallet::<T>::block_number();
			let expires_at = now.saturating_add(T::CeremonyValidityBlocks::get());

			let mut record = CeremonyRecord::<T> {
				ceremony_mrenclave,
				k,
				n,
				bot_public_key,
				participant_count,
				participant_enclaves: bounded_participants,
				initiator: who,
				created_at: now,
				status: CeremonyStatus::Active,
				expires_at,
				is_re_ceremony: false,
				supersedes: None,
				// C1-fix: 存储 bot_id_hash 以供 on_initialize peer_count 查询
				bot_id_hash,
			};

			// G2: 如果已有活跃仪式，标记为 Superseded，记录 Re-ceremony 关系
			let old_ceremony_hash = ActiveCeremony::<T>::get(&bot_public_key);
			if let Some(old_hash) = old_ceremony_hash {
				Ceremonies::<T>::mutate(&old_hash, |maybe_old| {
					if let Some(old) = maybe_old {
						old.status = CeremonyStatus::Superseded { replaced_by: ceremony_hash };
					}
				});
				// L2-R3: 从 ExpiryQueue 中移除被替代的旧仪式
				Self::remove_from_expiry_queue(&old_hash);
				Self::deposit_event(Event::CeremonySuperseded {
					old_hash,
					new_hash: ceremony_hash,
				});
			}

			record.is_re_ceremony = old_ceremony_hash.is_some();
			record.supersedes = old_ceremony_hash;

			Ceremonies::<T>::insert(&ceremony_hash, record);
			ActiveCeremony::<T>::insert(&bot_public_key, ceremony_hash);

			// M2-R4: 历史满时移除最旧条目 (FIFO)，避免永久阻塞新仪式
			CeremonyHistory::<T>::mutate(&bot_public_key, |history| {
				if history.is_full() {
					history.remove(0);
				}
				// 移除后一定有空间，unwrap 安全
				let _ = history.try_push(ceremony_hash);
			});

			CeremonyCount::<T>::mutate(|c| *c = c.saturating_add(1));

			// L2-R3: 插入 ExpiryQueue（按 expires_at 排序）
			// M1-R4: 队列满时返回错误而非静默丢弃（否则仪式永远不会被自动过期）
			ExpiryQueue::<T>::try_mutate(|queue| -> DispatchResult {
				let entry = (expires_at, bot_public_key, ceremony_hash);
				let pos = queue.iter().position(|(e, _, _)| *e > expires_at).unwrap_or(queue.len());
				queue.try_insert(pos, entry).map_err(|_| Error::<T>::ExpiryQueueFull)?;
				Ok(())
			})?;

			Self::deposit_event(Event::CeremonyRecorded { ceremony_hash, bot_public_key, k, n });
			Ok(())
		}

		/// 撤销仪式 (root)
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn revoke_ceremony(
			origin: OriginFor<T>,
			ceremony_hash: [u8; 32],
		) -> DispatchResult {
			ensure_root(origin)?;
			// L1-R6: 使用共享 helper
			Self::do_revoke(&ceremony_hash)?;
			Self::deposit_event(Event::CeremonyRevoked { ceremony_hash });
			Ok(())
		}

		/// 添加 Ceremony Enclave 到白名单
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
		pub fn approve_ceremony_enclave(
			origin: OriginFor<T>,
			mrenclave: [u8; 32],
			version: u32,
			description: alloc::vec::Vec<u8>,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(
				!ApprovedEnclaves::<T>::contains_key(&mrenclave),
				Error::<T>::EnclaveAlreadyApproved
			);

			let now = frame_system::Pallet::<T>::block_number();
			// M1-fix: 描述过长时返回错误而非静默截断为空
			let bounded_desc: BoundedVec<u8, ConstU32<128>> =
				description.try_into().map_err(|_| Error::<T>::DescriptionTooLong)?;

			let now_u64: u64 = now.unique_saturated_into();
			let info = CeremonyEnclaveInfo {
				version,
				approved_at: now_u64,
				description: bounded_desc,
			};
			ApprovedEnclaves::<T>::insert(&mrenclave, info);

			Self::deposit_event(Event::EnclaveApproved { mrenclave, version });
			Ok(())
		}

		/// 移除 Ceremony Enclave
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
		pub fn remove_ceremony_enclave(
			origin: OriginFor<T>,
			mrenclave: [u8; 32],
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(
				ApprovedEnclaves::<T>::contains_key(&mrenclave),
				Error::<T>::EnclaveNotFound
			);
			ApprovedEnclaves::<T>::remove(&mrenclave);
			Self::deposit_event(Event::EnclaveRemoved { mrenclave });
			Ok(())
		}

		/// 强制触发 re-ceremony (安全事件)
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn force_re_ceremony(
			origin: OriginFor<T>,
			ceremony_hash: [u8; 32],
		) -> DispatchResult {
			ensure_root(origin)?;
			// L1-R6: 使用共享 helper
			Self::do_revoke(&ceremony_hash)?;
			Self::deposit_event(Event::ForcedReCeremony { ceremony_hash });
			Ok(())
		}

		/// M3-R6: 清理终态仪式记录 (任何人可调用)
		/// 仅清理 Expired / Revoked / Superseded 状态的仪式
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::from_parts(30_000_000, 6_000))]
		pub fn cleanup_ceremony(
			origin: OriginFor<T>,
			ceremony_hash: [u8; 32],
		) -> DispatchResult {
			ensure_signed(origin)?;

			let record = Ceremonies::<T>::get(&ceremony_hash).ok_or(Error::<T>::CeremonyNotFound)?;
			ensure!(
				matches!(
					record.status,
					CeremonyStatus::Expired | CeremonyStatus::Revoked { .. } | CeremonyStatus::Superseded { .. }
				),
				Error::<T>::CeremonyNotTerminal
			);

			Ceremonies::<T>::remove(&ceremony_hash);

			Self::deposit_event(Event::CeremonyCleaned { ceremony_hash });
			Ok(())
		}
	}

	// ========================================================================
	// Helper Functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// L1-R6: 共享撤销逻辑 — revoke_ceremony 和 force_re_ceremony 的公共实现
		fn do_revoke(ceremony_hash: &[u8; 32]) -> DispatchResult {
			let record = Ceremonies::<T>::get(ceremony_hash).ok_or(Error::<T>::CeremonyNotFound)?;
			ensure!(
				matches!(record.status, CeremonyStatus::Active),
				Error::<T>::CeremonyNotActive
			);

			let now = frame_system::Pallet::<T>::block_number();
			let now_u64: u64 = now.unique_saturated_into();
			Ceremonies::<T>::mutate(ceremony_hash, |maybe_record| {
				if let Some(r) = maybe_record {
					r.status = CeremonyStatus::Revoked { revoked_at: now_u64 };
				}
			});

			if ActiveCeremony::<T>::get(&record.bot_public_key) == Some(*ceremony_hash) {
				ActiveCeremony::<T>::remove(&record.bot_public_key);
			}

			Self::remove_from_expiry_queue(ceremony_hash);
			Ok(())
		}

		/// L2-R3: 从 ExpiryQueue 中移除指定 ceremony_hash 的条目
		fn remove_from_expiry_queue(ceremony_hash: &[u8; 32]) {
			ExpiryQueue::<T>::mutate(|queue| {
				queue.retain(|(_, _, h)| h != ceremony_hash);
			});
		}

		/// 仪式是否活跃
		pub fn is_ceremony_active(bot_public_key: &[u8; 32]) -> bool {
			ActiveCeremony::<T>::get(bot_public_key)
				.and_then(|hash| Ceremonies::<T>::get(&hash))
				.map(|r| matches!(r.status, CeremonyStatus::Active))
				.unwrap_or(false)
		}

		/// 获取活跃仪式
		pub fn get_active_ceremony(bot_public_key: &[u8; 32]) -> Option<[u8; 32]> {
			ActiveCeremony::<T>::get(bot_public_key)
		}

		/// Enclave 是否已审批
		pub fn is_enclave_approved(mrenclave: &[u8; 32]) -> bool {
			ApprovedEnclaves::<T>::contains_key(mrenclave)
		}

		/// 获取 Shamir 参数
		pub fn ceremony_shamir_params(bot_public_key: &[u8; 32]) -> Option<(u8, u8)> {
			ActiveCeremony::<T>::get(bot_public_key)
				.and_then(|hash| Ceremonies::<T>::get(&hash))
				.map(|r| (r.k, r.n))
		}
	}

}
