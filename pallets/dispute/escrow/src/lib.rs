#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

pub mod migrations;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use frame_support::weights::Weight;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, EnsureOrigin, ExistenceRequirement},
        PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{AccountIdConversion, Saturating, Zero};
    use sp_runtime::DispatchError;

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    pub const STATE_LOCKED: u8 = 0;
    pub const STATE_DISPUTED: u8 = 1;
    pub const STATE_CLOSED: u8 = 3;

    /// 供其他 Pallet 内部调用的托管接口
    pub trait Escrow<AccountId, Balance> {
        /// 获取托管账户地址
        /// 函数级详细中文注释：返回托管 Pallet 的账户地址，用于外部模块进行押金操作
        fn escrow_account() -> AccountId;
        /// 从付款人转入托管并记录
        /// 函数级详细中文注释：安全要求
        /// - 必须确保付款人余额充足（不足则返回 Error::Insufficient）
        /// - 仅供其他 Pallet 内部调用，不对外暴露权限判断；外部 extrinsic 需严格限制 Origin
        fn lock_from(payer: &AccountId, id: u64, amount: Balance) -> DispatchResult;
        /// 从托管转出部分金额到指定账户（可多次分账），直至全部转出
        /// 函数级详细中文注释：安全要求
        /// - 必须确保本 id 当前托管余额充足（amount ≤ cur），否则拒绝（Error::Insufficient）
        /// - 一次成功划转为原子事务，状态与实际转账保持一致
        fn transfer_from_escrow(id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
        /// 将托管全部释放给收款人
        /// 函数级详细中文注释：将 id 对应全部锁定余额转给 to，用于正常履约或仲裁裁决
        fn release_all(id: u64, to: &AccountId) -> DispatchResult;
        /// 将托管全部退款给收款人
        /// 函数级详细中文注释：将 id 对应全部锁定余额退回给 to，用于撤单/到期退款等场景
        fn refund_all(id: u64, to: &AccountId) -> DispatchResult;
        /// 🆕 F1: 部分退款 — 从托管退回指定金额给 to
        /// 函数级详细中文注释：用于订单部分履约场景，退还未履约部分的金额
        fn refund_partial(id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
        /// 🆕 F3: 部分释放 — 从托管释放指定金额给 to（里程碑式释放）
        /// 函数级详细中文注释：用于服务类订单按进度分阶段释放资金
        fn release_partial(id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
        /// 查询当前托管余额
        fn amount_of(id: u64) -> Balance;
        /// 按比例分账：bps/10000 给 release_to，剩余给 refund_to
        /// 函数级详细中文注释：用于仲裁部分裁决场景
        /// - bps: 基点（10000 = 100%），表示 release_to 获得的比例
        /// - release_to: 获得 bps/10000 比例的账户
        /// - refund_to: 获得剩余比例的账户
        fn split_partial(id: u64, release_to: &AccountId, refund_to: &AccountId, bps: u16) -> DispatchResult;
        /// 将托管标记为争议状态（Disputed=1）
        /// 函数级详细中文注释：供业务模块在订单进入争议时调用，
        /// 设置后 release/refund/transfer 等操作将被阻止，仅允许仲裁决议接口处理
        fn set_disputed(id: u64) -> DispatchResult;
        /// 将托管从争议状态恢复为正常（Locked=0）
        /// 函数级详细中文注释：供仲裁模块在裁决执行前调用，解除争议锁定以允许资金操作
        fn set_resolved(id: u64) -> DispatchResult;
    }

    /// 🆕 F10: 托管状态变更观察者接口
    /// 函数级详细中文注释：业务模块实现此 trait 以接收托管状态变更通知，
    /// 用于同步更新订单状态等下游逻辑
    pub trait EscrowObserver<AccountId, Balance> {
        /// 托管资金已释放（含部分释放）
        fn on_released(id: u64, to: &AccountId, amount: Balance);
        /// 托管资金已退款（含部分退款）
        fn on_refunded(id: u64, to: &AccountId, amount: Balance);
        /// 托管到期已处理
        fn on_expired(id: u64, action: u8);
        /// 托管进入争议
        fn on_disputed(id: u64);
        /// 🆕 F6: 管理员应急操作已执行
        fn on_force_action(id: u64, action: u8);
    }

    /// 空实现：不需要回调时使用
    impl<AccountId, Balance> EscrowObserver<AccountId, Balance> for () {
        fn on_released(_id: u64, _to: &AccountId, _amount: Balance) {}
        fn on_refunded(_id: u64, _to: &AccountId, _amount: Balance) {}
        fn on_expired(_id: u64, _action: u8) {}
        fn on_disputed(_id: u64) {}
        fn on_force_action(_id: u64, _action: u8) {}
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;
        type EscrowPalletId: Get<PalletId>;
        /// 函数级中文注释：授权外部入口的 Origin（白名单 Origin）。
        type AuthorizedOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// 函数级中文注释：管理员 Origin（治理/应急）。
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// 函数级中文注释：每块最多处理的到期项（防御性限制）。
        #[pallet::constant]
        type MaxExpiringPerBlock: Get<u32>;
        /// 🆕 M4修复: release_split 最大分账条目数（防止区块超重）
        #[pallet::constant]
        type MaxSplitEntries: Get<u32>;
        /// 函数级中文注释：到期处理策略，由 runtime 注入。
        type ExpiryPolicy: ExpiryPolicy<Self::AccountId, BlockNumberFor<Self>>;
        /// 🆕 F5: 争议原因最大长度
        #[pallet::constant]
        type MaxReasonLen: Get<u32>;
        /// 🆕 F10: 托管状态变更观察者（通知业务模块）
        type Observer: EscrowObserver<Self::AccountId, BalanceOf<Self>>;
        /// 🆕 F8: 每次清理调用最大条目数
        #[pallet::constant]
        type MaxCleanupPerCall: Get<u32>;
        /// 争议最大持续时间（块数）。超时后到期处理自动绕过争议状态执行 ExpiryPolicy
        #[pallet::constant]
        type MaxDisputeDuration: Get<BlockNumberFor<Self>>;
        /// 🆕 M2修复: 权重信息
        type WeightInfo: crate::weights::WeightInfo;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// 简单托管：订单 -> 锁定余额
    #[pallet::storage]
    pub type Locked<T: Config> = StorageMap<_, Blake2_128Concat, u64, BalanceOf<T>, ValueQuery>;

    /// 函数级中文注释：全局暂停开关（应急止血）。
    /// - 为 true 时，除 AdminOrigin 外的变更性操作将被拒绝。
    #[pallet::storage]
    pub type Paused<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// 托管状态：0=Locked, 1=Disputed, 3=Closed
    /// - Disputed 状态下仅允许仲裁决议接口处理；
    /// - Closed 表示已全部结清，不再接受出金操作。
    #[pallet::storage]
    pub type LockStateOf<T: Config> = StorageMap<_, Blake2_128Concat, u64, u8, ValueQuery>;

    // LockNonces 已在 v2 migration 中移除（lock_with_nonce extrinsic 已删除）

    /// 函数级中文注释：到期块存储：id -> at（仅当启用到期策略时写入）。
    #[pallet::storage]
    pub type ExpiryOf<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, BlockNumberFor<T>, OptionQuery>;

    /// 函数级中文注释：按区块号索引到期项（H-1修复：优化 on_initialize 性能）
    /// 存储结构：block_number -> Vec<id>
    /// 用途：on_initialize 可以直接获取当前块到期的项，避免迭代所有 ExpiryOf
    #[pallet::storage]
    pub type ExpiringAt<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        BoundedVec<u64, T::MaxExpiringPerBlock>,
        ValueQuery,
    >;

    /// 🆕 F4: 争议时间戳记录（id -> 争议发起的区块号）
    #[pallet::storage]
    pub type DisputedAt<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, BlockNumberFor<T>, OptionQuery>;

    /// 🆕 R5-M1: 付款人记录（id -> payer），首次 lock_from 写入，ExpiryPolicy 默认退款目标
    #[pallet::storage]
    pub type PayerOf<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, T::AccountId, OptionQuery>;


    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 🆕 F2: 锁定到托管账户（含 payer 信息）
        Locked { id: u64, payer: T::AccountId, amount: BalanceOf<T> },
        /// 从托管部分划转（多次分账）
        Transferred {
            id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
            remaining: BalanceOf<T>,
        },
        /// 全额释放
        Released {
            id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// 全额退款
        Refunded {
            id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// 🆕 F1: 部分退款
        PartialRefunded {
            id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
            remaining: BalanceOf<T>,
        },
        /// 🆕 F3: 部分释放（里程碑式）
        PartialReleased {
            id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
            remaining: BalanceOf<T>,
        },
        /// 🆕 F5: 进入争议（含 BoundedVec 原因和时间戳）
        Disputed {
            id: u64,
            reason: u16,
            detail: BoundedVec<u8, T::MaxReasonLen>,
            at: BlockNumberFor<T>,
        },
        /// 已应用仲裁决议（0=ReleaseAll,1=RefundAll,2=PartialBps）
        DecisionApplied { id: u64, decision: u8 },
        /// 函数级中文注释：已安排到期处理（id, at）。
        ExpiryScheduled { id: u64, at: BlockNumberFor<T> },
        /// 函数级中文注释：到期已处理（id, action: 0=Release,1=Refund,2=Noop）。
        Expired { id: u64, action: u8 },
        /// 函数级中文注释：按比例分账完成
        PartialSplit {
            id: u64,
            release_to: T::AccountId,
            release_amount: BalanceOf<T>,
            refund_to: T::AccountId,
            refund_amount: BalanceOf<T>,
        },
        /// 🆕 F7: 全局暂停切换事件
        PauseToggled { paused: bool },
        /// 🆕 F6: 管理员应急操作（0=ForceRelease, 1=ForceRefund）
        ForceAction {
            id: u64,
            action: u8,
            to: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// 🆕 F8: 已清理的 Closed 托管记录
        Cleaned { ids: Vec<u64> },
    }

    #[pallet::error]
    pub enum Error<T> {
        Insufficient,
        NoLock,
        /// 托管处于争议状态，禁止操作
        DisputeActive,
        /// 托管已关闭
        AlreadyClosed,
        /// 全局暂停中
        GloballyPaused,
        /// 🆕 到期队列已满
        ExpiringAtFull,
        /// 🆕 L-2修复: 托管非争议状态（set_resolved 要求 state==1）
        NotInDispute,
    }

    pub trait ExpiryPolicy<AccountId, BlockNumber> {
        /// 返回到期应执行的动作：ReleaseAll(to) | RefundAll(to) | Noop
        fn on_expire(id: u64) -> Result<ExpiryAction<AccountId>, sp_runtime::DispatchError>;
    }

    pub enum ExpiryAction<AccountId> {
        ReleaseAll(AccountId),
        RefundAll(AccountId),
        Noop,
    }

    impl<T: Config> Pallet<T> {
        fn account() -> T::AccountId {
            T::EscrowPalletId::get().into_account_truncating()
        }
        /// 函数级中文注释：断言未暂停。
        #[inline]
        fn ensure_not_paused() -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::GloballyPaused);
            Ok(())
        }
        /// 函数级中文注释：统一授权校验（AuthorizedOrigin | Root）。
        #[inline]
        fn ensure_auth(origin: T::RuntimeOrigin) -> Result<(), DispatchError> {
            if frame_system::EnsureRoot::<T::AccountId>::try_origin(origin.clone()).is_ok() {
                return Ok(());
            }
            if <T as Config>::AuthorizedOrigin::try_origin(origin).is_ok() {
                return Ok(());
            }
            Err(DispatchError::BadOrigin)
        }

        /// 尝试将 id 调度到目标块（如目标块已满则尝试相邻块，最多 10 次）
        fn try_schedule_expiry_at(id: u64, at: BlockNumberFor<T>) -> bool {
            for offset in 0u32..10 {
                let target = at.saturating_add(offset.into());
                if ExpiringAt::<T>::try_mutate(target, |ids| {
                    ids.try_push(id).map_err(|_| ())
                }).is_ok() {
                    ExpiryOf::<T>::insert(id, target);
                    return true;
                }
            }
            false
        }

        /// R5-H3: 移除 id 的到期调度（ExpiryOf + ExpiringAt 索引）
        fn remove_expiry_schedule(id: u64) {
            if let Some(at) = ExpiryOf::<T>::take(id) {
                ExpiringAt::<T>::mutate(at, |ids| {
                    if let Some(pos) = ids.iter().position(|&x| x == id) {
                        ids.swap_remove(pos);
                    }
                });
            }
        }

        /// 共享争议逻辑：set_disputed trait 与 dispute extrinsic 共用
        fn do_set_disputed(id: u64, reason: u16, detail: BoundedVec<u8, T::MaxReasonLen>) -> DispatchResult {
            let cur = Locked::<T>::get(id);
            ensure!(!cur.is_zero(), Error::<T>::NoLock);
            let state = LockStateOf::<T>::get(id);
            ensure!(state != STATE_CLOSED, Error::<T>::AlreadyClosed);
            ensure!(state != STATE_DISPUTED, Error::<T>::DisputeActive);
            LockStateOf::<T>::insert(id, STATE_DISPUTED);
            let now = <frame_system::Pallet<T>>::block_number();
            DisputedAt::<T>::insert(id, now);
            if ExpiryOf::<T>::get(id).is_none() {
                let timeout_at = now.saturating_add(T::MaxDisputeDuration::get());
                Self::try_schedule_expiry_at(id, timeout_at);
            }
            Self::deposit_event(Event::Disputed { id, reason, detail, at: now });
            T::Observer::on_disputed(id);
            Ok(())
        }
    }

    impl<T: Config> Escrow<T::AccountId, BalanceOf<T>> for Pallet<T> {
        fn escrow_account() -> T::AccountId {
            Self::account()
        }
        fn lock_from(payer: &T::AccountId, id: u64, amount: BalanceOf<T>) -> DispatchResult {
            ensure!(!amount.is_zero(), Error::<T>::Insufficient);
            let state = LockStateOf::<T>::get(id);
            ensure!(state != STATE_CLOSED, Error::<T>::AlreadyClosed);
            ensure!(state != STATE_DISPUTED, Error::<T>::DisputeActive);
            let escrow = Self::account();
            T::Currency::transfer(payer, &escrow, amount, ExistenceRequirement::KeepAlive)
                .map_err(|_| Error::<T>::Insufficient)?;
            let cur = Locked::<T>::get(id);
            Locked::<T>::insert(id, cur.saturating_add(amount));
            // R5-M1: 记录首次付款人（ExpiryPolicy 退款目标）
            if !PayerOf::<T>::contains_key(id) {
                PayerOf::<T>::insert(id, payer.clone());
            }
            Self::deposit_event(Event::Locked { id, payer: payer.clone(), amount });
            Ok(())
        }
        fn transfer_from_escrow(
            id: u64,
            to: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let state = LockStateOf::<T>::get(id);
            ensure!(state != STATE_DISPUTED, Error::<T>::DisputeActive);
            ensure!(state != STATE_CLOSED, Error::<T>::AlreadyClosed);
            let cur = Locked::<T>::get(id);
            ensure!(!cur.is_zero(), Error::<T>::NoLock);
            ensure!(amount <= cur, Error::<T>::Insufficient);
            let new = cur.saturating_sub(amount);
            Locked::<T>::insert(id, new);
            let escrow = Self::account();
            // 🆕 L-3修复: 最后一笔使用 AllowDeath
            let existence = if new.is_zero() {
                ExistenceRequirement::AllowDeath
            } else {
                ExistenceRequirement::KeepAlive
            };
            T::Currency::transfer(&escrow, to, amount, existence)
                .map_err(|_| Error::<T>::NoLock)?;
            if new.is_zero() {
                Locked::<T>::remove(id);
            }
            Self::deposit_event(Event::Transferred {
                id,
                to: to.clone(),
                amount,
                remaining: new,
            });
            Ok(())
        }
        fn release_all(id: u64, to: &T::AccountId) -> DispatchResult {
            let state = LockStateOf::<T>::get(id);
            ensure!(state != STATE_DISPUTED, Error::<T>::DisputeActive);
            ensure!(state != STATE_CLOSED, Error::<T>::AlreadyClosed);
            
            let amount = Locked::<T>::take(id);
            ensure!(!amount.is_zero(), Error::<T>::NoLock);
            
            let escrow = Self::account();
            T::Currency::transfer(&escrow, to, amount, ExistenceRequirement::AllowDeath)
                .map_err(|_| Error::<T>::NoLock)?;
            
            LockStateOf::<T>::insert(id, STATE_CLOSED);
            Self::remove_expiry_schedule(id);
            
            Self::deposit_event(Event::Released {
                id,
                to: to.clone(),
                amount,
            });
            T::Observer::on_released(id, to, amount);
            Ok(())
        }
        fn refund_all(id: u64, to: &T::AccountId) -> DispatchResult {
            let state = LockStateOf::<T>::get(id);
            ensure!(state != STATE_DISPUTED, Error::<T>::DisputeActive);
            ensure!(state != STATE_CLOSED, Error::<T>::AlreadyClosed);
            
            let amount = Locked::<T>::take(id);
            ensure!(!amount.is_zero(), Error::<T>::NoLock);
            
            let escrow = Self::account();
            T::Currency::transfer(&escrow, to, amount, ExistenceRequirement::AllowDeath)
                .map_err(|_| Error::<T>::NoLock)?;
            
            LockStateOf::<T>::insert(id, STATE_CLOSED);
            Self::remove_expiry_schedule(id);
            
            Self::deposit_event(Event::Refunded {
                id,
                to: to.clone(),
                amount,
            });
            T::Observer::on_refunded(id, to, amount);
            Ok(())
        }
        fn refund_partial(id: u64, to: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            ensure!(!amount.is_zero(), Error::<T>::Insufficient);
            let state = LockStateOf::<T>::get(id);
            ensure!(state != STATE_DISPUTED, Error::<T>::DisputeActive);
            ensure!(state != STATE_CLOSED, Error::<T>::AlreadyClosed);
            let cur = Locked::<T>::get(id);
            ensure!(!cur.is_zero(), Error::<T>::NoLock);
            ensure!(amount <= cur, Error::<T>::Insufficient);
            let new = cur.saturating_sub(amount);
            Locked::<T>::insert(id, new);
            let escrow = Self::account();
            let existence = if new.is_zero() {
                ExistenceRequirement::AllowDeath
            } else {
                ExistenceRequirement::KeepAlive
            };
            T::Currency::transfer(&escrow, to, amount, existence)
                .map_err(|_| Error::<T>::NoLock)?;
            if new.is_zero() {
                Locked::<T>::remove(id);
                LockStateOf::<T>::insert(id, STATE_CLOSED);
                Self::remove_expiry_schedule(id);
            }
            Self::deposit_event(Event::PartialRefunded {
                id,
                to: to.clone(),
                amount,
                remaining: new,
            });
            T::Observer::on_refunded(id, to, amount);
            Ok(())
        }
        fn release_partial(id: u64, to: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            ensure!(!amount.is_zero(), Error::<T>::Insufficient);
            let state = LockStateOf::<T>::get(id);
            ensure!(state != STATE_DISPUTED, Error::<T>::DisputeActive);
            ensure!(state != STATE_CLOSED, Error::<T>::AlreadyClosed);
            let cur = Locked::<T>::get(id);
            ensure!(!cur.is_zero(), Error::<T>::NoLock);
            ensure!(amount <= cur, Error::<T>::Insufficient);
            let new = cur.saturating_sub(amount);
            Locked::<T>::insert(id, new);
            let escrow = Self::account();
            let existence = if new.is_zero() {
                ExistenceRequirement::AllowDeath
            } else {
                ExistenceRequirement::KeepAlive
            };
            T::Currency::transfer(&escrow, to, amount, existence)
                .map_err(|_| Error::<T>::NoLock)?;
            if new.is_zero() {
                Locked::<T>::remove(id);
                LockStateOf::<T>::insert(id, STATE_CLOSED);
                Self::remove_expiry_schedule(id);
            }
            Self::deposit_event(Event::PartialReleased {
                id,
                to: to.clone(),
                amount,
                remaining: new,
            });
            T::Observer::on_released(id, to, amount);
            Ok(())
        }
        fn amount_of(id: u64) -> BalanceOf<T> {
            Locked::<T>::get(id)
        }
        fn set_disputed(id: u64) -> DispatchResult {
            Self::do_set_disputed(id, 0, BoundedVec::default())
        }
        fn set_resolved(id: u64) -> DispatchResult {
            let state = LockStateOf::<T>::get(id);
            ensure!(state == STATE_DISPUTED, Error::<T>::NotInDispute);
            LockStateOf::<T>::insert(id, STATE_LOCKED);
            DisputedAt::<T>::remove(id);
            Ok(())
        }
        fn split_partial(
            id: u64,
            release_to: &T::AccountId,
            refund_to: &T::AccountId,
            bps: u16,
        ) -> DispatchResult {
            ensure!(bps <= 10_000, Error::<T>::Insufficient);
            let state = LockStateOf::<T>::get(id);
            ensure!(state != STATE_DISPUTED, Error::<T>::DisputeActive);
            ensure!(state != STATE_CLOSED, Error::<T>::AlreadyClosed);
            
            let total = Locked::<T>::take(id);
            ensure!(!total.is_zero(), Error::<T>::NoLock);
            
            let escrow = Self::account();
            
            let release_amount = sp_runtime::Permill::from_parts((bps as u32) * 100)
                .mul_floor(total);
            let refund_amount = total.saturating_sub(release_amount);
            
            if !release_amount.is_zero() && !refund_amount.is_zero() {
                T::Currency::transfer(&escrow, release_to, release_amount, ExistenceRequirement::KeepAlive)
                    .map_err(|_| Error::<T>::Insufficient)?;
                T::Currency::transfer(&escrow, refund_to, refund_amount, ExistenceRequirement::AllowDeath)
                    .map_err(|_| Error::<T>::Insufficient)?;
            } else if !release_amount.is_zero() {
                T::Currency::transfer(&escrow, release_to, release_amount, ExistenceRequirement::AllowDeath)
                    .map_err(|_| Error::<T>::Insufficient)?;
            } else if !refund_amount.is_zero() {
                T::Currency::transfer(&escrow, refund_to, refund_amount, ExistenceRequirement::AllowDeath)
                    .map_err(|_| Error::<T>::Insufficient)?;
            }
            
            LockStateOf::<T>::insert(id, STATE_CLOSED);
            Self::remove_expiry_schedule(id);
            
            Self::deposit_event(Event::PartialSplit {
                id,
                release_to: release_to.clone(),
                release_amount,
                refund_to: refund_to.clone(),
                refund_amount,
            });
            if !release_amount.is_zero() {
                T::Observer::on_released(id, release_to, release_amount);
            }
            if !refund_amount.is_zero() {
                T::Observer::on_refunded(id, refund_to, refund_amount);
            }
            Ok(())
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 锁定：从付款人划转到托管账户并记录
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::lock())]
        pub fn lock(
            origin: OriginFor<T>,
            id: u64,
            payer: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::lock_from(&payer, id, amount)
        }
        /// 释放：将托管金额转给收款人
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::release())]
        pub fn release(origin: OriginFor<T>, id: u64, to: T::AccountId) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::release_all(id, &to)
        }
        /// 退款：退回付款人
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::refund())]
        pub fn refund(origin: OriginFor<T>, id: u64, to: T::AccountId) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::refund_all(id, &to)
        }

        // ⚠️ call_index(3) 已移除：lock_with_nonce extrinsic 已删除。
        // nonce 幂等逻辑应由调用方（上游 pallet）自行实现。
        // trait 方法 lock_from 仍可用。

        /// 函数级详细中文注释：分账释放（原子）。校验合计不超过托管余额，逐笔转账，剩余为 0 则清键。
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::release_split())]
        pub fn release_split(
            origin: OriginFor<T>,
            id: u64,
            entries: BoundedVec<(T::AccountId, BalanceOf<T>), T::MaxSplitEntries>,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            let state = LockStateOf::<T>::get(id);
            ensure!(state != STATE_DISPUTED, Error::<T>::DisputeActive);
            ensure!(state != STATE_CLOSED, Error::<T>::AlreadyClosed);
            let mut cur = Locked::<T>::get(id);
            ensure!(!cur.is_zero(), Error::<T>::NoLock);
            // 校验合计
            let mut sum: BalanceOf<T> = Zero::zero();
            for (_to, amt) in entries.iter() {
                sum = sum.saturating_add(*amt);
            }
            ensure!(sum <= cur, Error::<T>::Insufficient);
            // 逐笔转账
            for (to, amt) in entries.into_iter() {
                if amt.is_zero() {
                    continue;
                }
                cur = cur.saturating_sub(amt);
                Locked::<T>::insert(id, cur);
                let escrow = Self::account();
                // 最后一笔（余额为零）使用 AllowDeath，避免 ED 问题
                let existence = if cur.is_zero() {
                    ExistenceRequirement::AllowDeath
                } else {
                    ExistenceRequirement::KeepAlive
                };
                T::Currency::transfer(&escrow, &to, amt, existence)
                    .map_err(|_| Error::<T>::NoLock)?;
                Self::deposit_event(Event::Transferred {
                    id,
                    to: to.clone(),
                    amount: amt,
                    remaining: cur,
                });
                // 🆕 M3-R2修复: 通知观察者每笔分账释放
                T::Observer::on_released(id, &to, amt);
            }
            if cur.is_zero() {
                Locked::<T>::remove(id);
                LockStateOf::<T>::insert(id, STATE_CLOSED);
                Self::remove_expiry_schedule(id);
            }
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::dispute())]
        pub fn dispute(
            origin: OriginFor<T>,
            id: u64,
            reason: u16,
            detail: BoundedVec<u8, T::MaxReasonLen>,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::do_set_disputed(id, reason, detail)
        }

        /// 函数级中文注释：仲裁决议-全额释放。
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::apply_decision_release())]
        pub fn apply_decision_release_all(
            origin: OriginFor<T>,
            id: u64,
            to: T::AccountId,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            ensure!(LockStateOf::<T>::get(id) == STATE_DISPUTED, Error::<T>::NotInDispute);
            LockStateOf::<T>::insert(id, STATE_LOCKED);
            DisputedAt::<T>::remove(id);
            Self::remove_expiry_schedule(id);
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::release_all(id, &to)?;
            Self::deposit_event(Event::DecisionApplied { id, decision: 0 });
            Ok(())
        }

        /// 函数级中文注释：仲裁决议-全额退款。
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::apply_decision_refund())]
        pub fn apply_decision_refund_all(
            origin: OriginFor<T>,
            id: u64,
            to: T::AccountId,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            ensure!(LockStateOf::<T>::get(id) == STATE_DISPUTED, Error::<T>::NotInDispute);
            LockStateOf::<T>::insert(id, STATE_LOCKED);
            DisputedAt::<T>::remove(id);
            Self::remove_expiry_schedule(id);
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::refund_all(id, &to)?;
            Self::deposit_event(Event::DecisionApplied { id, decision: 1 });
            Ok(())
        }

        /// 仲裁决议-按 bps 部分释放，其余退款给 refund_to。复用 split_partial 保证计算一致性
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::apply_decision_partial())]
        pub fn apply_decision_partial_bps(
            origin: OriginFor<T>,
            id: u64,
            release_to: T::AccountId,
            refund_to: T::AccountId,
            bps: u16,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            ensure!(LockStateOf::<T>::get(id) == STATE_DISPUTED, Error::<T>::NotInDispute);
            ensure!(bps <= 10_000, Error::<T>::Insufficient);
            LockStateOf::<T>::insert(id, STATE_LOCKED);
            DisputedAt::<T>::remove(id);
            Self::remove_expiry_schedule(id);
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::split_partial(id, &release_to, &refund_to, bps)?;
            Self::deposit_event(Event::DecisionApplied { id, decision: 2 });
            Ok(())
        }

        /// 🆕 F7: 设置全局暂停（Admin），发出 PauseToggled 事件
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::set_pause())]
        pub fn set_pause(origin: OriginFor<T>, paused: bool) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            Paused::<T>::put(paused);
            Self::deposit_event(Event::PauseToggled { paused });
            Ok(())
        }

        /// ⚠️ 运维后门：手动调度到期（正常业务流程不经此入口，由 do_set_disputed / on_initialize 内部调度）
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::schedule_expiry())]
        pub fn schedule_expiry(
            origin: OriginFor<T>,
            id: u64,
            at: BlockNumberFor<T>,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            let state = LockStateOf::<T>::get(id);
            ensure!(state != STATE_CLOSED, Error::<T>::AlreadyClosed);
            ensure!(!Locked::<T>::get(id).is_zero(), Error::<T>::NoLock);
            if state == STATE_DISPUTED {
                return Ok(());
            }
            Self::remove_expiry_schedule(id);
            ExpiryOf::<T>::insert(id, at);
            ExpiringAt::<T>::try_mutate(at, |ids| -> DispatchResult {
                ids.try_push(id).map_err(|_| Error::<T>::ExpiringAtFull)?;
                Ok(())
            })?;
            Self::deposit_event(Event::ExpiryScheduled { id, at });
            Ok(())
        }

        /// ⚠️ 运维后门：手动取消到期（正常业务流程不经此入口，由关闭操作内部清理）
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::cancel_expiry())]
        pub fn cancel_expiry(origin: OriginFor<T>, id: u64) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::remove_expiry_schedule(id);
            Ok(())
        }

        /// 🆕 F6: 管理员应急强制释放（绕过状态机）
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::force_release())]
        pub fn force_release(
            origin: OriginFor<T>,
            id: u64,
            to: T::AccountId,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            let amount = Locked::<T>::take(id);
            ensure!(!amount.is_zero(), Error::<T>::NoLock);
            let escrow = Self::account();
            T::Currency::transfer(&escrow, &to, amount, ExistenceRequirement::AllowDeath)
                .map_err(|_| Error::<T>::NoLock)?;
            LockStateOf::<T>::insert(id, STATE_CLOSED);
            DisputedAt::<T>::remove(id);
            Self::remove_expiry_schedule(id);
            Self::deposit_event(Event::ForceAction { id, action: 0, to: to.clone(), amount });
            T::Observer::on_force_action(id, 0);
            Ok(())
        }

        /// 🆕 F6: 管理员应急强制退款（绕过状态机）
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::force_refund())]
        pub fn force_refund(
            origin: OriginFor<T>,
            id: u64,
            to: T::AccountId,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            let amount = Locked::<T>::take(id);
            ensure!(!amount.is_zero(), Error::<T>::NoLock);
            let escrow = Self::account();
            T::Currency::transfer(&escrow, &to, amount, ExistenceRequirement::AllowDeath)
                .map_err(|_| Error::<T>::NoLock)?;
            LockStateOf::<T>::insert(id, STATE_CLOSED);
            DisputedAt::<T>::remove(id);
            Self::remove_expiry_schedule(id);
            Self::deposit_event(Event::ForceAction { id, action: 1, to: to.clone(), amount });
            T::Observer::on_force_action(id, 1);
            Ok(())
        }

        // ⚠️ call_index(14) 已移除：refund_partial extrinsic 已删除。
        // trait 方法 Escrow::refund_partial 仍可用，等待上游 pallet 接入。

        // ⚠️ call_index(15) 已移除：release_partial extrinsic 已删除。
        // trait 方法 Escrow::release_partial 仍可用，等待上游 pallet 接入。

        /// 🆕 F8: 清理已关闭的托管记录（任何人可调用）
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::cleanup_closed())]
        pub fn cleanup_closed(
            origin: OriginFor<T>,
            ids: BoundedVec<u64, T::MaxCleanupPerCall>,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let mut cleaned = Vec::new();
            for id in ids.iter() {
                if LockStateOf::<T>::get(id) != STATE_CLOSED {
                    continue;
                }
                if !Locked::<T>::get(id).is_zero() {
                    continue;
                }
                Self::remove_expiry_schedule(*id);
                Locked::<T>::remove(id);
                LockStateOf::<T>::remove(id);
                DisputedAt::<T>::remove(id);
                PayerOf::<T>::remove(id);
                cleaned.push(*id);
            }
            if !cleaned.is_empty() {
                Self::deposit_event(Event::Cleaned { ids: cleaned });
            }
            Ok(())
        }

    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// 函数级中文注释：每块处理最多 MaxExpiringPerBlock 个到期项。
        /// H-1修复：使用 ExpiringAt 索引，避免迭代所有 ExpiryOf
        /// 性能提升：O(N) -> O(1)，N = 总存储项数
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            let expiring_ids = ExpiringAt::<T>::take(n);
            let total = expiring_ids.len() as u32;

            for id in expiring_ids.iter() {
                if LockStateOf::<T>::get(id) == STATE_DISPUTED {
                    // 检查争议是否超时
                    let timed_out = DisputedAt::<T>::get(id)
                        .map(|at| n >= at.saturating_add(T::MaxDisputeDuration::get()))
                        .unwrap_or(true);

                    if !timed_out {
                        // 仍在争议窗口内，重调度到 1 天后
                        let base_recheck = n.saturating_add(14400u32.into());
                        if !Self::try_schedule_expiry_at(*id, base_recheck) {
                            ExpiryOf::<T>::remove(id);
                            log::warn!(target: "escrow", "Failed to reschedule disputed escrow id={}", id);
                        }
                        continue;
                    }
                    LockStateOf::<T>::insert(id, STATE_LOCKED);
                    DisputedAt::<T>::remove(id);
                }

                match T::ExpiryPolicy::on_expire(*id) {
                    Ok(ExpiryAction::ReleaseAll(to)) => {
                        match <Self as Escrow<T::AccountId, BalanceOf<T>>>::release_all(*id, &to) {
                            Ok(_) => {
                                Self::deposit_event(Event::Expired { id: *id, action: 0 });
                                T::Observer::on_expired(*id, 0);
                            }
                            Err(_) => {
                                log::warn!(target: "escrow", "Expiry release failed for id={}", id);
                            }
                        }
                    }
                    Ok(ExpiryAction::RefundAll(to)) => {
                        match <Self as Escrow<T::AccountId, BalanceOf<T>>>::refund_all(*id, &to) {
                            Ok(_) => {
                                Self::deposit_event(Event::Expired { id: *id, action: 1 });
                                T::Observer::on_expired(*id, 1);
                            }
                            Err(_) => {
                                log::warn!(target: "escrow", "Expiry refund failed for id={}", id);
                            }
                        }
                    }
                    _ => {
                        Self::deposit_event(Event::Expired { id: *id, action: 2 });
                        T::Observer::on_expired(*id, 2);
                    }
                }
                ExpiryOf::<T>::remove(id);
            }

            let per_item = Weight::from_parts(50_000_000, 3_500);
            let base = Weight::from_parts(5_000_000, 1_000);
            base.saturating_add(per_item.saturating_mul(total as u64))
        }
    }
}
