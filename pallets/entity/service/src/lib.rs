//! # 商城商品管理模块 (pallet-entity-service)
//!
//! ## 概述
//!
//! 本模块负责商品的生命周期管理，包括：
//! - 商品创建（从店铺派生账户扣取押金）
//! - 商品信息更新
//! - 商品上架/下架
//! - 商品删除（退还押金到店铺派生账户）
//! - 库存管理
//!
//! ## 押金机制
//!
//! - 创建商品时从店铺派生账户扣取 1 USDT 等值 NEX
//! - 押金转入 Pallet 账户托管
//! - 删除商品时从 Pallet 账户退还到店铺派生账户
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-01-31): 从 pallet-mall 拆分
//! - v0.2.0 (2026-02-01): 实现从店铺派生账户扣取押金机制

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;
pub use pallet_entity_common::{ProductCategory, ProductStatus};

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
        traits::{Currency, ExistenceRequirement},
        BoundedVec, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{PricingProvider, ProductCategory, ProductProvider, ProductStatus, EntityProvider, ShopProvider};
    use sp_runtime::{
        traits::{AccountIdConversion, Zero},
        SaturatedConversion,
    };

    /// 商品押金托管 PalletId
    const PRODUCT_PALLET_ID: PalletId = PalletId(*b"et/prod/");

    /// 货币余额类型别名
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// 商品信息
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxCidLen))]
    pub struct Product<Balance, BlockNumber, MaxCidLen: Get<u32>> {
        /// 商品 ID
        pub id: u64,
        /// 所属店铺 ID
        pub shop_id: u64,
        /// 商品名称 IPFS CID
        pub name_cid: BoundedVec<u8, MaxCidLen>,
        /// 商品图片 IPFS CID
        pub images_cid: BoundedVec<u8, MaxCidLen>,
        /// 商品详情 IPFS CID
        pub detail_cid: BoundedVec<u8, MaxCidLen>,
        /// 单价
        pub price: Balance,
        /// 库存数量（0 表示无限）
        pub stock: u32,
        /// 已售数量
        pub sold_count: u32,
        /// 商品状态
        pub status: ProductStatus,
        /// 商品类别
        pub category: ProductCategory,
        /// 创建时间
        pub created_at: BlockNumber,
        /// 更新时间
        pub updated_at: BlockNumber,
    }

    /// 商品类型别名
    pub type ProductOf<T> = Product<
        BalanceOf<T>,
        BlockNumberFor<T>,
        <T as Config>::MaxCidLength,
    >;

    /// 商品统计
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct ProductStatistics {
        /// 总商品数
        pub total_products: u64,
        /// 在售商品数
        pub on_sale_products: u64,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 运行时事件类型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 货币类型
        type Currency: Currency<Self::AccountId>;

        /// 实体查询接口
        /// 注意：当前未直接调用，因为 ShopProvider::is_shop_active 已隐式检查 Entity 状态。
        /// 保留此关联类型供未来扩展使用（如直接查询 Entity 元数据）。
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// Shop 查询接口（Entity-Shop 分离架构）
        type ShopProvider: ShopProvider<Self::AccountId>;

        /// 定价提供者（用于计算 USDT 等值 NEX 押金）
        type PricingProvider: PricingProvider;

        /// 每店铺最大商品数
        #[pallet::constant]
        type MaxProductsPerShop: Get<u32>;

        /// CID 最大长度
        #[pallet::constant]
        type MaxCidLength: Get<u32>;

        /// 商品押金 USDT 金额（精度 10^6，即 1_000_000 = 1 USDT）
        #[pallet::constant]
        type ProductDepositUsdt: Get<u64>;

        /// 最小押金 NEX
        #[pallet::constant]
        type MinProductDepositCos: Get<BalanceOf<Self>>;

        /// 最大押金 NEX
        #[pallet::constant]
        type MaxProductDepositCos: Get<BalanceOf<Self>>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ==================== 存储项 ====================

    /// 下一个商品 ID
    #[pallet::storage]
    #[pallet::getter(fn next_product_id)]
    pub type NextProductId<T> = StorageValue<_, u64, ValueQuery>;

    /// 商品存储
    #[pallet::storage]
    #[pallet::getter(fn products)]
    pub type Products<T: Config> = StorageMap<_, Blake2_128Concat, u64, ProductOf<T>>;

    /// 店铺商品索引
    #[pallet::storage]
    #[pallet::getter(fn shop_products)]
    pub type ShopProducts<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        BoundedVec<u64, T::MaxProductsPerShop>,
        ValueQuery,
    >;

    /// 商品统计
    #[pallet::storage]
    #[pallet::getter(fn product_stats)]
    pub type ProductStats<T: Config> = StorageValue<_, ProductStatistics, ValueQuery>;

    /// 商品押金记录（商品ID -> 押金信息）
    #[pallet::storage]
    #[pallet::getter(fn product_deposits)]
    pub type ProductDeposits<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // product_id
        ProductDepositInfo<T::AccountId, BalanceOf<T>>,
    >;

    /// 押金信息
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct ProductDepositInfo<AccountId, Balance> {
        /// 所属店铺 ID
        pub shop_id: u64,
        /// 押金金额
        pub amount: Balance,
        /// 押金来源账户（店铺派生账户）
        pub source_account: AccountId,
    }

    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 商品已创建（含押金）
        ProductCreated {
            product_id: u64,
            shop_id: u64,
            deposit: BalanceOf<T>,
        },
        /// 商品已更新
        ProductUpdated { product_id: u64 },
        /// 商品状态已变更
        ProductStatusChanged { product_id: u64, status: ProductStatus },
        /// 商品已删除（押金已退还）
        ProductDeleted {
            product_id: u64,
            deposit_refunded: BalanceOf<T>,
        },
        /// 库存已更新
        StockUpdated { product_id: u64, new_stock: u32 },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// 商品不存在
        ProductNotFound,
        /// 店铺不存在
        ShopNotFound,
        /// 不是店主
        NotShopOwner,
        /// 店铺未激活
        ShopNotActive,
        /// 库存不足
        InsufficientStock,
        /// 达到最大商品数
        MaxProductsReached,
        /// 无效的商品状态
        InvalidProductStatus,
        /// CID 过长
        CidTooLong,
        /// 店铺运营资金不足以支付押金
        InsufficientShopFund,
        /// 押金记录不存在
        DepositNotFound,
        /// 价格不可用
        PriceUnavailable,
        /// 算术溢出
        ArithmeticOverflow,
        /// 商品价格无效（不能为 0）
        InvalidPrice,
        /// CID 内容不能为空
        EmptyCid,
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 创建商品（从店铺派生账户扣取押金）
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(250_000_000, 12_000))]
        pub fn create_product(
            origin: OriginFor<T>,
            shop_id: u64,
            name_cid: Vec<u8>,
            images_cid: Vec<u8>,
            detail_cid: Vec<u8>,
            price: BalanceOf<T>,
            stock: u32,
            category: ProductCategory,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H4: 商品价格不能为零
            ensure!(!price.is_zero(), Error::<T>::InvalidPrice);

            // H2: name_cid 不能为空
            ensure!(!name_cid.is_empty(), Error::<T>::EmptyCid);

            // 验证店铺
            ensure!(T::ShopProvider::shop_exists(shop_id), Error::<T>::ShopNotFound);
            ensure!(T::ShopProvider::is_shop_active(shop_id), Error::<T>::ShopNotActive);
            let owner = T::ShopProvider::shop_owner(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            // 检查商品数量限制
            let product_ids = ShopProducts::<T>::get(shop_id);
            ensure!(
                product_ids.len() < T::MaxProductsPerShop::get() as usize,
                Error::<T>::MaxProductsReached
            );

            // H1: CID 校验移到转账前，避免无谓转账+回滚
            let name_cid: BoundedVec<u8, T::MaxCidLength> =
                name_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
            let images_cid: BoundedVec<u8, T::MaxCidLength> =
                images_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
            let detail_cid: BoundedVec<u8, T::MaxCidLength> =
                detail_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            // 计算押金（1 USDT 等值 NEX）
            let deposit = Self::calculate_product_deposit()?;

            // 获取店铺派生账户
            let shop_account = T::ShopProvider::shop_account(shop_id);
            let shop_balance = T::Currency::free_balance(&shop_account);
            ensure!(shop_balance >= deposit, Error::<T>::InsufficientShopFund);

            // 从店铺派生账户转入 Pallet 账户
            // L2: 使用 KeepAlive 防止 reap 店铺派生账户
            let pallet_account: T::AccountId = PRODUCT_PALLET_ID.into_account_truncating();
            T::Currency::transfer(
                &shop_account,
                &pallet_account,
                deposit,
                ExistenceRequirement::KeepAlive,
            )?;

            let product_id = NextProductId::<T>::get();
            let now = <frame_system::Pallet<T>>::block_number();

            let product = Product {
                id: product_id,
                shop_id,
                name_cid,
                images_cid,
                detail_cid,
                price,
                stock,
                sold_count: 0,
                status: ProductStatus::Draft,
                category,
                created_at: now,
                updated_at: now,
            };

            Products::<T>::insert(product_id, product);
            ShopProducts::<T>::try_mutate(shop_id, |ids| ids.try_push(product_id))
                .map_err(|_| Error::<T>::MaxProductsReached)?;
            NextProductId::<T>::put(product_id.saturating_add(1));

            // 记录押金信息
            ProductDeposits::<T>::insert(product_id, ProductDepositInfo {
                shop_id,
                amount: deposit,
                source_account: shop_account,
            });

            ProductStats::<T>::mutate(|stats| {
                stats.total_products = stats.total_products.saturating_add(1);
            });

            Self::deposit_event(Event::ProductCreated {
                product_id,
                shop_id,
                deposit,
            });
            Ok(())
        }

        /// 更新商品信息
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn update_product(
            origin: OriginFor<T>,
            product_id: u64,
            name_cid: Option<Vec<u8>>,
            images_cid: Option<Vec<u8>>,
            detail_cid: Option<Vec<u8>>,
            price: Option<BalanceOf<T>>,
            stock: Option<u32>,
            category: Option<ProductCategory>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Products::<T>::try_mutate(product_id, |maybe_product| -> DispatchResult {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;

                // 验证店主
                let owner = T::ShopProvider::shop_owner(product.shop_id)
                    .ok_or(Error::<T>::ShopNotFound)?;
                ensure!(owner == who, Error::<T>::NotShopOwner);

                if let Some(c) = name_cid {
                    // H2: name_cid 不能为空
                    ensure!(!c.is_empty(), Error::<T>::EmptyCid);
                    product.name_cid = c.try_into().map_err(|_| Error::<T>::CidTooLong)?;
                }
                if let Some(c) = images_cid {
                    product.images_cid = c.try_into().map_err(|_| Error::<T>::CidTooLong)?;
                }
                if let Some(c) = detail_cid {
                    product.detail_cid = c.try_into().map_err(|_| Error::<T>::CidTooLong)?;
                }
                if let Some(p) = price {
                    ensure!(!p.is_zero(), Error::<T>::InvalidPrice);
                    product.price = p;
                }
                if let Some(s) = stock {
                    product.stock = s;
                    if s > 0 && product.status == ProductStatus::SoldOut {
                        // H1: 补货恢复上架时检查 Shop 激活状态
                        ensure!(
                            T::ShopProvider::is_shop_active(product.shop_id),
                            Error::<T>::ShopNotActive
                        );
                        product.status = ProductStatus::OnSale;
                        // M3: 补货恢复在售统计
                        ProductStats::<T>::mutate(|stats| {
                            stats.on_sale_products = stats.on_sale_products.saturating_add(1);
                        });
                    }
                }
                if let Some(c) = category {
                    product.category = c;
                }

                product.updated_at = <frame_system::Pallet<T>>::block_number();
                Ok(())
            })?;

            Self::deposit_event(Event::ProductUpdated { product_id });
            Ok(())
        }

        /// 上架商品
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(120_000_000, 6_000))]
        pub fn publish_product(origin: OriginFor<T>, product_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Products::<T>::try_mutate(product_id, |maybe_product| -> DispatchResult {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;

                let owner = T::ShopProvider::shop_owner(product.shop_id)
                    .ok_or(Error::<T>::ShopNotFound)?;
                ensure!(owner == who, Error::<T>::NotShopOwner);
                ensure!(T::ShopProvider::is_shop_active(product.shop_id), Error::<T>::ShopNotActive);
                // C3: 只能从 Draft/OffShelf 上架，防止重复计数
                ensure!(
                    product.status == ProductStatus::Draft || product.status == ProductStatus::OffShelf,
                    Error::<T>::InvalidProductStatus
                );

                product.status = ProductStatus::OnSale;
                product.updated_at = <frame_system::Pallet<T>>::block_number();
                Ok(())
            })?;

            ProductStats::<T>::mutate(|stats| {
                stats.on_sale_products = stats.on_sale_products.saturating_add(1);
            });

            Self::deposit_event(Event::ProductStatusChanged {
                product_id,
                status: ProductStatus::OnSale,
            });
            Ok(())
        }

        /// 下架商品
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(120_000_000, 6_000))]
        pub fn unpublish_product(origin: OriginFor<T>, product_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Products::<T>::try_mutate(product_id, |maybe_product| -> DispatchResult {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;

                let owner = T::ShopProvider::shop_owner(product.shop_id)
                    .ok_or(Error::<T>::ShopNotFound)?;
                ensure!(owner == who, Error::<T>::NotShopOwner);
                // C4: 只能从 OnSale/SoldOut 下架，防止统计错误
                ensure!(
                    product.status == ProductStatus::OnSale || product.status == ProductStatus::SoldOut,
                    Error::<T>::InvalidProductStatus
                );

                let was_on_sale = product.status == ProductStatus::OnSale;
                product.status = ProductStatus::OffShelf;
                product.updated_at = <frame_system::Pallet<T>>::block_number();

                if was_on_sale {
                    ProductStats::<T>::mutate(|stats| {
                        stats.on_sale_products = stats.on_sale_products.saturating_sub(1);
                    });
                }

                Ok(())
            })?;

            Self::deposit_event(Event::ProductStatusChanged {
                product_id,
                status: ProductStatus::OffShelf,
            });
            Ok(())
        }

        /// 删除商品（退还押金到店铺派生账户）
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(200_000_000, 10_000))]
        pub fn delete_product(origin: OriginFor<T>, product_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let product = Products::<T>::get(product_id).ok_or(Error::<T>::ProductNotFound)?;

            // 验证店主
            let owner = T::ShopProvider::shop_owner(product.shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            // 只能删除草稿或已下架的商品
            ensure!(
                product.status == ProductStatus::Draft || product.status == ProductStatus::OffShelf,
                Error::<T>::InvalidProductStatus
            );

            // 退还押金到店铺派生账户
            let deposit_info = ProductDeposits::<T>::take(product_id)
                .ok_or(Error::<T>::DepositNotFound)?;
            let pallet_account: T::AccountId = PRODUCT_PALLET_ID.into_account_truncating();
            T::Currency::transfer(
                &pallet_account,
                &deposit_info.source_account,
                deposit_info.amount,
                ExistenceRequirement::AllowDeath,
            )?;
            let deposit_refunded = deposit_info.amount;

            // 删除商品
            Products::<T>::remove(product_id);

            // 从店铺商品列表移除
            ShopProducts::<T>::mutate(product.shop_id, |ids| {
                ids.retain(|&id| id != product_id);
            });

            // 更新统计
            ProductStats::<T>::mutate(|stats| {
                stats.total_products = stats.total_products.saturating_sub(1);
                if product.status == ProductStatus::OnSale {
                    stats.on_sale_products = stats.on_sale_products.saturating_sub(1);
                }
            });

            Self::deposit_event(Event::ProductDeleted {
                product_id,
                deposit_refunded,
            });
            Ok(())
        }
    }

    // ==================== 辅助函数 ====================

    impl<T: Config> Pallet<T> {
        /// 获取 Pallet 账户
        pub fn pallet_account() -> T::AccountId {
            PRODUCT_PALLET_ID.into_account_truncating()
        }

        /// 计算商品押金（1 USDT 等值 NEX）
        pub fn calculate_product_deposit() -> Result<BalanceOf<T>, sp_runtime::DispatchError> {
            let price = T::PricingProvider::get_nex_usdt_price();
            ensure!(price > 0, Error::<T>::PriceUnavailable);

            let min_deposit = T::MinProductDepositCos::get();
            let max_deposit = T::MaxProductDepositCos::get();

            // 价格过时时使用保守兜底值，避免基于过期数据计算押金
            if T::PricingProvider::is_price_stale() {
                return Ok(min_deposit);
            }

            let usdt_amount = T::ProductDepositUsdt::get();

            // nex_amount = usdt_amount * 10^12 / price
            let nex_amount_u128 = (usdt_amount as u128)
                .checked_mul(1_000_000_000_000u128)
                .ok_or(Error::<T>::ArithmeticOverflow)?
                .checked_div(price as u128)
                .ok_or(Error::<T>::ArithmeticOverflow)?;

            let nex_amount: BalanceOf<T> = nex_amount_u128.saturated_into();

            let final_deposit = nex_amount.max(min_deposit).min(max_deposit);

            Ok(final_deposit)
        }

        /// 获取当前押金金额（供前端查询）
        pub fn get_current_deposit() -> Result<BalanceOf<T>, sp_runtime::DispatchError> {
            Self::calculate_product_deposit()
        }
    }

    // ==================== ProductProvider 实现 ====================

    impl<T: Config> ProductProvider<T::AccountId, BalanceOf<T>> for Pallet<T> {
        fn product_exists(product_id: u64) -> bool {
            Products::<T>::contains_key(product_id)
        }

        fn is_product_on_sale(product_id: u64) -> bool {
            Products::<T>::get(product_id)
                .map(|p| p.status == ProductStatus::OnSale)
                .unwrap_or(false)
        }

        fn product_shop_id(product_id: u64) -> Option<u64> {
            Products::<T>::get(product_id).map(|p| p.shop_id)
        }

        fn product_price(product_id: u64) -> Option<BalanceOf<T>> {
            Products::<T>::get(product_id).map(|p| p.price)
        }

        fn product_stock(product_id: u64) -> Option<u32> {
            Products::<T>::get(product_id).map(|p| p.stock)
        }

        fn product_category(product_id: u64) -> Option<ProductCategory> {
            Products::<T>::get(product_id).map(|p| p.category)
        }

        fn deduct_stock(product_id: u64, quantity: u32) -> Result<(), sp_runtime::DispatchError> {
            Products::<T>::try_mutate(product_id, |maybe_product| -> Result<(), sp_runtime::DispatchError> {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;
                ensure!(product.status == ProductStatus::OnSale, Error::<T>::InvalidProductStatus);
                if product.stock > 0 {
                    ensure!(product.stock >= quantity, Error::<T>::InsufficientStock);
                    product.stock = product.stock.saturating_sub(quantity);
                    if product.stock == 0 {
                        product.status = ProductStatus::SoldOut;
                        // M4: 售罄时减少在售统计
                        ProductStats::<T>::mutate(|stats| {
                            stats.on_sale_products = stats.on_sale_products.saturating_sub(1);
                        });
                    }
                    // M2: 发出库存更新事件
                    Self::deposit_event(Event::StockUpdated {
                        product_id,
                        new_stock: product.stock,
                    });
                }
                Ok(())
            })
        }

        fn restore_stock(product_id: u64, quantity: u32) -> Result<(), sp_runtime::DispatchError> {
            Products::<T>::try_mutate(product_id, |maybe_product| -> Result<(), sp_runtime::DispatchError> {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;
                ensure!(product.status != ProductStatus::Draft, Error::<T>::InvalidProductStatus);
                if product.stock > 0 || product.status == ProductStatus::SoldOut {
                    let was_sold_out = product.status == ProductStatus::SoldOut;
                    product.stock = product.stock.saturating_add(quantity);
                    if was_sold_out {
                        product.status = ProductStatus::OnSale;
                        // M4: 恢复库存时增加在售统计
                        ProductStats::<T>::mutate(|stats| {
                            stats.on_sale_products = stats.on_sale_products.saturating_add(1);
                        });
                    }
                    // M2: 发出库存更新事件
                    Self::deposit_event(Event::StockUpdated {
                        product_id,
                        new_stock: product.stock,
                    });
                }
                Ok(())
            })
        }

        fn add_sold_count(product_id: u64, quantity: u32) -> Result<(), sp_runtime::DispatchError> {
            Products::<T>::try_mutate(product_id, |maybe_product| -> Result<(), sp_runtime::DispatchError> {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;
                product.sold_count = product.sold_count.saturating_add(quantity);
                // M2: 发出库存更新事件（sold_count 变更）
                Self::deposit_event(Event::StockUpdated {
                    product_id,
                    new_stock: product.stock,
                });
                Ok(())
            })
        }
    }
}
