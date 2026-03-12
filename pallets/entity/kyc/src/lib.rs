//! # 实体 KYC/AML 认证模块 (pallet-entity-kyc)
//!
//! ## 概述
//!
//! 本模块实现普通用户在 Entity 内的 KYC（了解你的客户）认证。
//! 同一账户在不同 Entity 拥有独立的 KYC 记录和状态。
//!
//! ## Per-Entity 认证模型
//!
//! - 用户向特定 Entity 提交 KYC 申请
//! - Entity 授权的 Provider 或 Entity Owner/Admin 审核
//! - 每个 (Entity, Account) 对拥有独立的 KYC 记录
//! - Entity 之间的 KYC 状态互不影响
//! - 升级申请独立存储在 UpgradeRequests，不覆盖已有 Approved 记录
//!
//! ## KYC 级别
//!
//! - **None**: 未认证
//! - **Basic**: 基础认证（邮箱/手机验证）
//! - **Standard**: 标准认证（身份证件）
//! - **Enhanced**: 增强认证（地址证明 + 资金来源）
//! - **Institutional**: 机构认证（企业文件 + 受益人）

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use pallet_entity_common::EntityProvider as _;
    use pallet_entity_common::OnKycStatusChange as _;
    use frame_support::{
        pallet_prelude::*,
        traits::Get,
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Saturating;

    // ==================== 类型定义 ====================

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum KycLevel {
        #[default]
        None,
        Basic,
        Standard,
        Enhanced,
        Institutional,
    }

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum KycStatus {
        #[default]
        NotSubmitted,
        Pending,
        Approved,
        Rejected,
        Expired,
        Revoked,
    }

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum RejectionReason {
        UnclearDocument,
        ExpiredDocument,
        InformationMismatch,
        SuspiciousActivity,
        SanctionedEntity,
        HighRiskCountry,
        ForgedDocument,
        TimedOut,
        Other,
    }

    impl KycLevel {
        pub fn as_u8(&self) -> u8 {
            match self {
                KycLevel::None => 0,
                KycLevel::Basic => 1,
                KycLevel::Standard => 2,
                KycLevel::Enhanced => 3,
                KycLevel::Institutional => 4,
            }
        }

        pub fn try_from_u8(v: u8) -> Option<Self> {
            match v {
                0 => Some(KycLevel::None),
                1 => Some(KycLevel::Basic),
                2 => Some(KycLevel::Standard),
                3 => Some(KycLevel::Enhanced),
                4 => Some(KycLevel::Institutional),
                _ => None,
            }
        }
    }

    /// 认证提供者（全局注册，account 由 storage key 提供）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxNameLen))]
    pub struct KycProvider<MaxNameLen: Get<u32>> {
        pub name: BoundedVec<u8, MaxNameLen>,
        pub max_level: KycLevel,
        pub suspended: bool,
    }

    pub type KycProviderOf<T> = KycProvider<
        <T as Config>::MaxProviderNameLength,
    >;

    /// 用户 KYC 记录（per-entity，account 由 storage key 提供）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxCidLen))]
    pub struct KycRecord<AccountId, BlockNumber, MaxCidLen: Get<u32>> {
        pub level: KycLevel,
        pub status: KycStatus,
        pub provider: Option<AccountId>,
        pub data_cid: Option<BoundedVec<u8, MaxCidLen>>,
        pub submitted_at: Option<BlockNumber>,
        pub verified_at: Option<BlockNumber>,
        pub expires_at: Option<BlockNumber>,
        pub rejection_reason: Option<RejectionReason>,
        pub rejection_details_cid: Option<BoundedVec<u8, MaxCidLen>>,
        pub country_code: Option<[u8; 2]>,
        pub risk_score: u8,
    }

    pub type KycRecordOf<T> = KycRecord<
        <T as frame_system::Config>::AccountId,
        BlockNumberFor<T>,
        <T as Config>::MaxCidLength,
    >;

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum KycAction {
        Submitted,
        Approved,
        Rejected,
        Revoked,
        Expired,
        Renewed,
        Cancelled,
        DataUpdated,
        DataPurged,
        ForceApproved,
        TimedOut,
    }

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct KycHistoryEntry<BlockNumber> {
        pub action: KycAction,
        pub level: KycLevel,
        pub block_number: BlockNumber,
    }

    pub type KycHistoryEntryOf<T> = KycHistoryEntry<BlockNumberFor<T>>;

    impl KycStatus {
        pub fn as_u8(&self) -> u8 {
            match self {
                KycStatus::NotSubmitted => 0,
                KycStatus::Pending => 1,
                KycStatus::Approved => 2,
                KycStatus::Rejected => 3,
                KycStatus::Expired => 4,
                KycStatus::Revoked => 5,
            }
        }
    }

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct EntityKycRequirement {
        pub min_level: KycLevel,
        pub mandatory: bool,
        pub grace_period: u32,
        pub allow_high_risk_countries: bool,
        pub max_risk_score: u8,
    }

    /// KYC 升级请求（独立于主记录，升级审核期间保留原有 Approved 状态）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxCidLen))]
    pub struct KycUpgradeRequest<BlockNumber, MaxCidLen: Get<u32>> {
        pub target_level: KycLevel,
        pub data_cid: BoundedVec<u8, MaxCidLen>,
        pub country_code: [u8; 2],
        pub submitted_at: BlockNumber,
    }

    pub type KycUpgradeRequestOf<T> = KycUpgradeRequest<
        BlockNumberFor<T>,
        <T as Config>::MaxCidLength,
    >;

    // ==================== 配置 ====================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        #[pallet::constant]
        type MaxCidLength: Get<u32>;

        #[pallet::constant]
        type MaxProviderNameLength: Get<u32>;

        #[pallet::constant]
        type MaxProviders: Get<u32>;

        #[pallet::constant]
        type BasicKycValidity: Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type StandardKycValidity: Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type EnhancedKycValidity: Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type InstitutionalKycValidity: Get<BlockNumberFor<Self>>;

        type AdminOrigin: frame_support::traits::EnsureOrigin<Self::RuntimeOrigin>;

        type EntityProvider: pallet_entity_common::EntityProvider<Self::AccountId>;

        #[pallet::constant]
        type MaxHistoryEntries: Get<u32>;

        #[pallet::constant]
        type PendingKycTimeout: Get<BlockNumberFor<Self>>;

        /// 单个 Provider 最多可被授权的 Entity 数量
        #[pallet::constant]
        type MaxAuthorizedEntities: Get<u32>;

        type OnKycStatusChange: pallet_entity_common::OnKycStatusChange<Self::AccountId>;

        type WeightInfo: WeightInfo;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // ==================== 存储项 ====================

    #[pallet::storage]
    pub type KycRecords<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,
        Blake2_128Concat,
        T::AccountId,
        KycRecordOf<T>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn providers)]
    pub type Providers<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        KycProviderOf<T>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn provider_count)]
    pub type ProviderCount<T: Config> = StorageValue<_, u32, ValueQuery>;

    #[pallet::storage]
    pub type EntityAuthorizedProviders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,
        Blake2_128Concat,
        T::AccountId,
        (),
    >;

    #[pallet::storage]
    #[pallet::getter(fn entity_requirements)]
    pub type EntityRequirements<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        EntityKycRequirement,
    >;

    #[pallet::storage]
    #[pallet::getter(fn high_risk_countries)]
    pub type HighRiskCountries<T: Config> = StorageValue<
        _,
        BoundedVec<[u8; 2], ConstU32<50>>,
        ValueQuery,
    >;

    #[pallet::storage]
    pub type KycHistory<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<KycHistoryEntryOf<T>, T::MaxHistoryEntries>,
        ValueQuery,
    >;

    #[pallet::storage]
    pub type PendingKycCount<T: Config> = StorageMap<_, Blake2_128Concat, u64, u32, ValueQuery>;

    #[pallet::storage]
    pub type ApprovedKycCount<T: Config> = StorageMap<_, Blake2_128Concat, u64, u32, ValueQuery>;

    /// KYC 升级请求（per-entity: entity_id × account → upgrade_request）
    #[pallet::storage]
    pub type UpgradeRequests<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,
        Blake2_128Concat,
        T::AccountId,
        KycUpgradeRequestOf<T>,
    >;

    /// Provider 被授权的 Entity 列表（反向索引，用于 remove_provider 有界清理）
    #[pallet::storage]
    pub type ProviderAuthorizedEntities<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, T::MaxAuthorizedEntities>,
        ValueQuery,
    >;

    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        KycSubmitted {
            entity_id: u64,
            account: T::AccountId,
            level: KycLevel,
        },
        KycApproved {
            entity_id: u64,
            account: T::AccountId,
            level: KycLevel,
            provider: T::AccountId,
            expires_at: BlockNumberFor<T>,
        },
        KycRejected {
            entity_id: u64,
            account: T::AccountId,
            level: KycLevel,
            reason: RejectionReason,
        },
        KycExpired {
            entity_id: u64,
            account: T::AccountId,
        },
        KycRevoked {
            entity_id: u64,
            account: T::AccountId,
            reason: RejectionReason,
        },
        ProviderRegistered {
            provider: T::AccountId,
            name: Vec<u8>,
        },
        ProviderRemoved {
            provider: T::AccountId,
        },
        EntityRequirementSet {
            entity_id: u64,
            min_level: KycLevel,
        },
        HighRiskCountriesUpdated {
            count: u32,
        },
        KycCancelled {
            entity_id: u64,
            account: T::AccountId,
        },
        ProviderUpdated {
            provider: T::AccountId,
        },
        ProviderSuspended {
            provider: T::AccountId,
        },
        ProviderResumed {
            provider: T::AccountId,
        },
        RiskScoreUpdated {
            entity_id: u64,
            account: T::AccountId,
            old_score: u8,
            new_score: u8,
        },
        KycForceApproved {
            entity_id: u64,
            account: T::AccountId,
            level: KycLevel,
            expires_at: BlockNumberFor<T>,
        },
        KycRenewed {
            entity_id: u64,
            account: T::AccountId,
            level: KycLevel,
            expires_at: BlockNumberFor<T>,
        },
        KycDataUpdated {
            entity_id: u64,
            account: T::AccountId,
        },
        KycDataPurged {
            entity_id: u64,
            account: T::AccountId,
        },
        EntityRequirementRemoved {
            entity_id: u64,
        },
        PendingKycTimedOut {
            entity_id: u64,
            account: T::AccountId,
        },
        ProviderKycsRevoked {
            entity_id: u64,
            provider: T::AccountId,
            count: u32,
            reason: RejectionReason,
        },
        ProviderAuthorized {
            entity_id: u64,
            provider: T::AccountId,
        },
        ProviderDeauthorized {
            entity_id: u64,
            provider: T::AccountId,
        },
        KycUpgradeRequested {
            entity_id: u64,
            account: T::AccountId,
            current_level: KycLevel,
            target_level: KycLevel,
        },
        KycUpgradeRejected {
            entity_id: u64,
            account: T::AccountId,
            target_level: KycLevel,
            reason: RejectionReason,
        },
        KycUpgradeCancelled {
            entity_id: u64,
            account: T::AccountId,
            target_level: KycLevel,
        },
        KycUpgradeTimedOut {
            entity_id: u64,
            account: T::AccountId,
            target_level: KycLevel,
        },
        EntityKycRevoked {
            entity_id: u64,
            account: T::AccountId,
            reason: RejectionReason,
            revoker: T::AccountId,
        },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        KycNotFound,
        KycAlreadyPending,
        KycAlreadyApproved,
        ProviderNotFound,
        ProviderAlreadyExists,
        CidTooLong,
        NameTooLong,
        MaxProvidersReached,
        InvalidKycStatus,
        InvalidKycLevel,
        ProviderLevelNotSupported,
        TooManyCountries,
        InvalidRiskScore,
        EmptyProviderName,
        EmptyDataCid,
        InvalidCountryCode,
        SelfApprovalNotAllowed,
        KycNotExpired,
        NotEntityOwnerOrAdmin,
        EntityNotFound,
        ProviderIsSuspended,
        ProviderNotSuspended,
        NothingToUpdate,
        EntityLocked,
        KycNotRenewable,
        KycDataCannotBePurged,
        RequirementNotFound,
        PendingNotTimedOut,
        ProviderMismatch,
        EmptyAccountList,
        ProviderNotAuthorized,
        ProviderAlreadyAuthorized,
        EntityNotActive,
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 向指定 Entity 提交 KYC 认证申请（或升级请求）
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::submit_kyc())]
        pub fn submit_kyc(
            origin: OriginFor<T>,
            entity_id: u64,
            level: KycLevel,
            data_cid: Vec<u8>,
            country_code: [u8; 2],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::ensure_entity_operable(entity_id)?;
            ensure!(level != KycLevel::None, Error::<T>::InvalidKycLevel);
            ensure!(!data_cid.is_empty(), Error::<T>::EmptyDataCid);
            ensure!(
                country_code[0].is_ascii_uppercase() && country_code[1].is_ascii_uppercase(),
                Error::<T>::InvalidCountryCode
            );
            ensure!(
                !UpgradeRequests::<T>::contains_key(entity_id, &who),
                Error::<T>::KycAlreadyPending
            );

            let data_bounded: BoundedVec<u8, T::MaxCidLength> =
                data_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            let now = <frame_system::Pallet<T>>::block_number();

            let (is_upgrade, current_level) = if let Some(record) = KycRecords::<T>::get(entity_id, &who) {
                ensure!(record.status != KycStatus::Pending, Error::<T>::KycAlreadyPending);
                if record.status == KycStatus::Approved && !Self::is_past_expiry(&record) {
                    ensure!(level > record.level, Error::<T>::KycAlreadyApproved);
                    (true, record.level)
                } else {
                    (false, record.level)
                }
            } else {
                (false, KycLevel::None)
            };

            if is_upgrade {

                let upgrade = KycUpgradeRequest {
                    target_level: level,
                    data_cid: data_bounded,
                    country_code,
                    submitted_at: now,
                };
                UpgradeRequests::<T>::insert(entity_id, &who, upgrade);
                PendingKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_add(1));
                Self::record_history(entity_id, &who, KycAction::Submitted, level);

                Self::deposit_event(Event::KycUpgradeRequested {
                    entity_id,
                    account: who,
                    current_level,
                    target_level: level,
                });
            } else {
                let old_status = KycRecords::<T>::get(entity_id, &who)
                    .map(|r| r.status)
                    .unwrap_or(KycStatus::NotSubmitted);

                let record = KycRecord {
                    level,
                    status: KycStatus::Pending,
                    provider: None,
                    data_cid: Some(data_bounded),
                    submitted_at: Some(now),
                    verified_at: None,
                    expires_at: None,
                    rejection_reason: None,
                    rejection_details_cid: None,
                    country_code: Some(country_code),
                    risk_score: 0,
                };

                KycRecords::<T>::insert(entity_id, &who, record);
                Self::update_status_counts(entity_id, old_status, KycStatus::Pending);
                Self::record_history(entity_id, &who, KycAction::Submitted, level);
                T::OnKycStatusChange::on_kyc_status_changed(
                    entity_id, &who, old_status.as_u8(), KycStatus::Pending.as_u8(), level.as_u8(),
                );

                Self::deposit_event(Event::KycSubmitted {
                    entity_id,
                    account: who,
                    level,
                });
            }

            Ok(())
        }

        /// 批准 KYC（Entity 授权的 Provider 或 Entity Owner/Admin 调用）
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::approve_kyc())]
        pub fn approve_kyc(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
            risk_score: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(who != account, Error::<T>::SelfApprovalNotAllowed);
            ensure!(risk_score <= 100, Error::<T>::InvalidRiskScore);
            Self::ensure_entity_operable(entity_id)?;

            let provider_max_level = Self::ensure_can_review(entity_id, &who)?;

            if let Some(upgrade) = UpgradeRequests::<T>::get(entity_id, &account) {
                if let Some(max_level) = provider_max_level {
                    ensure!(upgrade.target_level <= max_level, Error::<T>::ProviderLevelNotSupported);
                }

                let target_level = upgrade.target_level;
                let upgrade_country = upgrade.country_code;
                let upgrade_data = upgrade.data_cid;

                KycRecords::<T>::try_mutate(entity_id, &account, |maybe_record| -> DispatchResult {
                    let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;

                    let old_status = record.status;
                    let now = <frame_system::Pallet<T>>::block_number();
                    let validity = Self::get_validity_period(target_level);
                    let expires_at = now.saturating_add(validity);

                    record.level = target_level;
                    record.status = KycStatus::Approved;
                    record.provider = Some(who.clone());
                    record.data_cid = Some(upgrade_data);
                    record.verified_at = Some(now);
                    record.expires_at = Some(expires_at);
                    record.risk_score = risk_score;
                    record.country_code = Some(upgrade_country);

                    PendingKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_sub(1));
                    Self::update_status_counts(entity_id, old_status, KycStatus::Approved);
                    Self::record_history(entity_id, &account, KycAction::Approved, target_level);
                    T::OnKycStatusChange::on_kyc_status_changed(
                        entity_id, &account, old_status.as_u8(), KycStatus::Approved.as_u8(), target_level.as_u8(),
                    );

                    Self::deposit_event(Event::KycApproved {
                        entity_id,
                        account: account.clone(),
                        level: target_level,
                        provider: who.clone(),
                        expires_at,
                    });
                    Ok(())
                })?;

                UpgradeRequests::<T>::remove(entity_id, &account);
                return Ok(());
            }

            KycRecords::<T>::try_mutate(entity_id, &account, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(record.status == KycStatus::Pending, Error::<T>::InvalidKycStatus);

                if let Some(max_level) = provider_max_level {
                    ensure!(record.level <= max_level, Error::<T>::ProviderLevelNotSupported);
                }

                let now = <frame_system::Pallet<T>>::block_number();
                let validity = Self::get_validity_period(record.level);
                let expires_at = now.saturating_add(validity);

                let old_status = record.status;
                record.status = KycStatus::Approved;
                record.provider = Some(who.clone());
                record.verified_at = Some(now);
                record.expires_at = Some(expires_at);
                record.risk_score = risk_score;

                Self::update_status_counts(entity_id, old_status, KycStatus::Approved);
                Self::record_history(entity_id, &account, KycAction::Approved, record.level);
                T::OnKycStatusChange::on_kyc_status_changed(
                    entity_id, &account, old_status.as_u8(), KycStatus::Approved.as_u8(), record.level.as_u8(),
                );

                Self::deposit_event(Event::KycApproved {
                    entity_id,
                    account: account.clone(),
                    level: record.level,
                    provider: who.clone(),
                    expires_at,
                });
                Ok(())
            })
        }

        /// 拒绝 KYC（Entity 授权的 Provider 或 Entity Owner/Admin 调用）
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::reject_kyc())]
        pub fn reject_kyc(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
            reason: RejectionReason,
            details_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::ensure_entity_operable(entity_id)?;
            let provider_max_level = Self::ensure_can_review(entity_id, &who)?;

            let details_bounded = details_cid
                .map(|cid| {
                    ensure!(!cid.is_empty(), Error::<T>::EmptyDataCid);
                    cid.try_into().map_err(|_| Error::<T>::CidTooLong)
                })
                .transpose()?;

            if let Some(upgrade) = UpgradeRequests::<T>::get(entity_id, &account) {
                if let Some(max_level) = provider_max_level {
                    ensure!(upgrade.target_level <= max_level, Error::<T>::ProviderLevelNotSupported);
                }

                let target_level = upgrade.target_level;
                UpgradeRequests::<T>::remove(entity_id, &account);
                PendingKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_sub(1));
                Self::record_history(entity_id, &account, KycAction::Rejected, target_level);

                Self::deposit_event(Event::KycUpgradeRejected {
                    entity_id,
                    account: account.clone(),
                    target_level,
                    reason,
                });
                return Ok(());
            }

            KycRecords::<T>::try_mutate(entity_id, &account, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(record.status == KycStatus::Pending, Error::<T>::InvalidKycStatus);

                if let Some(max_level) = provider_max_level {
                    ensure!(record.level <= max_level, Error::<T>::ProviderLevelNotSupported);
                }

                let now = <frame_system::Pallet<T>>::block_number();

                let old_status = record.status;
                record.status = KycStatus::Rejected;
                record.provider = Some(who.clone());
                record.verified_at = Some(now);
                record.rejection_reason = Some(reason);
                record.rejection_details_cid = details_bounded;

                Self::update_status_counts(entity_id, old_status, KycStatus::Rejected);
                Self::record_history(entity_id, &account, KycAction::Rejected, record.level);
                T::OnKycStatusChange::on_kyc_status_changed(
                    entity_id, &account, old_status.as_u8(), KycStatus::Rejected.as_u8(), record.level.as_u8(),
                );

                Self::deposit_event(Event::KycRejected {
                    entity_id,
                    account: account.clone(),
                    level: record.level,
                    reason,
                });
                Ok(())
            })
        }

        /// 撤销 KYC（全局管理员调用）
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::revoke_kyc())]
        pub fn revoke_kyc(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
            reason: RejectionReason,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            Self::cleanup_upgrade_request(entity_id, &account);

            KycRecords::<T>::try_mutate(entity_id, &account, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(
                    record.status == KycStatus::Pending
                        || record.status == KycStatus::Approved
                        || record.status == KycStatus::Expired,
                    Error::<T>::InvalidKycStatus
                );

                let old_status = record.status;
                record.status = KycStatus::Revoked;
                record.rejection_reason = Some(reason);

                Self::update_status_counts(entity_id, old_status, KycStatus::Revoked);
                Self::record_history(entity_id, &account, KycAction::Revoked, record.level);
                T::OnKycStatusChange::on_kyc_status_changed(
                    entity_id, &account, old_status.as_u8(), KycStatus::Revoked.as_u8(), record.level.as_u8(),
                );

                Self::deposit_event(Event::KycRevoked {
                    entity_id,
                    account: account.clone(),
                    reason,
                });
                Ok(())
            })
        }

        /// 注册认证提供者（全局，AdminOrigin）
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::register_provider())]
        pub fn register_provider(
            origin: OriginFor<T>,
            provider_account: T::AccountId,
            name: Vec<u8>,
            max_level: KycLevel,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            ensure!(!Providers::<T>::contains_key(&provider_account), Error::<T>::ProviderAlreadyExists);
            ensure!(max_level != KycLevel::None, Error::<T>::InvalidKycLevel);

            let count = ProviderCount::<T>::get();
            ensure!(count < T::MaxProviders::get(), Error::<T>::MaxProvidersReached);
            ensure!(!name.is_empty(), Error::<T>::EmptyProviderName);

            let name_bounded: BoundedVec<u8, T::MaxProviderNameLength> =
                name.try_into().map_err(|_| Error::<T>::NameTooLong)?;

            let name_for_event = name_bounded.to_vec();

            let provider = KycProvider {
                name: name_bounded,
                max_level,
                suspended: false,
            };

            Providers::<T>::insert(&provider_account, provider);
            ProviderCount::<T>::put(count.saturating_add(1));

            Self::deposit_event(Event::ProviderRegistered {
                provider: provider_account,
                name: name_for_event,
            });
            Ok(())
        }

        /// 移除认证提供者（全局，AdminOrigin）
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::remove_provider())]
        pub fn remove_provider(
            origin: OriginFor<T>,
            provider_account: T::AccountId,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            ensure!(Providers::<T>::contains_key(&provider_account), Error::<T>::ProviderNotFound);

            Providers::<T>::remove(&provider_account);
            ProviderCount::<T>::mutate(|count| *count = count.saturating_sub(1));

            // 使用反向索引有界清理，避免全表扫描
            let entities = ProviderAuthorizedEntities::<T>::take(&provider_account);
            for eid in entities.iter() {
                EntityAuthorizedProviders::<T>::remove(eid, &provider_account);
            }

            Self::deposit_event(Event::ProviderRemoved {
                provider: provider_account,
            });
            Ok(())
        }

        /// 设置实体 KYC 要求（Entity Owner 或有 KYC_MANAGE 权限的管理员可调用）
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::set_entity_requirement())]
        pub fn set_entity_requirement(
            origin: OriginFor<T>,
            entity_id: u64,
            min_level: KycLevel,
            mandatory: bool,
            grace_period: u32,
            allow_high_risk_countries: bool,
            max_risk_score: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::ensure_entity_operable(entity_id)?;
            Self::ensure_entity_owner_or_admin(entity_id, &who)?;
            ensure!(max_risk_score <= 100, Error::<T>::InvalidRiskScore);

            let requirement = EntityKycRequirement {
                min_level,
                mandatory,
                grace_period,
                allow_high_risk_countries,
                max_risk_score,
            };

            EntityRequirements::<T>::insert(entity_id, requirement);

            Self::deposit_event(Event::EntityRequirementSet {
                entity_id,
                min_level,
            });
            Ok(())
        }

        /// 更新高风险国家列表（全局，AdminOrigin）
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::update_high_risk_countries())]
        pub fn update_high_risk_countries(
            origin: OriginFor<T>,
            countries: BoundedVec<[u8; 2], ConstU32<50>>,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            for code in countries.iter() {
                ensure!(
                    code[0].is_ascii_uppercase() && code[1].is_ascii_uppercase(),
                    Error::<T>::InvalidCountryCode
                );
            }

            let mut deduped: Vec<[u8; 2]> = countries.into_inner();
            deduped.sort();
            deduped.dedup();

            let bounded: BoundedVec<[u8; 2], ConstU32<50>> =
                deduped.try_into().map_err(|_| Error::<T>::TooManyCountries)?;

            let count = bounded.len() as u32;
            HighRiskCountries::<T>::put(bounded);

            Self::deposit_event(Event::HighRiskCountriesUpdated { count });
            Ok(())
        }

        /// 标记已过期的 KYC 记录（任何人可调用）
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::expire_kyc())]
        pub fn expire_kyc(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            KycRecords::<T>::try_mutate(entity_id, &account, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(record.status == KycStatus::Approved, Error::<T>::InvalidKycStatus);

                let expires_at = record.expires_at.ok_or(Error::<T>::InvalidKycStatus)?;
                let now = <frame_system::Pallet<T>>::block_number();
                ensure!(now > expires_at, Error::<T>::KycNotExpired);

                let old_status = record.status;
                record.status = KycStatus::Expired;

                Self::update_status_counts(entity_id, old_status, KycStatus::Expired);
                Self::record_history(entity_id, &account, KycAction::Expired, record.level);
                T::OnKycStatusChange::on_kyc_status_changed(
                    entity_id, &account, old_status.as_u8(), KycStatus::Expired.as_u8(), record.level.as_u8(),
                );

                Self::deposit_event(Event::KycExpired {
                    entity_id,
                    account: account.clone(),
                });
                Ok(())
            })
        }

        /// 取消待审核的 KYC 申请（用户自行撤回，含升级请求）
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::cancel_kyc())]
        pub fn cancel_kyc(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            if let Some(upgrade) = UpgradeRequests::<T>::get(entity_id, &who) {
                let target_level = upgrade.target_level;
                UpgradeRequests::<T>::remove(entity_id, &who);
                PendingKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_sub(1));
                Self::record_history(entity_id, &who, KycAction::Cancelled, target_level);

                Self::deposit_event(Event::KycUpgradeCancelled {
                    entity_id,
                    account: who,
                    target_level,
                });
                return Ok(());
            }

            KycRecords::<T>::try_mutate(entity_id, &who, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(record.status == KycStatus::Pending, Error::<T>::InvalidKycStatus);

                let level = record.level;
                *maybe_record = None;

                Self::update_status_counts(entity_id, KycStatus::Pending, KycStatus::NotSubmitted);
                Self::record_history(entity_id, &who, KycAction::Cancelled, level);
                T::OnKycStatusChange::on_kyc_status_changed(
                    entity_id, &who, KycStatus::Pending.as_u8(), KycStatus::NotSubmitted.as_u8(), level.as_u8(),
                );

                Self::deposit_event(Event::KycCancelled {
                    entity_id,
                    account: who.clone(),
                });
                Ok(())
            })
        }

        /// 强制设置实体 KYC 要求（AdminOrigin，跳过 Entity 存在性检查）
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::force_set_entity_requirement())]
        pub fn force_set_entity_requirement(
            origin: OriginFor<T>,
            entity_id: u64,
            min_level: KycLevel,
            mandatory: bool,
            grace_period: u32,
            allow_high_risk_countries: bool,
            max_risk_score: u8,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            ensure!(max_risk_score <= 100, Error::<T>::InvalidRiskScore);

            let requirement = EntityKycRequirement {
                min_level,
                mandatory,
                grace_period,
                allow_high_risk_countries,
                max_risk_score,
            };

            EntityRequirements::<T>::insert(entity_id, requirement);

            Self::deposit_event(Event::EntityRequirementSet {
                entity_id,
                min_level,
            });
            Ok(())
        }

        /// 更新已批准用户的风险评分（Entity 授权的 Provider 或 Entity Owner/Admin 调用）
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::update_risk_score())]
        pub fn update_risk_score(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
            new_score: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::ensure_entity_operable(entity_id)?;
            Self::ensure_can_review(entity_id, &who)?;
            ensure!(new_score <= 100, Error::<T>::InvalidRiskScore);

            KycRecords::<T>::try_mutate(entity_id, &account, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(record.status == KycStatus::Approved, Error::<T>::InvalidKycStatus);

                let old_score = record.risk_score;
                record.risk_score = new_score;

                Self::deposit_event(Event::RiskScoreUpdated {
                    entity_id,
                    account: account.clone(),
                    old_score,
                    new_score,
                });
                Ok(())
            })
        }

        /// 更新认证提供者信息（全局，AdminOrigin）
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::update_provider())]
        pub fn update_provider(
            origin: OriginFor<T>,
            provider_account: T::AccountId,
            name: Option<Vec<u8>>,
            max_level: Option<KycLevel>,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            ensure!(name.is_some() || max_level.is_some(), Error::<T>::NothingToUpdate);

            Providers::<T>::try_mutate(&provider_account, |maybe_provider| -> DispatchResult {
                let provider = maybe_provider.as_mut().ok_or(Error::<T>::ProviderNotFound)?;

                if let Some(new_name) = name {
                    ensure!(!new_name.is_empty(), Error::<T>::EmptyProviderName);
                    let name_bounded: BoundedVec<u8, T::MaxProviderNameLength> =
                        new_name.try_into().map_err(|_| Error::<T>::NameTooLong)?;
                    provider.name = name_bounded;
                }

                if let Some(new_level) = max_level {
                    ensure!(new_level != KycLevel::None, Error::<T>::InvalidKycLevel);
                    provider.max_level = new_level;
                }

                Self::deposit_event(Event::ProviderUpdated {
                    provider: provider_account.clone(),
                });
                Ok(())
            })
        }

        /// 暂停认证提供者（全局，AdminOrigin）
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::suspend_provider())]
        pub fn suspend_provider(
            origin: OriginFor<T>,
            provider_account: T::AccountId,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            Providers::<T>::try_mutate(&provider_account, |maybe_provider| -> DispatchResult {
                let provider = maybe_provider.as_mut().ok_or(Error::<T>::ProviderNotFound)?;
                ensure!(!provider.suspended, Error::<T>::ProviderIsSuspended);
                provider.suspended = true;

                Self::deposit_event(Event::ProviderSuspended {
                    provider: provider_account.clone(),
                });
                Ok(())
            })
        }

        /// 恢复认证提供者（全局，AdminOrigin）
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::resume_provider())]
        pub fn resume_provider(
            origin: OriginFor<T>,
            provider_account: T::AccountId,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            Providers::<T>::try_mutate(&provider_account, |maybe_provider| -> DispatchResult {
                let provider = maybe_provider.as_mut().ok_or(Error::<T>::ProviderNotFound)?;
                ensure!(provider.suspended, Error::<T>::ProviderNotSuspended);
                provider.suspended = false;

                Self::deposit_event(Event::ProviderResumed {
                    provider: provider_account.clone(),
                });
                Ok(())
            })
        }

        /// 强制批准 KYC（AdminOrigin，用于数据迁移/特殊豁免）
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::force_approve_kyc())]
        pub fn force_approve_kyc(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
            level: KycLevel,
            risk_score: u8,
            country_code: [u8; 2],
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            ensure!(level != KycLevel::None, Error::<T>::InvalidKycLevel);
            ensure!(risk_score <= 100, Error::<T>::InvalidRiskScore);
            ensure!(
                country_code[0].is_ascii_uppercase() && country_code[1].is_ascii_uppercase(),
                Error::<T>::InvalidCountryCode
            );

            Self::cleanup_upgrade_request(entity_id, &account);

            let old_status = KycRecords::<T>::get(entity_id, &account)
                .map(|r| r.status)
                .unwrap_or(KycStatus::NotSubmitted);

            let now = <frame_system::Pallet<T>>::block_number();
            let validity = Self::get_validity_period(level);
            let expires_at = now.saturating_add(validity);

            let record = KycRecord {
                level,
                status: KycStatus::Approved,
                provider: None,
                data_cid: None,
                submitted_at: Some(now),
                verified_at: Some(now),
                expires_at: Some(expires_at),
                rejection_reason: None,
                rejection_details_cid: None,
                country_code: Some(country_code),
                risk_score,
            };

            KycRecords::<T>::insert(entity_id, &account, record);

            Self::update_status_counts(entity_id, old_status, KycStatus::Approved);
            Self::record_history(entity_id, &account, KycAction::ForceApproved, level);
            T::OnKycStatusChange::on_kyc_status_changed(
                entity_id, &account, old_status.as_u8(), KycStatus::Approved.as_u8(), level.as_u8(),
            );

            Self::deposit_event(Event::KycForceApproved {
                entity_id,
                account,
                level,
                expires_at,
            });
            Ok(())
        }

        /// 续期 KYC（Entity 授权的 Provider 或 Entity Owner/Admin 调用）
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::renew_kyc())]
        pub fn renew_kyc(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(who != account, Error::<T>::SelfApprovalNotAllowed);
            Self::ensure_entity_operable(entity_id)?;
            let provider_max_level = Self::ensure_can_review(entity_id, &who)?;

            KycRecords::<T>::try_mutate(entity_id, &account, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;

                ensure!(
                    record.status == KycStatus::Approved || record.status == KycStatus::Expired,
                    Error::<T>::KycNotRenewable
                );

                if let Some(max_level) = provider_max_level {
                    ensure!(record.level <= max_level, Error::<T>::ProviderLevelNotSupported);
                }

                let now = <frame_system::Pallet<T>>::block_number();
                let validity = Self::get_validity_period(record.level);
                let expires_at = now.saturating_add(validity);

                let old_status = record.status;
                record.status = KycStatus::Approved;
                record.provider = Some(who.clone());
                record.verified_at = Some(now);
                record.expires_at = Some(expires_at);

                Self::update_status_counts(entity_id, old_status, KycStatus::Approved);
                Self::record_history(entity_id, &account, KycAction::Renewed, record.level);
                T::OnKycStatusChange::on_kyc_status_changed(
                    entity_id, &account, old_status.as_u8(), KycStatus::Approved.as_u8(), record.level.as_u8(),
                );

                Self::deposit_event(Event::KycRenewed {
                    entity_id,
                    account: account.clone(),
                    level: record.level,
                    expires_at,
                });
                Ok(())
            })
        }

        /// 更新待审核 KYC 的数据（用户补充/替换材料，含升级请求）
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::update_kyc_data())]
        pub fn update_kyc_data(
            origin: OriginFor<T>,
            entity_id: u64,
            new_data_cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!new_data_cid.is_empty(), Error::<T>::EmptyDataCid);

            let data_bounded: BoundedVec<u8, T::MaxCidLength> =
                new_data_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            if UpgradeRequests::<T>::contains_key(entity_id, &who) {
                UpgradeRequests::<T>::try_mutate(entity_id, &who, |maybe_upgrade| -> DispatchResult {
                    let upgrade = maybe_upgrade.as_mut().ok_or(Error::<T>::KycNotFound)?;
                    upgrade.data_cid = data_bounded;

                    Self::record_history(entity_id, &who, KycAction::DataUpdated, upgrade.target_level);

                    Self::deposit_event(Event::KycDataUpdated {
                        entity_id,
                        account: who.clone(),
                    });
                    Ok(())
                })?;
                return Ok(());
            }

            KycRecords::<T>::try_mutate(entity_id, &who, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(record.status == KycStatus::Pending, Error::<T>::InvalidKycStatus);

                record.data_cid = Some(data_bounded);
                // 不重置 submitted_at，防止用户通过反复更新数据逃避 PendingKycTimeout

                Self::record_history(entity_id, &who, KycAction::DataUpdated, record.level);

                Self::deposit_event(Event::KycDataUpdated {
                    entity_id,
                    account: who.clone(),
                });
                Ok(())
            })
        }

        /// 清除 KYC 数据（GDPR 数据删除权，仅限终态记录）
        #[pallet::call_index(18)]
        #[pallet::weight(T::WeightInfo::purge_kyc_data())]
        pub fn purge_kyc_data(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            KycRecords::<T>::try_mutate(entity_id, &who, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;

                ensure!(
                    record.status == KycStatus::Rejected
                        || record.status == KycStatus::Revoked
                        || record.status == KycStatus::Expired,
                    Error::<T>::KycDataCannotBePurged
                );

                record.data_cid = None;
                record.rejection_details_cid = None;
                record.country_code = None;
                record.risk_score = 0;

                Self::record_history(entity_id, &who, KycAction::DataPurged, record.level);

                Self::deposit_event(Event::KycDataPurged {
                    entity_id,
                    account: who.clone(),
                });
                Ok(())
            })
        }

        /// 移除实体 KYC 要求（Entity Owner 或有 KYC_MANAGE 权限的管理员可调用）
        #[pallet::call_index(19)]
        #[pallet::weight(T::WeightInfo::remove_entity_requirement())]
        pub fn remove_entity_requirement(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::ensure_entity_operable(entity_id)?;
            Self::ensure_entity_owner_or_admin(entity_id, &who)?;
            ensure!(EntityRequirements::<T>::contains_key(entity_id), Error::<T>::RequirementNotFound);

            EntityRequirements::<T>::remove(entity_id);

            Self::deposit_event(Event::EntityRequirementRemoved { entity_id });
            Ok(())
        }

        /// 超时待审核 KYC（任何人可调用，含升级请求）
        #[pallet::call_index(20)]
        #[pallet::weight(T::WeightInfo::timeout_pending_kyc())]
        pub fn timeout_pending_kyc(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            if let Some(upgrade) = UpgradeRequests::<T>::get(entity_id, &account) {
                let now = <frame_system::Pallet<T>>::block_number();
                let timeout = T::PendingKycTimeout::get();
                ensure!(now > upgrade.submitted_at.saturating_add(timeout), Error::<T>::PendingNotTimedOut);

                let target_level = upgrade.target_level;
                UpgradeRequests::<T>::remove(entity_id, &account);
                PendingKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_sub(1));
                Self::record_history(entity_id, &account, KycAction::TimedOut, target_level);

                Self::deposit_event(Event::KycUpgradeTimedOut {
                    entity_id,
                    account: account.clone(),
                    target_level,
                });
                return Ok(());
            }

            KycRecords::<T>::try_mutate(entity_id, &account, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(record.status == KycStatus::Pending, Error::<T>::InvalidKycStatus);

                let submitted_at = record.submitted_at.ok_or(Error::<T>::InvalidKycStatus)?;
                let now = <frame_system::Pallet<T>>::block_number();
                let timeout = T::PendingKycTimeout::get();
                ensure!(now > submitted_at.saturating_add(timeout), Error::<T>::PendingNotTimedOut);

                let old_status = record.status;
                record.status = KycStatus::Rejected;
                record.rejection_reason = Some(RejectionReason::TimedOut);

                Self::update_status_counts(entity_id, old_status, KycStatus::Rejected);
                Self::record_history(entity_id, &account, KycAction::TimedOut, record.level);
                T::OnKycStatusChange::on_kyc_status_changed(
                    entity_id, &account, old_status.as_u8(), KycStatus::Rejected.as_u8(), record.level.as_u8(),
                );

                Self::deposit_event(Event::PendingKycTimedOut {
                    entity_id,
                    account: account.clone(),
                });
                Ok(())
            })
        }

        /// 批量撤销指定 Provider 在指定 Entity 中批准的 KYC
        #[pallet::call_index(21)]
        #[pallet::weight(T::WeightInfo::batch_revoke_by_provider(accounts.len() as u32))]
        pub fn batch_revoke_by_provider(
            origin: OriginFor<T>,
            entity_id: u64,
            provider_account: T::AccountId,
            accounts: BoundedVec<T::AccountId, ConstU32<100>>,
            reason: RejectionReason,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            ensure!(!accounts.is_empty(), Error::<T>::EmptyAccountList);
            ensure!(Providers::<T>::contains_key(&provider_account), Error::<T>::ProviderNotFound);

            let mut revoked_count: u32 = 0;

            for account in &accounts {
                Self::cleanup_upgrade_request(entity_id, account);

                KycRecords::<T>::try_mutate(entity_id, account, |maybe_record| -> DispatchResult {
                    let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;

                    ensure!(
                        record.provider.as_ref() == Some(&provider_account),
                        Error::<T>::ProviderMismatch
                    );
                    ensure!(
                        record.status == KycStatus::Approved || record.status == KycStatus::Expired,
                        Error::<T>::InvalidKycStatus
                    );

                    let old_status = record.status;
                    record.status = KycStatus::Revoked;
                    record.rejection_reason = Some(reason);

                    Self::update_status_counts(entity_id, old_status, KycStatus::Revoked);
                    Self::record_history(entity_id, account, KycAction::Revoked, record.level);
                    T::OnKycStatusChange::on_kyc_status_changed(
                        entity_id, account, old_status.as_u8(), KycStatus::Revoked.as_u8(), record.level.as_u8(),
                    );

                    revoked_count = revoked_count.saturating_add(1);
                    Ok(())
                })?;
            }

            Self::deposit_event(Event::ProviderKycsRevoked {
                entity_id,
                provider: provider_account,
                count: revoked_count,
                reason,
            });
            Ok(())
        }

        // call_index(22) reserved (was force_remove_provider, removed as redundant with remove_provider)

        /// 授权 Provider 为指定 Entity 审核 KYC（Entity Owner 或 KYC_MANAGE Admin）
        #[pallet::call_index(23)]
        #[pallet::weight(T::WeightInfo::authorize_provider())]
        pub fn authorize_provider(
            origin: OriginFor<T>,
            entity_id: u64,
            provider_account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::ensure_entity_operable(entity_id)?;
            Self::ensure_entity_owner_or_admin(entity_id, &who)?;
            ensure!(Providers::<T>::contains_key(&provider_account), Error::<T>::ProviderNotFound);
            ensure!(
                !EntityAuthorizedProviders::<T>::contains_key(entity_id, &provider_account),
                Error::<T>::ProviderAlreadyAuthorized
            );

            EntityAuthorizedProviders::<T>::insert(entity_id, &provider_account, ());

            // 维护反向索引
            ProviderAuthorizedEntities::<T>::try_mutate(&provider_account, |entities| -> DispatchResult {
                entities.try_push(entity_id).map_err(|_| Error::<T>::MaxProvidersReached)?;
                Ok(())
            })?;

            Self::deposit_event(Event::ProviderAuthorized {
                entity_id,
                provider: provider_account,
            });
            Ok(())
        }

        /// 撤销 Provider 对指定 Entity 的审核授权（Entity Owner 或 KYC_MANAGE Admin）
        #[pallet::call_index(24)]
        #[pallet::weight(T::WeightInfo::deauthorize_provider())]
        pub fn deauthorize_provider(
            origin: OriginFor<T>,
            entity_id: u64,
            provider_account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::ensure_entity_operable(entity_id)?;
            Self::ensure_entity_owner_or_admin(entity_id, &who)?;
            ensure!(
                EntityAuthorizedProviders::<T>::contains_key(entity_id, &provider_account),
                Error::<T>::ProviderNotAuthorized
            );

            EntityAuthorizedProviders::<T>::remove(entity_id, &provider_account);

            // 维护反向索引
            ProviderAuthorizedEntities::<T>::mutate(&provider_account, |entities| {
                entities.retain(|&eid| eid != entity_id);
            });

            Self::deposit_event(Event::ProviderDeauthorized {
                entity_id,
                provider: provider_account,
            });
            Ok(())
        }

        /// Entity Owner/Admin 撤销用户 KYC（per-entity 级别的撤销权限）
        #[pallet::call_index(25)]
        #[pallet::weight(T::WeightInfo::entity_revoke_kyc())]
        pub fn entity_revoke_kyc(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
            reason: RejectionReason,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::ensure_entity_operable(entity_id)?;
            let is_owner = T::EntityProvider::entity_owner(entity_id)
                .map(|owner| owner == who)
                .unwrap_or(false);
            let is_admin = T::EntityProvider::is_entity_admin(
                entity_id, &who, pallet_entity_common::AdminPermission::KYC_MANAGE,
            );
            ensure!(is_owner || is_admin, Error::<T>::NotEntityOwnerOrAdmin);

            Self::cleanup_upgrade_request(entity_id, &account);

            KycRecords::<T>::try_mutate(entity_id, &account, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(
                    record.status == KycStatus::Pending
                        || record.status == KycStatus::Approved
                        || record.status == KycStatus::Expired,
                    Error::<T>::InvalidKycStatus
                );

                let old_status = record.status;
                record.status = KycStatus::Revoked;
                record.rejection_reason = Some(reason);

                Self::update_status_counts(entity_id, old_status, KycStatus::Revoked);
                Self::record_history(entity_id, &account, KycAction::Revoked, record.level);
                T::OnKycStatusChange::on_kyc_status_changed(
                    entity_id, &account, old_status.as_u8(), KycStatus::Revoked.as_u8(), record.level.as_u8(),
                );

                Self::deposit_event(Event::EntityKycRevoked {
                    entity_id,
                    account: account.clone(),
                    reason,
                    revoker: who.clone(),
                });
                Ok(())
            })
        }
    }

    // ==================== 辅助函数 ====================

    impl<T: Config> Pallet<T> {
        pub fn get_validity_period(level: KycLevel) -> BlockNumberFor<T> {
            match level {
                KycLevel::None => BlockNumberFor::<T>::from(0u32),
                KycLevel::Basic => T::BasicKycValidity::get(),
                KycLevel::Standard => T::StandardKycValidity::get(),
                KycLevel::Enhanced => T::EnhancedKycValidity::get(),
                KycLevel::Institutional => T::InstitutionalKycValidity::get(),
            }
        }

        fn ensure_can_review(entity_id: u64, who: &T::AccountId) -> Result<Option<KycLevel>, DispatchError> {
            if let Some(provider) = Providers::<T>::get(who) {
                ensure!(!provider.suspended, Error::<T>::ProviderIsSuspended);
                ensure!(
                    EntityAuthorizedProviders::<T>::contains_key(entity_id, who),
                    Error::<T>::ProviderNotAuthorized
                );
                return Ok(Some(provider.max_level));
            }

            let is_owner = T::EntityProvider::entity_owner(entity_id)
                .map(|owner| owner == *who)
                .unwrap_or(false);
            let is_admin = T::EntityProvider::is_entity_admin(
                entity_id, who, pallet_entity_common::AdminPermission::KYC_MANAGE,
            );
            ensure!(is_owner || is_admin, Error::<T>::NotEntityOwnerOrAdmin);
            Ok(None)
        }

        fn ensure_entity_owner_or_admin(entity_id: u64, who: &T::AccountId) -> DispatchResult {
            let is_owner = T::EntityProvider::entity_owner(entity_id)
                .map(|owner| owner == *who)
                .unwrap_or(false);
            let is_admin = T::EntityProvider::is_entity_admin(
                entity_id, who, pallet_entity_common::AdminPermission::KYC_MANAGE,
            );
            ensure!(is_owner || is_admin, Error::<T>::NotEntityOwnerOrAdmin);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            Ok(())
        }

        fn ensure_entity_operable(entity_id: u64) -> DispatchResult {
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            Ok(())
        }

        fn is_past_expiry(record: &KycRecordOf<T>) -> bool {
            record.expires_at
                .map(|expires_at| <frame_system::Pallet<T>>::block_number() > expires_at)
                .unwrap_or(false)
        }

        fn is_record_currently_valid(record: &KycRecordOf<T>) -> bool {
            record.status == KycStatus::Approved && !Self::is_past_expiry(record)
        }

        fn cleanup_upgrade_request(entity_id: u64, account: &T::AccountId) {
            if UpgradeRequests::<T>::contains_key(entity_id, account) {
                UpgradeRequests::<T>::remove(entity_id, account);
                PendingKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_sub(1));
            }
        }

        pub fn get_kyc_level(entity_id: u64, account: &T::AccountId) -> KycLevel {
            KycRecords::<T>::get(entity_id, account)
                .filter(|r| Self::is_record_currently_valid(r))
                .map(|r| r.level)
                .unwrap_or(KycLevel::None)
        }

        pub fn meets_kyc_requirement(entity_id: u64, account: &T::AccountId, min_level: KycLevel) -> bool {
            if let Some(record) = KycRecords::<T>::get(entity_id, account) {
                if !Self::is_record_currently_valid(&record) {
                    return false;
                }
                record.level >= min_level
            } else {
                min_level == KycLevel::None
            }
        }

        pub fn is_high_risk_country(entity_id: u64, account: &T::AccountId) -> bool {
            if let Some(record) = KycRecords::<T>::get(entity_id, account) {
                if let Some(country) = record.country_code {
                    return HighRiskCountries::<T>::get().contains(&country);
                }
            }
            false
        }

        pub fn can_participate_in_entity(account: &T::AccountId, entity_id: u64) -> bool {
            if let Some(requirement) = EntityRequirements::<T>::get(entity_id) {
                Self::check_account_compliance(entity_id, account, &requirement)
            } else {
                true
            }
        }

        pub fn get_risk_score(entity_id: u64, account: &T::AccountId) -> u8 {
            KycRecords::<T>::get(entity_id, account)
                .filter(|r| Self::is_record_currently_valid(r))
                .map(|r| r.risk_score)
                .unwrap_or(100)
        }

        // ==================== 历史记录 ====================

        pub(crate) fn record_history(
            entity_id: u64,
            account: &T::AccountId,
            action: KycAction,
            level: KycLevel,
        ) {
            KycHistory::<T>::mutate(entity_id, account, |history| {
                let entry = KycHistoryEntry {
                    action,
                    level,
                    block_number: <frame_system::Pallet<T>>::block_number(),
                };
                if history.is_full() {
                    history.remove(0);
                }
                let _ = history.try_push(entry);
            });
        }

        pub fn get_kyc_history(
            entity_id: u64,
            account: &T::AccountId,
        ) -> alloc::vec::Vec<KycHistoryEntryOf<T>> {
            KycHistory::<T>::get(entity_id, account).into_inner()
        }

        // ==================== 状态计数 ====================

        pub(crate) fn update_status_counts(entity_id: u64, old_status: KycStatus, new_status: KycStatus) {
            if old_status == new_status {
                return;
            }
            match old_status {
                KycStatus::Pending => PendingKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_sub(1)),
                KycStatus::Approved => ApprovedKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_sub(1)),
                _ => {},
            }
            match new_status {
                KycStatus::Pending => PendingKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_add(1)),
                KycStatus::Approved => ApprovedKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_add(1)),
                _ => {},
            }
        }

        pub fn get_kyc_stats(entity_id: u64) -> (u32, u32) {
            (PendingKycCount::<T>::get(entity_id), ApprovedKycCount::<T>::get(entity_id))
        }

        // ==================== 合规检查 ====================

        pub fn check_account_compliance(
            entity_id: u64,
            account: &T::AccountId,
            requirement: &EntityKycRequirement,
        ) -> bool {
            if !requirement.mandatory {
                return true;
            }

            if let Some(record) = KycRecords::<T>::get(entity_id, account) {
                if record.status != KycStatus::Approved && record.status != KycStatus::Expired {
                    return false;
                }
                if record.level < requirement.min_level {
                    return false;
                }
                if !requirement.allow_high_risk_countries {
                    if let Some(country) = record.country_code {
                        if HighRiskCountries::<T>::get().contains(&country) {
                            return false;
                        }
                    }
                }
                if record.risk_score > requirement.max_risk_score {
                    return false;
                }
                if let Some(expires_at) = record.expires_at {
                    let now = <frame_system::Pallet<T>>::block_number();
                    let grace = BlockNumberFor::<T>::from(requirement.grace_period);
                    if now > expires_at.saturating_add(grace) {
                        return false;
                    }
                }
                return true;
            }
            false
        }
    }

    // ==================== 跨模块接口实现 ====================

    impl<T: Config> pallet_entity_common::KycProvider<T::AccountId> for Pallet<T> {
        fn kyc_level(entity_id: u64, account: &T::AccountId) -> u8 {
            Self::get_kyc_level(entity_id, account).as_u8()
        }

        fn is_kyc_expired(entity_id: u64, account: &T::AccountId) -> bool {
            KycRecords::<T>::get(entity_id, account)
                .map(|r| {
                    r.status == KycStatus::Expired ||
                    (r.status == KycStatus::Approved && Self::is_past_expiry(&r))
                })
                .unwrap_or(false)
        }

        fn can_participate(entity_id: u64, account: &T::AccountId) -> bool {
            Self::can_participate_in_entity(account, entity_id)
        }

        fn kyc_expires_at(entity_id: u64, account: &T::AccountId) -> u64 {
            KycRecords::<T>::get(entity_id, account)
                .and_then(|r| r.expires_at)
                .map(|b| {
                    use sp_runtime::traits::SaturatedConversion;
                    b.saturated_into::<u64>()
                })
                .unwrap_or(0)
        }
    }

    // ========================================================================
    // KycGovernancePort 实现
    // ========================================================================

    impl<T: Config> pallet_entity_common::KycGovernancePort for Pallet<T> {
        fn governance_set_kyc_requirement(
            entity_id: u64,
            min_level: u8,
            mandatory: bool,
            grace_period: u32,
        ) -> Result<(), sp_runtime::DispatchError> {
            let min_level = KycLevel::try_from_u8(min_level)
                .ok_or(sp_runtime::DispatchError::Other("InvalidKycLevel"))?;

            let requirement = EntityKycRequirement {
                min_level,
                mandatory,
                grace_period,
                allow_high_risk_countries: false,
                max_risk_score: 100,
            };

            EntityRequirements::<T>::insert(entity_id, requirement);

            Self::deposit_event(Event::EntityRequirementSet {
                entity_id,
                min_level,
            });
            Ok(())
        }

        fn governance_authorize_kyc_provider(
            _entity_id: u64,
            _provider_id: u64,
        ) -> Result<(), sp_runtime::DispatchError> {
            // provider_id 为 u64 但 KYC 模块使用 AccountId 索引 Provider
            // 治理提案通过后由管理员通过 authorize_provider extrinsic 执行
            Ok(())
        }

        fn governance_deauthorize_kyc_provider(
            _entity_id: u64,
            _provider_id: u64,
        ) -> Result<(), sp_runtime::DispatchError> {
            // provider_id 为 u64 但 KYC 模块使用 AccountId 索引 Provider
            // 治理提案通过后由管理员通过 deauthorize_provider extrinsic 执行
            Ok(())
        }
    }
}
