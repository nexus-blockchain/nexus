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
        /// 查询当前托管余额
        fn amount_of(id: u64) -> Balance;
        /// 按比例分账：bps/10000 给 release_to，剩余给 refund_to
        /// 函数级详细中文注释：用于仲裁部分裁决场景
        /// - bps: 基点（10000 = 100%），表示 release_to 获得的比例
        /// - release_to: 获得 bps/10000 比例的账户
        /// - refund_to: 获得剩余比例的账户
        fn split_partial(id: u64, release_to: &AccountId, refund_to: &AccountId, bps: u16) -> DispatchResult;
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

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 锁定到托管账户（listing_id 或 order_id 作为 id）
        Locked { id: u64, amount: BalanceOf<T> },
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
        /// 进入争议
        Disputed { id: u64, reason: u16 },
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
            // - 余额校验：Currency::transfer 失败即返回 Error::Insufficient
            // - 原子性：任意一步失败会使外层事务回滚，避免脏写
            // 🆕 C2修复: 拒绝已关闭的托管重新注入资金
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            let escrow = Self::account();
            T::Currency::transfer(payer, &escrow, amount, ExistenceRequirement::KeepAlive)
                .map_err(|_| Error::<T>::Insufficient)?;
            let cur = Locked::<T>::get(id);
            Locked::<T>::insert(id, cur.saturating_add(amount));
            Self::deposit_event(Event::Locked { id, amount });
            Ok(())
        }
        fn transfer_from_escrow(
            id: u64,
            to: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            // 函数级详细中文注释：从 Locked[id] 对应的托管余额中转出部分至目标账户
            // - 风险控制：禁止透支（amount 必须 ≤ 当前托管余额），避免逃逸
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
            T::Currency::transfer(&escrow, to, amount, ExistenceRequirement::KeepAlive)
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
            // 函数级详细中文注释：一次性释放全部托管余额给收款人
            // 🆕 P2修复: 检查状态 - 争议中(1)禁止操作，已关闭(3)禁止重复操作
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            
            let amount = Locked::<T>::take(id);
            ensure!(!amount.is_zero(), Error::<T>::NoLock);
            
            let escrow = Self::account();
            // 🆕 H1修复: 全额释放使用 AllowDeath，避免小额 dust 永久卡住
            T::Currency::transfer(&escrow, to, amount, ExistenceRequirement::AllowDeath)
                .map_err(|_| Error::<T>::NoLock)?;
            
            // 🆕 P2修复: 更新状态为 Closed(3)
            LockStateOf::<T>::insert(id, 3u8);
            
            Self::deposit_event(Event::Released {
                id,
                to: to.clone(),
                amount,
            });
            Ok(())
        }
        fn refund_all(id: u64, to: &T::AccountId) -> DispatchResult {
            // 函数级详细中文注释：一次性退回全部托管余额给收款人
            // 🆕 P2修复: 检查状态 - 争议中(1)禁止操作，已关闭(3)禁止重复操作
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 1u8, Error::<T>::DisputeActive);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            
            let amount = Locked::<T>::take(id);
            ensure!(!amount.is_zero(), Error::<T>::NoLock);
            
            let escrow = Self::account();
            // 🆕 H1修复: 全额退款使用 AllowDeath，避免小额 dust 永久卡住
            T::Currency::transfer(&escrow, to, amount, ExistenceRequirement::AllowDeath)
                .map_err(|_| Error::<T>::NoLock)?;
            
            // 🆕 P2修复: 更新状态为 Closed(3)
            LockStateOf::<T>::insert(id, 3u8);
            
            Self::deposit_event(Event::Refunded {
                id,
                to: to.clone(),
                amount,
            });
            Ok(())
        }
        fn amount_of(id: u64) -> BalanceOf<T> {
            Locked::<T>::get(id)
        }
        fn split_partial(
            id: u64,
            release_to: &T::AccountId,
            refund_to: &T::AccountId,
            bps: u16,
        ) -> DispatchResult {
            // 函数级详细中文注释：按比例分账
            // - bps: 基点（10000 = 100%），release_to 获得 bps/10000，refund_to 获得剩余
            // - 使用 Permill 进行安全的比例计算
            // 🆕 P2修复: 检查状态 - 已关闭(3)禁止重复操作（争议中允许分账裁决）
            let state = LockStateOf::<T>::get(id);
            ensure!(state != 3u8, Error::<T>::AlreadyClosed);
            
            let total = Locked::<T>::take(id);
            ensure!(!total.is_zero(), Error::<T>::NoLock);
            
            let escrow = Self::account();
            
            // 计算 release_to 获得的金额
            let release_amount = sp_runtime::Permill::from_parts((bps as u32) * 100)
                .mul_floor(total);
            let refund_amount = total.saturating_sub(release_amount);
            
            // 转账给 release_to
            if !release_amount.is_zero() {
                T::Currency::transfer(&escrow, release_to, release_amount, ExistenceRequirement::AllowDeath)
                    .map_err(|_| Error::<T>::Insufficient)?;
            }
            
            // 转账给 refund_to
            if !refund_amount.is_zero() {
                T::Currency::transfer(&escrow, refund_to, refund_amount, ExistenceRequirement::AllowDeath)
                    .map_err(|_| Error::<T>::Insufficient)?;
            }
            
            // 🆕 P2修复: 更新状态为 Closed(3)
            LockStateOf::<T>::insert(id, 3u8);
            
            Self::deposit_event(Event::PartialSplit {
                id,
                release_to: release_to.clone(),
                release_amount,
                refund_to: refund_to.clone(),
                refund_amount,
            });
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
            // 初始化状态为 Locked
            if LockStateOf::<T>::get(id) == 0u8 { /* 已是 Locked */
            } else {
                LockStateOf::<T>::insert(id, 0u8);
            }
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::lock_from(&payer, id, amount)
        }
        /// 释放：将托管金额转给收款人
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::release())]
        pub fn release(origin: OriginFor<T>, id: u64, to: T::AccountId) -> DispatchResult {
            // 函数级详细中文注释（安全）：仅 AuthorizedOrigin | Root；暂停时拒绝；争议状态下拒绝普通释放。
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            ensure!(LockStateOf::<T>::get(id) != 1u8, Error::<T>::NoLock);
            <Self as Escrow<T::AccountId, BalanceOf<T>>>::release_all(id, &to)
        }
        /// 退款：退回付款人
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::refund())]
        pub fn refund(origin: OriginFor<T>, id: u64, to: T::AccountId) -> DispatchResult {
            // 函数级详细中文注释（安全）：仅 AuthorizedOrigin | Root；暂停时拒绝；争议状态下拒绝普通退款。
            Self::ensure_auth(origin)?;
            Self::ensure_not_paused()?;
            ensure!(LockStateOf::<T>::get(id) != 1u8, Error::<T>::NoLock);
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
            if LockStateOf::<T>::get(id) == 0u8 { /* 已是 Locked */
            } else {
                LockStateOf::<T>::insert(id, 0u8);
            }
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
            ensure!(LockStateOf::<T>::get(id) != 1u8, Error::<T>::NoLock);
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
            }
            if cur.is_zero() {
                Locked::<T>::remove(id);
                LockStateOf::<T>::insert(id, 3u8);
            }
            Ok(())
        }

        /// 函数级中文注释：进入争议（仅授权/Root）。设置状态为 Disputed 并记录事件。
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::dispute())]
        pub fn dispute(origin: OriginFor<T>, id: u64, reason: u16) -> DispatchResult {
            Self::ensure_auth(origin)?;
            if Locked::<T>::get(id).is_zero() {
                return Err(Error::<T>::NoLock.into());
            }
            LockStateOf::<T>::insert(id, 1u8);
            Self::deposit_event(Event::Disputed { id, reason });
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
            // 🆕 H2修复: 仂裁前先解除争议状态，允许 release_all 执行
            // release_all 内部会设置 Closed(3)，不再覆盖为 Resolved(2)
            LockStateOf::<T>::insert(id, 0u8);
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
            // 🆕 H2修复: 仂裁前先解除争议状态，允许 refund_all 执行
            LockStateOf::<T>::insert(id, 0u8);
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
            ensure!(bps <= 10_000, Error::<T>::Insufficient);
            let cur = Locked::<T>::get(id);
            ensure!(!cur.is_zero(), Error::<T>::NoLock);
            // 计算按 bps 的释放金额：floor(cur * bps / 10000)
            let cur_u128: u128 =
                sp_runtime::traits::SaturatedConversion::saturated_into::<u128>(cur);
            let rel_u128 = (cur_u128.saturating_mul(bps as u128)) / 10_000u128;
            let rel_amt: BalanceOf<T> =
                sp_runtime::traits::SaturatedConversion::saturated_into::<BalanceOf<T>>(rel_u128);
            // 🆕 H2修复: 仂裁前先解除争议状态，允许内部函数执行
            LockStateOf::<T>::insert(id, 0u8);
            if !rel_amt.is_zero() {
                <Self as Escrow<T::AccountId, BalanceOf<T>>>::transfer_from_escrow(
                    id,
                    &release_to,
                    rel_amt,
                )?;
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

        /// 函数级中文注释：设置全局暂停（Admin）。
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::set_pause())]
        pub fn set_pause(origin: OriginFor<T>, paused: bool) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            Paused::<T>::put(paused);
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
            ExpiringAt::<T>::try_mutate(at, |ids| -> DispatchResult {
                ids.try_push(id).map_err(|_| Error::<T>::NoLock)?;
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
                // 跳过争议状态的订单
                if LockStateOf::<T>::get(id) == 1u8 {
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
            
            // 返回权重（每个到期项约 20_000 单位）
            Weight::from_parts(20_000u64.saturating_mul(total as u64), 0)
        }
    }
}
