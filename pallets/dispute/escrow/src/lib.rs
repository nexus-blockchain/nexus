#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

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
    pub trait EscrowObserver<AccountId, Balance, BlockNumber> {
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
    impl<AccountId, Balance, BlockNumber> EscrowObserver<AccountId, Balance, BlockNumber> for () {
        fn on_released(_id: u64, _to: &AccountId, _amount: Balance) {}
        fn on_refunded(_id: u64, _to: &AccountId, _amount: Balance) {}
        fn on_expired(_id: u64, _action: u8) {}
        fn on_disputed(_id: u64) {}
        fn on_force_action(_id: u64, _action: u8) {}
    }

    /// 🆕 F9: Token 托管处理器接口
    /// 函数级详细中文注释：由 runtime 实现，处理 Entity Token 类型的托管操作
    pub trait TokenEscrowHandler<AccountId> {
        /// 从付款人锁定 token 到托管
        fn lock_token(payer: &AccountId, entity_id: u64, escrow_id: u64, amount: u128) -> DispatchResult;
        /// 从托管释放 token 给收款人
        fn release_token(entity_id: u64, escrow_id: u64, to: &AccountId, amount: u128) -> DispatchResult;
        /// 从托管退款 token 给付款人
        fn refund_token(entity_id: u64, escrow_id: u64, to: &AccountId, amount: u128) -> DispatchResult;
        /// 查询 token 托管余额
        fn token_amount(entity_id: u64, escrow_id: u64) -> u128;
    }

    /// 空实现：不支持 Token 托管时使用
    impl<AccountId> TokenEscrowHandler<AccountId> for () {
        fn lock_token(_payer: &AccountId, _entity_id: u64, _escrow_id: u64, _amount: u128) -> DispatchResult {
            Err(DispatchError::Other("TokenEscrow not supported"))
        }
        fn release_token(_entity_id: u64, _escrow_id: u64, _to: &AccountId, _amount: u128) -> DispatchResult {
            Err(DispatchError::Other("TokenEscrow not supported"))
        }
        fn refund_token(_entity_id: u64, _escrow_id: u64, _to: &AccountId, _amount: u128) -> DispatchResult {
            Err(DispatchError::Other("TokenEscrow not supported"))
        }
        fn token_amount(_entity_id: u64, _escrow_id: u64) -> u128 { 0 }
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
        /// 🆕 F9: Token 托管处理器（处理 Entity Token 类型资产）
        type TokenHandler: TokenEscrowHandler<Self::AccountId>;
        /// 🆕 F10: 托管状态变更观察者（通知业务模块）
        type Observer: EscrowObserver<Self::AccountId, BalanceOf<Self>, BlockNumberFor<Self>>;
        /// 🆕 F8: 每次清理调用最大条目数
        #[pallet::constant]
        type MaxCleanupPerCall: Get<u32>;
        /// 🆕 M2修复: 权重信息
        type WeightInfo: crate::weights::WeightInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// 简单托管：订单 -> 锁定余额
    #[pallet::storage]
    pub type Locked<T: Config> = StorageMap<_, Blake2_128Concat, u64, BalanceOf<T>, ValueQuery>;

    /// 函数级中文注释：全局暂停开关（应急止血）。
    /// - 为 true 时，除 AdminOrigin 外的变更性操作将被拒绝。
    #[pallet::storage]
    pub type Paused<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// 函数级中文注释：托管状态：0=Locked,1=Disputed,2=Resolved,3=Closed。
    /// - Disputed 状态下仅允许仲裁决议接口处理；
    /// - Closed 表示已全部结清，不再接受出金操作。
    #[pallet::storage]
    pub type LockStateOf<T: Config> = StorageMap<_, Blake2_128Concat, u64, u8, ValueQuery>;

    /// 函数级中文注释：幂等 nonce：记录每个 id 的最新 nonce，避免重复 lock 被重放。
    #[pallet::storage]
    pub type LockNonces<T: Config> = StorageMap<_, Blake2_128Concat, u64, u64, ValueQuery>;

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

    /// 🆕 F9: Token 托管余额（(entity_id, escrow_id) -> amount）
    #[pallet::storage]
    pub type TokenLocked<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, u64,
        u128,
        ValueQuery,
    >;

    /// 🆕 F9: Token 托管状态（(entity_id, escrow_id) -> state）
    #[pallet::storage]
    pub type TokenLockStateOf<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, u64,
        u8,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 🆕 F2: 锁定到托管账户（含 payer 信息）
        Locked { id: u64, payer: T::AccountId, amount: BalanceOf<T> },
        /// 从托管部分划转（多次分账）
        Transfered {
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
        /// 🆕 F9: Token 托管锁定
        TokenLocked {
            entity_id: u64,
            escrow_id: u64,
            payer: T::AccountId,
            amount: u128,
        },
        /// 🆕 F9: Token 托管释放
        TokenReleased {
            entity_id: u64,
            escrow_id: u64,
            to: T::AccountId,
            amount: u128,
        },
        /// 🆕 F9: Token 托管退款
        TokenRefunded {
            entity_id: u64,
            escrow_id: u64,
            to: T::AccountId,
            amount: u128,
        },
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
        /// 🆕 F8: 托管未关闭，不可清理
        NotClosed,
        /// 🆕 F9: Token 托管操作失败
        TokenEscrowError,
    }

    /// 函数级中文注释：到期处理策略接口（由 runtime 实现）。
    pub trait ExpiryPolicy<AccountId, BlockNumber> {
        /// 返回到期应执行的动作：ReleaseAll(to) | RefundAll(to) | Noop。
        fn on_expire(id: u64) -> Result<ExpiryAction<AccountId>, sp_runtime::DispatchError>;
        /// 返回当前块（用于调度比较）。
        fn now() -> BlockNumber;
    }

    /// 函数级中文注释：到期动作枚举。
    pub enum ExpiryAction<AccountId> {
        ReleaseAll(AccountId),
        RefundAll(AccountId),
        Noop,
    }

    impl<AccountId> ExpiryAction<AccountId> {
        /// 用于日志/权重估算
        pub fn is_noop(&self) -> bool {
            matches!(self, Self::Noop)
        }
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
    }

    impl<T: Config> Escrow<T::AccountId, BalanceOf<T>> for Pallet<T> {
        fn escrow_account() -> T::AccountId {
            Self::account()
        }
        fn lock_from(payer: &T::AccountId, id: u64, amount: BalanceOf<T>) -> DispatchResult {
            // 函数级详细中文注释：从指定付款人向托管账户划转指定金额，并累加到 Locked[id]
            // 🆕 L4修复: 拒绝零金额锁定（浪费事件和存储操作）
            ensure!(!amount.is_zero(), Error::<T>::Insufficient);
            // 🆕 C2修复: 拒绝已关闭的托管重新注入资金
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            // 🆕 M1-R2修复: trait 层也拒绝争议中的托管追加锁定
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            let escrow = Self::account();
            T::Currency::transfer(payer, &escrow, amount, ExistenceRequirement::KeepAlive)
                .map_err(|_| Error::<T>::Insufficient)?;
            let cur = Locked::<T>::get(id);
            Locked::<T>::insert(id, cur.saturating_add(amount));
            // 🆕 F2: Locked 事件包含 payer 信息
            Self::deposit_event(Event::Locked { id, payer: payer.clone(), amount });
            Ok(())
        }
        fn transfer_from_escrow(
            id: u64,
            to: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            // 🆕 C1修复: 检查状态 - 争议中(1)禁止操作，已关闭(3)禁止操作
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
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
            Self::deposit_event(Event::Transfered {
                id,
                to: to.clone(),
                amount,
                remaining: new,
            });
            Ok(())
        }
        fn release_all(id: u64, to: &T::AccountId) -> DispatchResult {
            // 🆕 P2修复: 检查状态
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            
            let amount = Locked::<T>::take(id);
            ensure!(!amount.is_zero(), Error::<T>::NoLock);
            
            let escrow = Self::account();
            T::Currency::transfer(&escrow, to, amount, ExistenceRequirement::AllowDeath)
                .map_err(|_| Error::<T>::NoLock)?;
            
            LockStateOf::<T>::insert(id, 3u8);
            
            Self::deposit_event(Event::Released {
                id,
                to: to.clone(),
                amount,
            });
            // 🆕 F10: 通知观察者
            T::Observer::on_released(id, to, amount);
            Ok(())
        }
        fn refund_all(id: u64, to: &T::AccountId) -> DispatchResult {
            // 🆕 P2修复: 检查状态
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            
            let amount = Locked::<T>::take(id);
            ensure!(!amount.is_zero(), Error::<T>::NoLock);
            
            let escrow = Self::account();
            T::Currency::transfer(&escrow, to, amount, ExistenceRequirement::AllowDeath)
                .map_err(|_| Error::<T>::NoLock)?;
            
            LockStateOf::<T>::insert(id, 3u8);
            
            Self::deposit_event(Event::Refunded {
                id,
                to: to.clone(),
                amount,
            });
            // 🆕 F10: 通知观察者
            T::Observer::on_refunded(id, to, amount);
            Ok(())
        }
        /// 🆕 F1: 部分退款
        fn refund_partial(id: u64, to: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            // 🆕 M1-R3修复: 拒绝零金额（防止幻影事件和冗余存储写入）
            ensure!(!amount.is_zero(), Error::<T>::Insufficient);
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
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
                LockStateOf::<T>::insert(id, 3u8);
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
        /// 🆕 F3: 部分释放（里程碑式）
        fn release_partial(id: u64, to: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            // 🆕 M1-R3修复: 拒绝零金额（防止幻影事件和冗余存储写入）
            ensure!(!amount.is_zero(), Error::<T>::Insufficient);
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
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
                LockStateOf::<T>::insert(id, 3u8);
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
            let cur = Locked::<T>::get(id);
            ensure!(!cur.is_zero(), Error::<T>::NoLock);
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            // 🆕 M1修复: 防止重复 dispute 重置时间戳
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            LockStateOf::<T>::insert(id, 1u8);
            // 🆕 F4: 记录争议时间戳
            let now = <frame_system::Pallet<T>>::block_number();
            DisputedAt::<T>::insert(id, now);
            // 🆕 F5: 扩展的争议事件
            Self::deposit_event(Event::Disputed {
                id,
                reason: 0,
                detail: BoundedVec::default(),
                at: now,
            });
            // 🆕 F10: 通知观察者
            T::Observer::on_disputed(id);
            Ok(())
        }
        fn set_resolved(id: u64) -> DispatchResult {
            let state = LockStateOf::<T>::get(id);
            ensure!(state == 1u8, Error::<T>::NotInDispute);
            LockStateOf::<T>::insert(id, 0u8);
            // 🆕 F4: 清理争议时间戳
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
            // 🆕 M2修复: 争议期间禁止分账（需先 set_resolved）
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            
            let total = Locked::<T>::take(id);
            ensure!(!total.is_zero(), Error::<T>::NoLock);
            
            let escrow = Self::account();
            
            let release_amount = sp_runtime::Permill::from_parts((bps as u32) * 100)
                .mul_floor(total);
            let refund_amount = total.saturating_sub(release_amount);
            
            // 🆕 H1-R2修复: 第一笔转账用 KeepAlive 防止 ED dust，最后一笔用 AllowDeath
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
            
            LockStateOf::<T>::insert(id, 3u8);
            
            Self::deposit_event(Event::PartialSplit {
                id,
                release_to: release_to.clone(),
                release_amount,
                refund_to: refund_to.clone(),
                refund_amount,
            });
            // 🆕 M3-R3修复: 通知观察者分账释放和退款
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
            // 函数级详细中文注释（安全）：仅允许 AuthorizedOrigin | Root 调用，防止冒用 payer 盗划资金；支持全局暂停。
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            // 🆕 EH3修复: 已关闭的托管不允许通过 extrinsic 重新打开
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            // 🆕 E4修复: 争议中的托管禁止追加锁定（防止绕过争议保护）
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::lock_from(&payer, id, amount)
        }
        /// 释放：将托管金额转给收款人
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::release())]
        pub fn release(origin: OriginFor<T>, id: u64, to: T::AccountId) -> DispatchResult {
            // 函数级详细中文注释（安全）：仅 AuthorizedOrigin | Root；暂停时拒绝；争议状态下拒绝普通释放。
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            // 🆕 EM2修复: 争议中使用正确的错误码
            ensure!(LockStateOf::<T>::get(id) != 1u8, Error::<T>::DisputeActive);
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::release_all(id, &to)
        }
        /// 退款：退回付款人
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::refund())]
        pub fn refund(origin: OriginFor<T>, id: u64, to: T::AccountId) -> DispatchResult {
            // 函数级详细中文注释（安全）：仅 AuthorizedOrigin | Root；暂停时拒绝；争议状态下拒绝普通退款。
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            // 🆕 EM2修复: 争议中使用正确的错误码
            ensure!(LockStateOf::<T>::get(id) != 1u8, Error::<T>::DisputeActive);
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::refund_all(id, &to)
        }

        /// 函数级详细中文注释：幂等锁定（带 nonce）。相同 id 下 nonce 必须严格递增；否则忽略以防重放。
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::lock_with_nonce())]
        pub fn lock_with_nonce(
            origin: OriginFor<T>,
            id: u64,
            payer: T::AccountId,
            amount: BalanceOf<T>,
            nonce: u64,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            let last = LockNonces::<T>::get(id);
            if nonce <= last {
                return Ok(());
            } // 幂等：忽略重放
            LockNonces::<T>::insert(id, nonce);
            // 🆕 EH3修复: 已关闭的托管不允许通过 nonce 重新打开
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            // 🆕 E4修复: 争议中的托管禁止追加锁定（防止绕过争议保护）
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::lock_from(&payer, id, amount)
        }

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
            // 🆕 EM2修复: 争议中使用正确的错误码
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            // 🆕 L1-R3修复: 已关闭的托管不允许再次释放
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
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
                Self::deposit_event(Event::Transfered {
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
                LockStateOf::<T>::insert(id, 3u8);
            }
            Ok(())
        }

        /// 🆕 F5: 进入争议（仅授权/Root），支持 BoundedVec 争议详情
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::dispute())]
        pub fn dispute(
            origin: OriginFor<T>,
            id: u64,
            reason: u16,
            detail: BoundedVec<u8, T::MaxReasonLen>,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            ensure!(!Locked::<T>::get(id).is_zero(), Error::<T>::NoLock);
            // 🆕 M1修复: 检查状态 — 已关闭/已争议均拒绝
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            LockStateOf::<T>::insert(id, 1u8);
            // 🆕 F4: 记录争议时间戳
            let now = <frame_system::Pallet<T>>::block_number();
            DisputedAt::<T>::insert(id, now);
            Self::deposit_event(Event::Disputed { id, reason, detail, at: now });
            // 🆕 F10: 通知观察者
            T::Observer::on_disputed(id);
            Ok(())
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
            // 🆕 H1修复: 仲裁裁决仅允许在争议状态下执行
            ensure!(LockStateOf::<T>::get(id) == 1u8, Error::<T>::NotInDispute);
            // 解除争议状态，允许 release_all 执行
            LockStateOf::<T>::insert(id, 0u8);
            // 🆕 M4-R2修复: 清理争议时间戳
            DisputedAt::<T>::remove(id);
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
            // 🆕 H1修复: 仲裁裁决仅允许在争议状态下执行
            ensure!(LockStateOf::<T>::get(id) == 1u8, Error::<T>::NotInDispute);
            // 解除争议状态，允许 refund_all 执行
            LockStateOf::<T>::insert(id, 0u8);
            // 🆕 M4-R2修复: 清理争议时间戳
            DisputedAt::<T>::remove(id);
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::refund_all(id, &to)?;
            Self::deposit_event(Event::DecisionApplied { id, decision: 1 });
            Ok(())
        }

        /// 函数级中文注释：仲裁决议-按 bps 部分释放，其余退款给 refund_to。
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
            // 🆕 H1修复: 仲裁裁决仅允许在争议状态下执行
            ensure!(LockStateOf::<T>::get(id) == 1u8, Error::<T>::NotInDispute);
            ensure!(bps <= 10_000, Error::<T>::Insufficient);
            let cur = Locked::<T>::get(id);
            ensure!(!cur.is_zero(), Error::<T>::NoLock);
            // 计算按 bps 的释放金额：floor(cur * bps / 10000)
            let cur_u128: u128 =
                sp_runtime::traits::SaturatedConversion::saturated_into::<u128>(cur);
            let rel_u128 = (cur_u128.saturating_mul(bps as u128)) / 10_000u128;
            let rel_amt: BalanceOf<T> =
                sp_runtime::traits::SaturatedConversion::saturated_into::<BalanceOf<T>>(rel_u128);
            // 解除争议状态，允许内部函数执行
            LockStateOf::<T>::insert(id, 0u8);
            // 🆕 M4-R2修复: 清理争议时间戳
            DisputedAt::<T>::remove(id);
            if !rel_amt.is_zero() {
                <Self as Escrow<T::AccountId, BalanceOf<T>>>::transfer_from_escrow(
                    id,
                    &release_to,
                    rel_amt,
                )?;
                // 🆕 M4-R3修复: transfer_from_escrow 不触发 Observer，手动通知
                T::Observer::on_released(id, &release_to, rel_amt);
            }
            let after = Locked::<T>::get(id);
            if !after.is_zero() {
                <Self as Escrow<T::AccountId, BalanceOf<T>>>::refund_all(id, &refund_to)?;
            } else {
                // transfer_from_escrow 已转完全部，手动设置 Closed
                LockStateOf::<T>::insert(id, 3u8);
            }
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

        /// 函数级中文注释：安排到期处理（仅 AuthorizedOrigin）。当处于 Disputed 时不生效。
        /// H-1修复：同时更新 ExpiringAt 索引
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::schedule_expiry())]
        pub fn schedule_expiry(
            origin: OriginFor<T>,
            id: u64,
            at: BlockNumberFor<T>,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            if LockStateOf::<T>::get(id) == 1u8 {
                return Ok(());
            }
            
            // 如果已有到期时间，先从旧索引中移除
            if let Some(old_at) = ExpiryOf::<T>::get(id) {
                ExpiringAt::<T>::mutate(old_at, |ids| {
                    if let Some(pos) = ids.iter().position(|&x| x == id) {
                        ids.swap_remove(pos);
                    }
                });
            }
            
            // 更新到期时间
            ExpiryOf::<T>::insert(id, at);
            
            // 添加到新的索引
            // 🆕 EM1修复: 使用专用错误码
            ExpiringAt::<T>::try_mutate(at, |ids| -> DispatchResult {
                ids.try_push(id).map_err(|_| Error::<T>::ExpiringAtFull)?;
                Ok(())
            })?;
            
            Self::deposit_event(Event::ExpiryScheduled { id, at });
            Ok(())
        }

        /// 函数级中文注释：取消到期处理（仅 AuthorizedOrigin）。
        /// H-1修复：同时从 ExpiringAt 索引中移除
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::cancel_expiry())]
        pub fn cancel_expiry(origin: OriginFor<T>, id: u64) -> DispatchResult {
            Self::ensure_auth(origin)?;
            
            // 从索引中移除
            if let Some(at) = ExpiryOf::<T>::get(id) {
                ExpiringAt::<T>::mutate(at, |ids| {
                    if let Some(pos) = ids.iter().position(|&x| x == id) {
                        ids.swap_remove(pos);
                    }
                });
            }
            
            ExpiryOf::<T>::remove(id);
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
            LockStateOf::<T>::insert(id, 3u8);
            // 清理争议时间戳（如果存在）
            DisputedAt::<T>::remove(id);
            // 🆕 M2-R3修复: 清理到期调度索引（防止 on_initialize 空转）
            if let Some(at) = ExpiryOf::<T>::take(id) {
                ExpiringAt::<T>::mutate(at, |ids| {
                    if let Some(pos) = ids.iter().position(|&x| x == id) {
                        ids.swap_remove(pos);
                    }
                });
            }
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
            LockStateOf::<T>::insert(id, 3u8);
            DisputedAt::<T>::remove(id);
            // 🆕 M2-R3修复: 清理到期调度索引（防止 on_initialize 空转）
            if let Some(at) = ExpiryOf::<T>::take(id) {
                ExpiringAt::<T>::mutate(at, |ids| {
                    if let Some(pos) = ids.iter().position(|&x| x == id) {
                        ids.swap_remove(pos);
                    }
                });
            }
            Self::deposit_event(Event::ForceAction { id, action: 1, to: to.clone(), amount });
            T::Observer::on_force_action(id, 1);
            Ok(())
        }

        /// 🆕 F1: 部分退款 extrinsic（仅 AuthorizedOrigin）
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::refund_partial())]
        pub fn refund_partial(
            origin: OriginFor<T>,
            id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            ensure!(LockStateOf::<T>::get(id) != 1u8, Error::<T>::DisputeActive);
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::refund_partial(id, &to, amount)
        }

        /// 🆕 F3: 部分释放 extrinsic（里程碑式，仅 AuthorizedOrigin）
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::release_partial())]
        pub fn release_partial(
            origin: OriginFor<T>,
            id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            ensure!(LockStateOf::<T>::get(id) != 1u8, Error::<T>::DisputeActive);
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::release_partial(id, &to, amount)
        }

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
                let state = LockStateOf::<T>::get(id);
                // 🆕 M3修复: 跳过非 Closed 的 id，避免整体回滚
                if state != 3u8 {
                    continue;
                }
                // 确保余额为零
                let amount = Locked::<T>::get(id);
                if !amount.is_zero() {
                    continue;
                }
                // 🆕 M2-R2修复: 清理 ExpiringAt 索引中的残留引用
                if let Some(at) = ExpiryOf::<T>::get(id) {
                    ExpiringAt::<T>::mutate(at, |expiring_ids| {
                        if let Some(pos) = expiring_ids.iter().position(|&x| x == *id) {
                            expiring_ids.swap_remove(pos);
                        }
                    });
                }
                // 清理所有相关存储
                Locked::<T>::remove(id);
                LockStateOf::<T>::remove(id);
                LockNonces::<T>::remove(id);
                ExpiryOf::<T>::remove(id);
                DisputedAt::<T>::remove(id);
                cleaned.push(*id);
            }
            if !cleaned.is_empty() {
                Self::deposit_event(Event::Cleaned { ids: cleaned });
            }
            Ok(())
        }

        /// 🆕 F9: Token 托管锁定
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::token_lock())]
        pub fn token_lock(
            origin: OriginFor<T>,
            entity_id: u64,
            escrow_id: u64,
            payer: T::AccountId,
            amount: u128,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            let state = TokenLockStateOf::<T>::get(entity_id, escrow_id);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            // 🆕 L5-R2修复: 拒绝零金额锁定
            ensure!(amount > 0, Error::<T>::Insufficient);
            T::TokenHandler::lock_token(&payer, entity_id, escrow_id, amount)
                .map_err(|_| Error::<T>::TokenEscrowError)?;
            let cur = TokenLocked::<T>::get(entity_id, escrow_id);
            TokenLocked::<T>::insert(entity_id, escrow_id, cur.saturating_add(amount));
            Self::deposit_event(Event::TokenLocked {
                entity_id,
                escrow_id,
                payer: payer.clone(),
                amount,
            });
            Ok(())
        }

        /// 🆕 F9: Token 托管释放
        #[pallet::call_index(18)]
        #[pallet::weight(T::WeightInfo::token_release())]
        pub fn token_release(
            origin: OriginFor<T>,
            entity_id: u64,
            escrow_id: u64,
            to: T::AccountId,
            amount: u128,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            let state = TokenLockStateOf::<T>::get(entity_id, escrow_id);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            let cur = TokenLocked::<T>::get(entity_id, escrow_id);
            // 🆕 L5-R2修复: 拒绝零金额释放
            ensure!(amount > 0 && amount <= cur, Error::<T>::Insufficient);
            T::TokenHandler::release_token(entity_id, escrow_id, &to, amount)
                .map_err(|_| Error::<T>::TokenEscrowError)?;
            let new = cur.saturating_sub(amount);
            // 🆕 L5-R2修复: 避免先 insert 再 remove 的冗余写入
            if new == 0 {
                TokenLocked::<T>::remove(entity_id, escrow_id);
                TokenLockStateOf::<T>::insert(entity_id, escrow_id, 3u8);
            } else {
                TokenLocked::<T>::insert(entity_id, escrow_id, new);
            }
            Self::deposit_event(Event::TokenReleased {
                entity_id,
                escrow_id,
                to: to.clone(),
                amount,
            });
            Ok(())
        }

        /// 🆕 F9: Token 托管退款
        #[pallet::call_index(19)]
        #[pallet::weight(T::WeightInfo::token_refund())]
        pub fn token_refund(
            origin: OriginFor<T>,
            entity_id: u64,
            escrow_id: u64,
            to: T::AccountId,
            amount: u128,
        ) -> DispatchResult {
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            let state = TokenLockStateOf::<T>::get(entity_id, escrow_id);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            let cur = TokenLocked::<T>::get(entity_id, escrow_id);
            // 🆕 L5-R2修复: 拒绝零金额退款
            ensure!(amount > 0 && amount <= cur, Error::<T>::Insufficient);
            T::TokenHandler::refund_token(entity_id, escrow_id, &to, amount)
                .map_err(|_| Error::<T>::TokenEscrowError)?;
            let new = cur.saturating_sub(amount);
            // 🆕 L5-R2修复: 避免先 insert 再 remove 的冗余写入
            if new == 0 {
                TokenLocked::<T>::remove(entity_id, escrow_id);
                TokenLockStateOf::<T>::insert(entity_id, escrow_id, 3u8);
            } else {
                TokenLocked::<T>::insert(entity_id, escrow_id, new);
            }
            Self::deposit_event(Event::TokenRefunded {
                entity_id,
                escrow_id,
                to: to.clone(),
                amount,
            });
            Ok(())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// 函数级中文注释：每块处理最多 MaxExpiringPerBlock 个到期项。
        /// H-1修复：使用 ExpiringAt 索引，避免迭代所有 ExpiryOf
        /// 性能提升：O(N) -> O(1)，N = 总存储项数
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            // 直接获取当前块到期的项，O(1) 复杂度
            let expiring_ids = ExpiringAt::<T>::take(n);
            let total = expiring_ids.len() as u32;
            
            for id in expiring_ids.iter() {
                // 🆕 H-NEW-2修复: 争议中的项重新调度到未来块，而非丢弃
                if LockStateOf::<T>::get(id) == 1u8 {
                    // 🆕 H2修复: 重新调度到 1 天后，失败时尝试相邻块（最多 10 次）
                    let base_recheck = n.saturating_add(14400u32.into());
                    let mut rescheduled = false;
                    for offset in 0u32..10 {
                        let target = base_recheck.saturating_add(offset.into());
                        if ExpiringAt::<T>::try_mutate(target, |ids| {
                            ids.try_push(*id).map_err(|_| ())
                        }).is_ok() {
                            ExpiryOf::<T>::insert(id, target);
                            rescheduled = true;
                            break;
                        }
                    }
                    if !rescheduled {
                        // 🆕 L2-R3修复: 清理 ExpiryOf 避免指向已不存在的 ExpiringAt 条目
                        ExpiryOf::<T>::remove(id);
                        log::warn!(target: "escrow", "Failed to reschedule disputed escrow id={}", id);
                    }
                    continue;
                }
                
                // 🆕 H3修复: 仅在成功时才更新状态，失败时记录日志
                match T::ExpiryPolicy::on_expire(*id) {
                    Ok(ExpiryAction::ReleaseAll(to)) => {
                        match <Self as Escrow<T::AccountId, BalanceOf<T>>>::release_all(*id, &to) {
                            Ok(_) => {
                                // release_all 内部已设置 Closed(3)
                                Self::deposit_event(Event::Expired { id: *id, action: 0 });
                            }
                            Err(_) => {
                                log::warn!(target: "escrow", "Expiry release failed for id={}", id);
                            }
                        }
                    }
                    Ok(ExpiryAction::RefundAll(to)) => {
                        match <Self as Escrow<T::AccountId, BalanceOf<T>>>::refund_all(*id, &to) {
                            Ok(_) => {
                                // refund_all 内部已设置 Closed(3)
                                Self::deposit_event(Event::Expired { id: *id, action: 1 });
                            }
                            Err(_) => {
                                log::warn!(target: "escrow", "Expiry refund failed for id={}", id);
                            }
                        }
                    }
                    _ => {
                        Self::deposit_event(Event::Expired { id: *id, action: 2 });
                    }
                }
                
                // 清理到期记录
                ExpiryOf::<T>::remove(id);
            }
            
            // 🆕 E1修复: 每项到期处理涉及 LockStateOf(r) + ExpiryPolicy(r) + Currency::transfer(r+w)
            // + Locked(rw) + LockStateOf(w) + ExpiryOf(w) + Event = ~3r+4w ≈ 50M ref_time/项
            let per_item = Weight::from_parts(50_000_000, 3_500);
            let base = Weight::from_parts(5_000_000, 1_000); // ExpiringAt::take 开销
            base.saturating_add(per_item.saturating_mul(total as u64))
        }
    }
}
