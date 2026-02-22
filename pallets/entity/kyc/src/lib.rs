//! # 实体 KYC/AML 认证模块 (pallet-entity-kyc)
//!
//! ## 概述
//!
//! 本模块实现实体和用户的 KYC/AML 认证功能：
//! - 多级别认证（基础、标准、增强、机构）
//! - 多种认证提供者支持
//! - 认证状态管理和过期处理
//! - 合规性检查接口
//!
//! ## KYC 级别
//!
//! - **None**: 未认证
//! - **Basic**: 基础认证（邮箱/手机验证）
//! - **Standard**: 标准认证（身份证件）
//! - **Enhanced**: 增强认证（地址证明 + 资金来源）
//! - **Institutional**: 机构认证（企业文件 + 受益人）
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-02-03): Phase 7 初始版本

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::Get,
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Saturating;

    // ==================== 类型定义 ====================

    /// KYC 级别
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum KycLevel {
        /// 未认证
        #[default]
        None,
        /// 基础认证（邮箱/手机）
        Basic,
        /// 标准认证（身份证件）
        Standard,
        /// 增强认证（地址 + 资金来源）
        Enhanced,
        /// 机构认证（企业文件）
        Institutional,
    }

    /// KYC 状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum KycStatus {
        /// 未提交
        #[default]
        NotSubmitted,
        /// 待审核
        Pending,
        /// 已通过
        Approved,
        /// 已拒绝
        Rejected,
        /// 已过期
        Expired,
        /// 已撤销
        Revoked,
    }

    /// 认证类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum VerificationType {
        /// 邮箱验证
        Email,
        /// 手机验证
        Phone,
        /// 身份证件
        IdentityDocument,
        /// 地址证明
        AddressProof,
        /// 资金来源证明
        SourceOfFunds,
        /// 企业注册文件
        BusinessRegistration,
        /// 受益所有人
        BeneficialOwner,
        /// 财务报表
        FinancialStatements,
        /// 人脸识别
        FaceVerification,
        /// 视频认证
        VideoVerification,
    }

    /// 拒绝原因
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum RejectionReason {
        /// 文件不清晰
        UnclearDocument,
        /// 文件过期
        ExpiredDocument,
        /// 信息不匹配
        InformationMismatch,
        /// 可疑活动
        SuspiciousActivity,
        /// 制裁名单
        SanctionedEntity,
        /// 高风险国家
        HighRiskCountry,
        /// 文件伪造
        ForgedDocument,
        /// 其他
        Other,
    }

    /// 认证提供者类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum ProviderType {
        /// 平台内部
        #[default]
        Internal,
        /// 第三方 KYC 服务
        ThirdParty,
        /// 政府机构
        Government,
        /// 金融机构
        Financial,
    }

    /// 认证提供者
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxNameLen))]
    pub struct KycProvider<AccountId, MaxNameLen: Get<u32>> {
        /// 提供者账户
        pub account: AccountId,
        /// 名称
        pub name: BoundedVec<u8, MaxNameLen>,
        /// 类型
        pub provider_type: ProviderType,
        /// 支持的最高认证级别
        pub max_level: KycLevel,
        /// 是否活跃
        pub active: bool,
        /// 已完成的认证数量
        pub verifications_count: u64,
        /// 押金（可选）
        pub deposit: u128,
    }

    /// 认证提供者类型别名
    pub type KycProviderOf<T> = KycProvider<
        <T as frame_system::Config>::AccountId,
        <T as Config>::MaxProviderNameLength,
    >;

    /// 用户 KYC 记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxCidLen))]
    pub struct KycRecord<AccountId, BlockNumber, MaxCidLen: Get<u32>> {
        /// 用户账户
        pub account: AccountId,
        /// 当前 KYC 级别
        pub level: KycLevel,
        /// 当前状态
        pub status: KycStatus,
        /// 认证提供者
        pub provider: Option<AccountId>,
        /// 认证数据 CID（加密存储在 IPFS）
        pub data_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 提交时间
        pub submitted_at: Option<BlockNumber>,
        /// 审核时间
        pub verified_at: Option<BlockNumber>,
        /// 过期时间
        pub expires_at: Option<BlockNumber>,
        /// 拒绝原因
        pub rejection_reason: Option<RejectionReason>,
        /// 拒绝详情 CID
        pub rejection_details_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 国家/地区代码（ISO 3166-1 alpha-2）
        pub country_code: Option<[u8; 2]>,
        /// 风险评分（0-100，越高风险越大）
        pub risk_score: u8,
    }

    /// KYC 记录类型别名
    pub type KycRecordOf<T> = KycRecord<
        <T as frame_system::Config>::AccountId,
        BlockNumberFor<T>,
        <T as Config>::MaxCidLength,
    >;

    /// 实体 KYC 要求配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct EntityKycRequirement {
        /// 最低 KYC 级别要求
        pub min_level: KycLevel,
        /// 是否强制要求
        pub mandatory: bool,
        /// 宽限期（区块数）
        pub grace_period: u32,
        /// 是否允许高风险国家
        pub allow_high_risk_countries: bool,
        /// 最大风险评分
        pub max_risk_score: u8,
    }

    // ==================== 配置 ====================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 运行时事件类型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// CID 最大长度
        #[pallet::constant]
        type MaxCidLength: Get<u32>;

        /// 提供者名称最大长度
        #[pallet::constant]
        type MaxProviderNameLength: Get<u32>;

        /// 最大提供者数量
        #[pallet::constant]
        type MaxProviders: Get<u32>;

        /// 基础 KYC 有效期（区块数）
        #[pallet::constant]
        type BasicKycValidity: Get<BlockNumberFor<Self>>;

        /// 标准 KYC 有效期（区块数）
        #[pallet::constant]
        type StandardKycValidity: Get<BlockNumberFor<Self>>;

        /// 增强 KYC 有效期（区块数）
        #[pallet::constant]
        type EnhancedKycValidity: Get<BlockNumberFor<Self>>;

        /// 管理员 Origin
        type AdminOrigin: frame_support::traits::EnsureOrigin<Self::RuntimeOrigin>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ==================== 存储项 ====================

    /// 用户 KYC 记录
    #[pallet::storage]
    #[pallet::getter(fn kyc_records)]
    pub type KycRecords<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        KycRecordOf<T>,
    >;

    /// 认证提供者列表
    #[pallet::storage]
    #[pallet::getter(fn providers)]
    pub type Providers<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        KycProviderOf<T>,
    >;

    /// 活跃提供者数量
    #[pallet::storage]
    #[pallet::getter(fn provider_count)]
    pub type ProviderCount<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// 实体 KYC 要求配置
    #[pallet::storage]
    #[pallet::getter(fn entity_requirements)]
    pub type EntityRequirements<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        EntityKycRequirement,
    >;

    /// 待审核队列
    #[pallet::storage]
    #[pallet::getter(fn pending_verifications)]
    pub type PendingVerifications<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,  // provider
        BoundedVec<T::AccountId, ConstU32<1000>>,
        ValueQuery,
    >;

    /// 高风险国家列表
    #[pallet::storage]
    #[pallet::getter(fn high_risk_countries)]
    pub type HighRiskCountries<T: Config> = StorageValue<
        _,
        BoundedVec<[u8; 2], ConstU32<50>>,
        ValueQuery,
    >;

    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// KYC 已提交
        KycSubmitted {
            account: T::AccountId,
            level: KycLevel,
        },
        /// KYC 已通过
        KycApproved {
            account: T::AccountId,
            level: KycLevel,
            provider: T::AccountId,
            expires_at: BlockNumberFor<T>,
        },
        /// KYC 已拒绝
        KycRejected {
            account: T::AccountId,
            level: KycLevel,
            reason: RejectionReason,
        },
        /// KYC 已过期
        KycExpired {
            account: T::AccountId,
        },
        /// KYC 已撤销
        KycRevoked {
            account: T::AccountId,
            reason: RejectionReason,
        },
        /// 提供者已注册
        ProviderRegistered {
            provider: T::AccountId,
            name: Vec<u8>,
            provider_type: ProviderType,
        },
        /// 提供者已移除
        ProviderRemoved {
            provider: T::AccountId,
        },
        /// 实体 KYC 要求已设置
        EntityRequirementSet {
            entity_id: u64,
            min_level: KycLevel,
        },
        /// 高风险国家已更新
        HighRiskCountriesUpdated {
            count: u32,
        },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// KYC 记录不存在
        KycNotFound,
        /// 已有待审核的 KYC
        KycAlreadyPending,
        /// KYC 已通过，无需重复提交
        KycAlreadyApproved,
        /// 提供者不存在
        ProviderNotFound,
        /// 提供者已存在
        ProviderAlreadyExists,
        /// 不是认证提供者
        NotAProvider,
        /// 提供者不活跃
        ProviderNotActive,
        /// 无权限
        Unauthorized,
        /// CID 过长
        CidTooLong,
        /// 名称过长
        NameTooLong,
        /// 达到最大提供者数量
        MaxProvidersReached,
        /// 无效的 KYC 状态
        InvalidKycStatus,
        /// 无效的 KYC 级别
        InvalidKycLevel,
        /// 实体要求不存在
        EntityRequirementNotFound,
        /// KYC 级别不满足要求
        InsufficientKycLevel,
        /// 高风险国家
        HighRiskCountry,
        /// 风险评分过高
        RiskScoreTooHigh,
        /// KYC 已过期
        KycExpired,
        /// 提供者不支持此级别
        ProviderLevelNotSupported,
        /// 高风险国家列表超出上限
        TooManyCountries,
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 提交 KYC 认证申请
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(50_000_000, 5_000))]
        pub fn submit_kyc(
            origin: OriginFor<T>,
            level: KycLevel,
            data_cid: Vec<u8>,
            country_code: [u8; 2],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 检查是否已有待审核或已通过的 KYC
            if let Some(record) = KycRecords::<T>::get(&who) {
                ensure!(record.status != KycStatus::Pending, Error::<T>::KycAlreadyPending);
                // H6: 已批准且未过期的 KYC 不允许覆盖提交
                if record.status == KycStatus::Approved {
                    if let Some(expires_at) = record.expires_at {
                        let now = <frame_system::Pallet<T>>::block_number();
                        ensure!(now > expires_at, Error::<T>::KycAlreadyApproved);
                    } else {
                        // 无过期时间的已批准记录不允许覆盖
                        return Err(Error::<T>::KycAlreadyApproved.into());
                    }
                }
                // 允许已过期、已拒绝、已撤销的重新提交
            }

            let data_bounded: BoundedVec<u8, T::MaxCidLength> = 
                data_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            let now = <frame_system::Pallet<T>>::block_number();

            let record = KycRecord {
                account: who.clone(),
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

            KycRecords::<T>::insert(&who, record);

            Self::deposit_event(Event::KycSubmitted {
                account: who,
                level,
            });
            Ok(())
        }

        /// 批准 KYC（认证提供者调用）
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(80_000_000, 6_000))]
        pub fn approve_kyc(
            origin: OriginFor<T>,
            account: T::AccountId,
            risk_score: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证是活跃提供者
            let provider = Providers::<T>::get(&who).ok_or(Error::<T>::ProviderNotFound)?;
            ensure!(provider.active, Error::<T>::ProviderNotActive);

            KycRecords::<T>::try_mutate(&account, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(record.status == KycStatus::Pending, Error::<T>::InvalidKycStatus);
                ensure!(record.level <= provider.max_level, Error::<T>::ProviderLevelNotSupported);

                let now = <frame_system::Pallet<T>>::block_number();
                let validity = Self::get_validity_period(record.level);
                let expires_at = now.saturating_add(validity);

                record.status = KycStatus::Approved;
                record.provider = Some(who.clone());
                record.verified_at = Some(now);
                record.expires_at = Some(expires_at);
                record.risk_score = risk_score;

                Self::deposit_event(Event::KycApproved {
                    account: account.clone(),
                    level: record.level,
                    provider: who.clone(),
                    expires_at,
                });
                Ok(())
            })?;

            // 更新提供者统计
            Providers::<T>::mutate(&who, |maybe_provider| {
                if let Some(p) = maybe_provider {
                    p.verifications_count = p.verifications_count.saturating_add(1);
                }
            });

            Ok(())
        }

        /// 拒绝 KYC（认证提供者调用）
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(60_000_000, 6_000))]
        pub fn reject_kyc(
            origin: OriginFor<T>,
            account: T::AccountId,
            reason: RejectionReason,
            details_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证是活跃提供者
            let provider = Providers::<T>::get(&who).ok_or(Error::<T>::ProviderNotFound)?;
            ensure!(provider.active, Error::<T>::ProviderNotActive);

            let details_bounded = details_cid
                .map(|cid| cid.try_into().map_err(|_| Error::<T>::CidTooLong))
                .transpose()?;

            KycRecords::<T>::try_mutate(&account, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(record.status == KycStatus::Pending, Error::<T>::InvalidKycStatus);

                let now = <frame_system::Pallet<T>>::block_number();

                record.status = KycStatus::Rejected;
                record.provider = Some(who.clone());
                record.verified_at = Some(now);
                record.rejection_reason = Some(reason);
                record.rejection_details_cid = details_bounded;

                Self::deposit_event(Event::KycRejected {
                    account: account.clone(),
                    level: record.level,
                    reason,
                });
                Ok(())
            })
        }

        /// 撤销 KYC（管理员调用）
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(50_000_000, 5_000))]
        pub fn revoke_kyc(
            origin: OriginFor<T>,
            account: T::AccountId,
            reason: RejectionReason,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            KycRecords::<T>::try_mutate(&account, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::KycNotFound)?;
                ensure!(record.status == KycStatus::Approved, Error::<T>::InvalidKycStatus);

                record.status = KycStatus::Revoked;
                record.rejection_reason = Some(reason);

                Self::deposit_event(Event::KycRevoked {
                    account: account.clone(),
                    reason,
                });
                Ok(())
            })
        }

        /// 注册认证提供者
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(60_000_000, 5_000))]
        pub fn register_provider(
            origin: OriginFor<T>,
            provider_account: T::AccountId,
            name: Vec<u8>,
            provider_type: ProviderType,
            max_level: KycLevel,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            ensure!(!Providers::<T>::contains_key(&provider_account), Error::<T>::ProviderAlreadyExists);

            let count = ProviderCount::<T>::get();
            ensure!(count < T::MaxProviders::get(), Error::<T>::MaxProvidersReached);

            let name_bounded: BoundedVec<u8, T::MaxProviderNameLength> = 
                name.clone().try_into().map_err(|_| Error::<T>::NameTooLong)?;

            let provider = KycProvider {
                account: provider_account.clone(),
                name: name_bounded,
                provider_type,
                max_level,
                active: true,
                verifications_count: 0,
                deposit: 0,
            };

            Providers::<T>::insert(&provider_account, provider);
            ProviderCount::<T>::put(count.saturating_add(1));

            Self::deposit_event(Event::ProviderRegistered {
                provider: provider_account,
                name,
                provider_type,
            });
            Ok(())
        }

        /// 移除认证提供者
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn remove_provider(
            origin: OriginFor<T>,
            provider_account: T::AccountId,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            ensure!(Providers::<T>::contains_key(&provider_account), Error::<T>::ProviderNotFound);

            Providers::<T>::remove(&provider_account);
            ProviderCount::<T>::mutate(|count| *count = count.saturating_sub(1));

            Self::deposit_event(Event::ProviderRemoved {
                provider: provider_account,
            });
            Ok(())
        }

        /// 设置实体 KYC 要求
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_entity_requirement(
            origin: OriginFor<T>,
            entity_id: u64,
            min_level: KycLevel,
            mandatory: bool,
            grace_period: u32,
            allow_high_risk_countries: bool,
            max_risk_score: u8,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

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

        /// 更新高风险国家列表
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(50_000_000, 5_000))]
        pub fn update_high_risk_countries(
            origin: OriginFor<T>,
            countries: Vec<[u8; 2]>,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            let bounded: BoundedVec<[u8; 2], ConstU32<50>> = 
                countries.try_into().map_err(|_| Error::<T>::TooManyCountries)?;

            let count = bounded.len() as u32;
            HighRiskCountries::<T>::put(bounded);

            Self::deposit_event(Event::HighRiskCountriesUpdated { count });
            Ok(())
        }
    }

    // ==================== 辅助函数 ====================

    impl<T: Config> Pallet<T> {
        /// 获取 KYC 有效期
        pub fn get_validity_period(level: KycLevel) -> BlockNumberFor<T> {
            match level {
                KycLevel::None => BlockNumberFor::<T>::from(0u32),
                KycLevel::Basic => T::BasicKycValidity::get(),
                KycLevel::Standard => T::StandardKycValidity::get(),
                KycLevel::Enhanced | KycLevel::Institutional => T::EnhancedKycValidity::get(),
            }
        }

        /// 检查用户是否满足 KYC 要求
        pub fn meets_kyc_requirement(account: &T::AccountId, min_level: KycLevel) -> bool {
            if let Some(record) = KycRecords::<T>::get(account) {
                if record.status != KycStatus::Approved {
                    return false;
                }
                // 检查是否过期
                if let Some(expires_at) = record.expires_at {
                    let now = <frame_system::Pallet<T>>::block_number();
                    if now > expires_at {
                        return false;
                    }
                }
                record.level >= min_level
            } else {
                min_level == KycLevel::None
            }
        }

        /// 获取用户 KYC 级别
        pub fn get_kyc_level(account: &T::AccountId) -> KycLevel {
            KycRecords::<T>::get(account)
                .filter(|r| r.status == KycStatus::Approved)
                .map(|r| r.level)
                .unwrap_or(KycLevel::None)
        }

        /// 检查用户是否来自高风险国家
        pub fn is_high_risk_country(account: &T::AccountId) -> bool {
            if let Some(record) = KycRecords::<T>::get(account) {
                if let Some(country) = record.country_code {
                    return HighRiskCountries::<T>::get().contains(&country);
                }
            }
            false
        }

        /// 检查用户是否可以参与实体活动
        pub fn can_participate_in_entity(account: &T::AccountId, entity_id: u64) -> bool {
            if let Some(requirement) = EntityRequirements::<T>::get(entity_id) {
                if !requirement.mandatory {
                    return true;
                }

                if let Some(record) = KycRecords::<T>::get(account) {
                    // 检查状态
                    if record.status != KycStatus::Approved {
                        return false;
                    }
                    // 检查级别
                    if record.level < requirement.min_level {
                        return false;
                    }
                    // 检查高风险国家
                    if !requirement.allow_high_risk_countries {
                        if let Some(country) = record.country_code {
                            if HighRiskCountries::<T>::get().contains(&country) {
                                return false;
                            }
                        }
                    }
                    // 检查风险评分
                    if record.risk_score > requirement.max_risk_score {
                        return false;
                    }
                    // 检查过期
                    if let Some(expires_at) = record.expires_at {
                        let now = <frame_system::Pallet<T>>::block_number();
                        if now > expires_at {
                            return false;
                        }
                    }
                    return true;
                }
                return false;
            }
            // 无要求时默认允许
            true
        }

        /// 获取用户风险评分
        pub fn get_risk_score(account: &T::AccountId) -> u8 {
            KycRecords::<T>::get(account)
                .map(|r| r.risk_score)
                .unwrap_or(100) // 未认证用户最高风险
        }
    }
}
