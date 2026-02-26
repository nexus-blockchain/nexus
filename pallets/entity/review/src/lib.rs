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
    use pallet_entity_common::{OrderProvider, ShopProvider};

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
    }

    /// 评价类型别名
    pub type MallReviewOf<T> = MallReview<
        <T as frame_system::Config>::AccountId,
        BlockNumberFor<T>,
        <T as Config>::MaxCidLength,
    >;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// 订单查询接口
        type OrderProvider: OrderProvider<Self::AccountId, u128>;

        /// 店铺更新接口
        type ShopProvider: ShopProvider<Self::AccountId>;

        /// CID 最大长度
        #[pallet::constant]
        type MaxCidLength: Get<u32>;

        /// 权重信息
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

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
            ensure!(!Reviews::<T>::contains_key(order_id), Error::<T>::AlreadyReviewed);

            // 转换 CID
            let content_cid: Option<BoundedVec<u8, T::MaxCidLength>> = content_cid
                .map(|c| {
                    ensure!(!c.is_empty(), Error::<T>::EmptyCid);
                    c.try_into().map_err(|_| Error::<T>::CidTooLong)
                })
                .transpose()?;

            let now = <frame_system::Pallet<T>>::block_number();

            let review = MallReview {
                order_id,
                reviewer: who.clone(),
                rating,
                content_cid,
                created_at: now,
            };

            Reviews::<T>::insert(order_id, review);
            ReviewCount::<T>::mutate(|c| *c = c.saturating_add(1));

            // 更新店铺评分
            let shop_id = T::OrderProvider::order_shop_id(order_id);
            if let Some(sid) = shop_id {
                T::ShopProvider::update_shop_rating(sid, rating)?;
                ShopReviewCount::<T>::mutate(sid, |c| *c = c.saturating_add(1));
            }

            Self::deposit_event(Event::ReviewSubmitted {
                order_id,
                reviewer: who,
                shop_id,
                rating,
            });

            Ok(())
        }
    }
}
