//! # 商城评价管理模块 (pallet-entity-review)
//!
//! ## 概述
//!
//! 本模块负责订单评价管理，包括：
//! - 订单完成后提交评价
//! - 评分和评价内容
//! - 店铺评分更新
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-01-31): 从 pallet-mall 拆分

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use crate::weights::WeightInfo;
    use frame_support::{
        pallet_prelude::*,
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{EntityProvider, OrderProvider, ShopProvider, AdminPermission};

    /// 订单评价
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxCidLen))]
    pub struct MallReview<AccountId, BlockNumber, MaxCidLen: Get<u32>> {
        /// 订单 ID
        pub order_id: u64,
        /// 评价者
        pub reviewer: AccountId,
        /// 评分 (1-5)
        pub rating: u8,
        /// 评价内容 IPFS CID
        pub content_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 评价时间
        pub created_at: BlockNumber,
        /// F3: 商品 ID（可选，来自 OrderProvider）
        pub product_id: Option<u64>,
        /// F6: 是否已编辑
        pub edited: bool,
    }

    /// F1: 商家回复评价
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxCidLen))]
    pub struct ReviewReply<AccountId, BlockNumber, MaxCidLen: Get<u32>> {
        /// 回复者（商家/管理员）
        pub replier: AccountId,
        /// 回复内容 IPFS CID
        pub content_cid: BoundedVec<u8, MaxCidLen>,
        /// 回复时间
        pub created_at: BlockNumber,
    }

    /// F1: 回复类型别名
    pub type ReviewReplyOf<T> = ReviewReply<
        <T as frame_system::Config>::AccountId,
        BlockNumberFor<T>,
        <T as Config>::MaxCidLength,
    >;

    /// 评价类型别名
    pub type MallReviewOf<T> = MallReview<
        <T as frame_system::Config>::AccountId,
        BlockNumberFor<T>,
        <T as Config>::MaxCidLength,
    >;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// 实体查询接口
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// 订单查询接口
        type OrderProvider: OrderProvider<Self::AccountId, u128>;

        /// 店铺更新接口
        type ShopProvider: ShopProvider<Self::AccountId>;

        /// CID 最大长度
        #[pallet::constant]
        type MaxCidLength: Get<u32>;

        /// 每用户最大评价数
        #[pallet::constant]
        type MaxReviewsPerUser: Get<u32>;

        /// 评价时间窗口（区块数），订单完成后超过此区块数则不可评价
        /// 设为 0 表示不限制
        #[pallet::constant]
        type ReviewWindowBlocks: Get<u64>;

        /// F6: 评价修改时间窗口（区块数），评价提交后超过此区块数则不可修改
        /// 设为 0 表示不限制
        #[pallet::constant]
        type EditWindowBlocks: Get<u64>;

        /// F3: 每个商品最大评价索引数
        #[pallet::constant]
        type MaxProductReviews: Get<u32>;

        /// 权重信息
        type WeightInfo: WeightInfo;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// M1: Config 参数完整性校验
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        #[cfg(feature = "std")]
        fn integrity_test() {
            assert!(T::MaxCidLength::get() > 0, "MaxCidLength must be > 0");
            assert!(T::MaxReviewsPerUser::get() > 0, "MaxReviewsPerUser must be > 0");
            assert!(T::MaxProductReviews::get() > 0, "MaxProductReviews must be > 0");
        }
    }

    // ==================== 存储项 ====================

    /// 订单评价存储
    #[pallet::storage]
    #[pallet::getter(fn reviews)]
    pub type Reviews<T: Config> = StorageMap<_, Blake2_128Concat, u64, MallReviewOf<T>>;

    /// 评价统计
    #[pallet::storage]
    #[pallet::getter(fn review_count)]
    pub type ReviewCount<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 店铺评价计数
    #[pallet::storage]
    #[pallet::getter(fn shop_review_count)]
    pub type ShopReviewCount<T: Config> = StorageMap<_, Blake2_128Concat, u64, u64, ValueQuery>;

    /// 用户评价索引（用户 → 已评价的 order_id 列表）
    #[pallet::storage]
    #[pallet::getter(fn user_reviews)]
    pub type UserReviews<T: Config> = StorageMap<
        _, Blake2_128Concat, T::AccountId,
        BoundedVec<u64, T::MaxReviewsPerUser>, ValueQuery,
    >;

    /// Entity 评价关闭标记（存在 = 已关闭评价，不存在 = 开启评价）
    #[pallet::storage]
    #[pallet::getter(fn entity_review_disabled)]
    pub type EntityReviewDisabled<T: Config> = StorageMap<_, Blake2_128Concat, u64, (), OptionQuery>;

    /// F1: 商家回复存储（order_id → ReviewReply）
    #[pallet::storage]
    #[pallet::getter(fn review_replies)]
    pub type ReviewReplies<T: Config> = StorageMap<_, Blake2_128Concat, u64, ReviewReplyOf<T>>;

    /// F3: 商品评价索引（product_id → order_id 列表）
    #[pallet::storage]
    #[pallet::getter(fn product_reviews)]
    pub type ProductReviews<T: Config> = StorageMap<
        _, Blake2_128Concat, u64,
        BoundedVec<u64, T::MaxProductReviews>, ValueQuery,
    >;

    /// F3: 商品评价计数
    #[pallet::storage]
    #[pallet::getter(fn product_review_count)]
    pub type ProductReviewCount<T: Config> = StorageMap<_, Blake2_128Concat, u64, u64, ValueQuery>;

    /// F3: 商品评分总和（用于计算平均分）
    #[pallet::storage]
    #[pallet::getter(fn product_rating_sum)]
    pub type ProductRatingSum<T: Config> = StorageMap<_, Blake2_128Concat, u64, u64, ValueQuery>;

    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 评价已提交
        ReviewSubmitted {
            order_id: u64,
            reviewer: T::AccountId,
            shop_id: Option<u64>,
            rating: u8,
        },
        /// 店铺评分更新失败（评价仍已记录）
        ShopRatingUpdateFailed {
            order_id: u64,
            shop_id: u64,
        },
        /// Entity 评价配置已更新
        ReviewConfigUpdated {
            entity_id: u64,
            enabled: bool,
        },
        /// M4: 评价已被 Root 移除
        ReviewRemoved {
            order_id: u64,
            reviewer: T::AccountId,
        },
        /// F1: 商家已回复评价
        ReviewReplied {
            order_id: u64,
            replier: T::AccountId,
        },
        /// F3: 商品评价已索引
        ProductReviewIndexed {
            product_id: u64,
            order_id: u64,
        },
        /// F6: 评价已修改
        ReviewEdited {
            order_id: u64,
            reviewer: T::AccountId,
            old_rating: u8,
            new_rating: u8,
        },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// 订单不存在
        OrderNotFound,
        /// 不是订单买家
        NotOrderBuyer,
        /// 订单未完成
        OrderNotCompleted,
        /// 已评价过
        AlreadyReviewed,
        /// 无效的评分
        InvalidRating,
        /// CID 过长
        CidTooLong,
        /// CID 为空
        EmptyCid,
        /// 用户评价数已达上限
        UserReviewLimitReached,
        /// 该 Entity 已关闭评价功能
        ReviewsDisabledForEntity,
        /// 不是 Entity 管理员
        NotEntityAdmin,
        /// Entity 不存在
        EntityNotFound,
        /// Entity 未激活
        EntityNotActive,
        /// 订单处于争议状态
        OrderDisputed,
        /// 评价计数溢出
        ReviewCountOverflow,
        /// 评价时间窗口已过期
        ReviewWindowExpired,
        /// 评价不存在（Root 删除时）
        ReviewNotFound,
        /// 实体已被全局锁定
        EntityLocked,
        /// F1: 该评价已有回复
        AlreadyReplied,
        /// F1: 回复内容 CID 为空
        ReplyContentEmpty,
        /// F1: 不是店铺关联的 Entity 管理员
        NotShopEntityAdmin,
        /// F3: 商品评价索引已满
        ProductReviewsFull,
        /// F6: 评价修改窗口已过期
        EditWindowExpired,
        /// F6: 评价已被修改过（仅允许修改一次）
        AlreadyEdited,
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 提交评价
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::submit_review())]
        pub fn submit_review(
            origin: OriginFor<T>,
            order_id: u64,
            rating: u8,
            content_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证评分范围
            ensure!(rating >= 1 && rating <= 5, Error::<T>::InvalidRating);

            // 验证订单
            let buyer = T::OrderProvider::order_buyer(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(buyer == who, Error::<T>::NotOrderBuyer);
            ensure!(T::OrderProvider::is_order_completed(order_id), Error::<T>::OrderNotCompleted);
            ensure!(!T::OrderProvider::is_order_disputed(order_id), Error::<T>::OrderDisputed);
            ensure!(!Reviews::<T>::contains_key(order_id), Error::<T>::AlreadyReviewed);

            // 评价时间窗口检查
            let window = T::ReviewWindowBlocks::get();
            if window > 0 {
                if let Some(completed_at) = T::OrderProvider::order_completed_at(order_id) {
                    let now_u64: u64 = <frame_system::Pallet<T>>::block_number()
                        .try_into().unwrap_or(u64::MAX);
                    ensure!(
                        now_u64.saturating_sub(completed_at) <= window,
                        Error::<T>::ReviewWindowExpired
                    );
                }
            }

            // M2-R6: 统一获取 shop_id，避免重复调用 order_shop_id
            let shop_id = T::OrderProvider::order_shop_id(order_id);

            // M1-R7: 统一获取 entity_id，避免重复调用 shop_entity_id
            let entity_id = shop_id.and_then(|sid| T::ShopProvider::shop_entity_id(sid));

            // 检查 Entity 是否关闭了评价功能
            if let Some(eid) = entity_id {
                ensure!(!EntityReviewDisabled::<T>::contains_key(eid), Error::<T>::ReviewsDisabledForEntity);
            }

            // 转换 CID
            let content_cid: Option<BoundedVec<u8, T::MaxCidLength>> = content_cid
                .map(|c| {
                    ensure!(!c.is_empty(), Error::<T>::EmptyCid);
                    c.try_into().map_err(|_| Error::<T>::CidTooLong)
                })
                .transpose()?;

            let now = <frame_system::Pallet<T>>::block_number();

            // F3: 获取商品 ID
            let product_id = T::OrderProvider::order_product_id(order_id);

            let review = MallReview {
                order_id,
                reviewer: who.clone(),
                rating,
                content_cid,
                created_at: now,
                product_id,
                edited: false,
            };

            // H2: 更新用户评价索引
            UserReviews::<T>::try_mutate(&who, |reviews| {
                reviews.try_push(order_id).map_err(|_| Error::<T>::UserReviewLimitReached)
            })?;

            Reviews::<T>::insert(order_id, review);
            ReviewCount::<T>::try_mutate(|c| {
                *c = c.checked_add(1).ok_or(Error::<T>::ReviewCountOverflow)?;
                Ok::<(), Error<T>>(())
            })?;

            // 更新店铺评分（best-effort，失败不回滚评价）
            if let Some(sid) = shop_id {
                match T::ShopProvider::update_shop_rating(sid, rating) {
                    Ok(_) => {
                        // M1-R7: ShopReviewCount 溢出也 best-effort，不阻塞评价
                        ShopReviewCount::<T>::mutate(sid, |c| {
                            if let Some(new_val) = c.checked_add(1) {
                                *c = new_val;
                            } else {
                                log::warn!(
                                    "ShopReviewCount overflow for shop {}, count stuck at {}",
                                    sid, *c
                                );
                            }
                        });
                    },
                    Err(e) => {
                        log::warn!(
                            "update_shop_rating failed for shop {} order {}: {:?}",
                            sid, order_id, e
                        );
                        Self::deposit_event(Event::ShopRatingUpdateFailed {
                            order_id,
                            shop_id: sid,
                        });
                    },
                }

                // Note: Entity 级别评分已移至 Shop 层级，此处不再更新 Entity 评分
            }

            // F3: 更新商品评价索引（best-effort）
            if let Some(pid) = product_id {
                let indexed = ProductReviews::<T>::try_mutate(pid, |reviews| {
                    reviews.try_push(order_id).map_err(|_| Error::<T>::ProductReviewsFull)
                });
                if indexed.is_ok() {
                    ProductReviewCount::<T>::mutate(pid, |c| *c = c.saturating_add(1));
                    ProductRatingSum::<T>::mutate(pid, |s| *s = s.saturating_add(rating as u64));
                    Self::deposit_event(Event::ProductReviewIndexed {
                        product_id: pid,
                        order_id,
                    });
                } else {
                    log::warn!("ProductReviews full for product {}, order {} not indexed", pid, order_id);
                }
            }

            Self::deposit_event(Event::ReviewSubmitted {
                order_id,
                reviewer: who,
                shop_id,
                rating,
            });

            Ok(())
        }

        /// M4: Root 移除违规评价
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::remove_review())]
        pub fn remove_review(
            origin: OriginFor<T>,
            order_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let review = Reviews::<T>::take(order_id)
                .ok_or(Error::<T>::ReviewNotFound)?;

            // 递减全局计数（best-effort）
            ReviewCount::<T>::mutate(|c| *c = c.saturating_sub(1));

            // 递减店铺计数（best-effort）
            let shop_id = T::OrderProvider::order_shop_id(order_id);
            if let Some(sid) = shop_id {
                ShopReviewCount::<T>::mutate(sid, |c| *c = c.saturating_sub(1));
            }

            // 从用户索引中移除
            UserReviews::<T>::mutate(&review.reviewer, |reviews| {
                reviews.retain(|&id| id != order_id);
            });

            // F3: 清理商品评价索引
            if let Some(pid) = review.product_id {
                ProductReviews::<T>::mutate(pid, |reviews| {
                    reviews.retain(|&id| id != order_id);
                });
                ProductReviewCount::<T>::mutate(pid, |c| *c = c.saturating_sub(1));
                ProductRatingSum::<T>::mutate(pid, |s| *s = s.saturating_sub(review.rating as u64));
            }

            // F1: 清理商家回复
            ReviewReplies::<T>::remove(order_id);

            Self::deposit_event(Event::ReviewRemoved {
                order_id,
                reviewer: review.reviewer,
            });

            Ok(())
        }

        /// 设置 Entity 评价开关
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::set_review_enabled())]
        pub fn set_review_enabled(
            origin: OriginFor<T>,
            entity_id: u64,
            enabled: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::REVIEW_MANAGE), Error::<T>::NotEntityAdmin);

            let currently_disabled = EntityReviewDisabled::<T>::contains_key(entity_id);
            let want_disabled = !enabled;

            // H2: 仅在状态实际变更时写入存储和发射事件
            if currently_disabled != want_disabled {
                if enabled {
                    EntityReviewDisabled::<T>::remove(entity_id);
                } else {
                    EntityReviewDisabled::<T>::insert(entity_id, ());
                }

                Self::deposit_event(Event::ReviewConfigUpdated { entity_id, enabled });
            }

            Ok(())
        }

        /// F1: 商家回复评价
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::reply_to_review())]
        pub fn reply_to_review(
            origin: OriginFor<T>,
            order_id: u64,
            content_cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 评价必须存在
            ensure!(Reviews::<T>::contains_key(order_id), Error::<T>::ReviewNotFound);
            // 不能重复回复
            ensure!(!ReviewReplies::<T>::contains_key(order_id), Error::<T>::AlreadyReplied);

            // 回复内容不能为空
            ensure!(!content_cid.is_empty(), Error::<T>::ReplyContentEmpty);
            let bounded_cid: BoundedVec<u8, T::MaxCidLength> = content_cid
                .try_into().map_err(|_| Error::<T>::CidTooLong)?;

            // 权限检查：必须是订单关联店铺的 Entity Owner 或 Admin (REVIEW_MANAGE)
            let shop_id = T::OrderProvider::order_shop_id(order_id)
                .ok_or(Error::<T>::NotShopEntityAdmin)?;
            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::NotShopEntityAdmin)?;
            // M2-R10: Entity 必须处于激活状态
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::REVIEW_MANAGE),
                Error::<T>::NotShopEntityAdmin
            );

            let now = <frame_system::Pallet<T>>::block_number();

            let reply = ReviewReply {
                replier: who.clone(),
                content_cid: bounded_cid,
                created_at: now,
            };

            ReviewReplies::<T>::insert(order_id, reply);

            Self::deposit_event(Event::ReviewReplied {
                order_id,
                replier: who,
            });

            Ok(())
        }

        /// F6: 买家修改评价（仅允许修改一次，在时间窗口内）
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::edit_review())]
        pub fn edit_review(
            origin: OriginFor<T>,
            order_id: u64,
            new_rating: u8,
            new_content_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证新评分范围
            ensure!(new_rating >= 1 && new_rating <= 5, Error::<T>::InvalidRating);

            // 评价必须存在
            let mut review = Reviews::<T>::get(order_id)
                .ok_or(Error::<T>::ReviewNotFound)?;

            // 必须是评价者本人
            ensure!(review.reviewer == who, Error::<T>::NotOrderBuyer);

            // 仅允许修改一次
            ensure!(!review.edited, Error::<T>::AlreadyEdited);

            // M1-R10: 检查 Entity 评价开关（评价关闭时不允许修改）
            let shop_id = T::OrderProvider::order_shop_id(order_id);
            let entity_id = shop_id.and_then(|sid| T::ShopProvider::shop_entity_id(sid));
            if let Some(eid) = entity_id {
                ensure!(!EntityReviewDisabled::<T>::contains_key(eid), Error::<T>::ReviewsDisabledForEntity);
            }

            // 修改时间窗口检查
            let edit_window = T::EditWindowBlocks::get();
            if edit_window > 0 {
                let created_u64: u64 = review.created_at
                    .try_into().unwrap_or(u64::MAX);
                let now_u64: u64 = <frame_system::Pallet<T>>::block_number()
                    .try_into().unwrap_or(u64::MAX);
                ensure!(
                    now_u64.saturating_sub(created_u64) <= edit_window,
                    Error::<T>::EditWindowExpired
                );
            }

            // 转换新 CID
            let new_cid: Option<BoundedVec<u8, T::MaxCidLength>> = new_content_cid
                .map(|c| {
                    ensure!(!c.is_empty(), Error::<T>::EmptyCid);
                    c.try_into().map_err(|_| Error::<T>::CidTooLong)
                })
                .transpose()?;

            let old_rating = review.rating;

            // 更新评价
            review.rating = new_rating;
            review.content_cid = new_cid;
            review.edited = true;

            Reviews::<T>::insert(order_id, review);

            // H1-R10: 更新商品评分差值（ShopProvider::update_shop_rating 是追加模式，
            // 无法减去旧评分，故 edit 不更新店铺评分，仅更新商品评分）
            if old_rating != new_rating {
                // F3: 更新商品评分（ProductRatingSum 是模块自管理的，可精确修正）
                let product_id = T::OrderProvider::order_product_id(order_id);
                if let Some(pid) = product_id {
                    ProductRatingSum::<T>::mutate(pid, |s| {
                        *s = s.saturating_sub(old_rating as u64)
                            .saturating_add(new_rating as u64);
                    });
                }
            }

            Self::deposit_event(Event::ReviewEdited {
                order_id,
                reviewer: who,
                old_rating,
                new_rating,
            });

            Ok(())
        }
    }
}
