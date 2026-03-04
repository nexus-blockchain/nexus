//! # 商城商品管理模块 (pallet-entity-product)
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

pub use pallet::*;
pub use pallet_entity_common::{ProductCategory, ProductStatus};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use sp_std::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement},
        BoundedVec, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{
        AdminPermission, PricingProvider, ProductCategory, ProductProvider, ProductStatus,
        ProductVisibility, EntityProvider, ShopProvider,
    };
    use pallet_storage_service::{IpfsPinner, SubjectType, PinTier};
    use sp_runtime::{
        traits::{AccountIdConversion, Zero, Saturating},
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
        /// 单价（NEX）
        pub price: Balance,
        /// USDT 价格（精度 10^6，0 表示未设置）
        pub usdt_price: u64,
        /// 库存数量（0 表示无限）
        pub stock: u32,
        /// 已售数量
        pub sold_count: u32,
        /// 商品状态
        pub status: ProductStatus,
        /// 商品类别
        pub category: ProductCategory,
        /// 显示排序权重（越大越靠前，默认 0）
        pub sort_weight: u32,
        /// 标签 IPFS CID（空 = 无标签，链下 JSON）
        pub tags_cid: BoundedVec<u8, MaxCidLen>,
        /// SKU 变体 IPFS CID（空 = 无 SKU，链下 JSON）
        pub sku_cid: BoundedVec<u8, MaxCidLen>,
        /// 最小购买数量（0 = 不限，默认 1）
        pub min_order_quantity: u32,
        /// 最大购买数量（0 = 不限）
        pub max_order_quantity: u32,
        /// 商品可见性
        pub visibility: ProductVisibility,
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

        /// IPFS Pin 管理接口（用于商品元数据 CID 持久化）
        type IpfsPinner: IpfsPinner<Self::AccountId, BalanceOf<Self>>;

        /// 批量操作最大数量
        #[pallet::constant]
        type MaxBatchSize: Get<u32>;

        /// 强制下架原因最大长度
        #[pallet::constant]
        type MaxReasonLength: Get<u32>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        #[cfg(feature = "try-runtime")]
        fn try_state(_n: BlockNumberFor<T>) -> Result<(), sp_runtime::TryRuntimeError> {
            Ok(())
        }

        fn integrity_test() {
            assert!(
                T::MaxProductsPerShop::get() > 0,
                "MaxProductsPerShop must be > 0"
            );
            assert!(
                T::MaxCidLength::get() > 0,
                "MaxCidLength must be > 0"
            );
            assert!(
                T::MaxBatchSize::get() > 0,
                "MaxBatchSize must be > 0"
            );
            assert!(
                T::MaxReasonLength::get() > 0,
                "MaxReasonLength must be > 0"
            );
            assert!(
                T::MinProductDepositCos::get() <= T::MaxProductDepositCos::get(),
                "MinProductDepositCos must be <= MaxProductDepositCos"
            );
            assert!(
                T::ProductDepositUsdt::get() > 0,
                "ProductDepositUsdt must be > 0"
            );
        }
    }

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
        /// 销量已更新
        SoldCountUpdated { product_id: u64, sold_count: u32 },
        /// 商品被强制下架（Root/治理）
        ProductForceUnpublished {
            product_id: u64,
            reason: Option<BoundedVec<u8, T::MaxReasonLength>>,
        },
        /// 批量操作完成（best-effort 模式，部分失败不回滚）
        BatchCompleted {
            /// 操作类型描述
            operation: BatchOperation,
            /// 成功数量
            succeeded: u32,
            /// 失败数量
            failed: u32,
            /// 失败的商品 ID 列表
            failed_ids: Vec<u64>,
        },
    }

    /// 批量操作类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
    pub enum BatchOperation {
        Publish,
        Unpublish,
        Delete,
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
        /// 在售商品不可将库存设为 0（stock=0 仅在创建时表示无限库存）
        CannotClearStockWhileOnSale,
        /// 库存溢出（restore_stock 超过 u32::MAX）
        StockOverflow,
        /// 无操作权限（非 Owner/Admin/Manager）
        NotAuthorized,
        /// 批量操作超过 MaxBatchSize
        BatchTooLarge,
        /// 强制下架原因过长
        ReasonTooLong,
        /// 实体已被全局锁定
        EntityLocked,
        /// 无效的购买数量限制（max < min）
        InvalidOrderQuantity,
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 创建商品（从店铺派生账户扣取押金）
        ///
        /// 权限：Owner / Admin(SHOP_MANAGE) / Manager
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(250_000_000, 12_000))]
        pub fn create_product(
            origin: OriginFor<T>,
            shop_id: u64,
            name_cid: Vec<u8>,
            images_cid: Vec<u8>,
            detail_cid: Vec<u8>,
            price: BalanceOf<T>,
            usdt_price: u64,
            stock: u32,
            category: ProductCategory,
            sort_weight: u32,
            tags_cid: Vec<u8>,
            sku_cid: Vec<u8>,
            min_order_quantity: u32,
            max_order_quantity: u32,
            visibility: ProductVisibility,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H4: 商品价格不能为零
            ensure!(!price.is_zero(), Error::<T>::InvalidPrice);

            // 购买数量限制校验：max > 0 时必须 >= min
            if max_order_quantity > 0 && min_order_quantity > 0 {
                ensure!(max_order_quantity >= min_order_quantity, Error::<T>::InvalidOrderQuantity);
            }

            // H2: CID 不能为空
            ensure!(!name_cid.is_empty(), Error::<T>::EmptyCid);
            ensure!(!images_cid.is_empty(), Error::<T>::EmptyCid);
            ensure!(!detail_cid.is_empty(), Error::<T>::EmptyCid);

            // 验证店铺
            ensure!(T::ShopProvider::shop_exists(shop_id), Error::<T>::ShopNotFound);
            ensure!(T::ShopProvider::is_shop_active(shop_id), Error::<T>::ShopNotActive);
            Self::ensure_product_operator(&who, shop_id, true)?;

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
            let tags_cid: BoundedVec<u8, T::MaxCidLength> =
                tags_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
            let sku_cid: BoundedVec<u8, T::MaxCidLength> =
                sku_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            // 计算押金（1 USDT 等值 NEX）
            let deposit = Self::calculate_product_deposit()?;

            // 获取店铺派生账户
            let shop_account = T::ShopProvider::shop_account(shop_id);
            let shop_balance = T::Currency::free_balance(&shop_account);
            // M2: KeepAlive 要求转账后余额 >= ED，预检查须一致
            let ed = T::Currency::minimum_balance();
            ensure!(shop_balance >= deposit.saturating_add(ed), Error::<T>::InsufficientShopFund);

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
                usdt_price,
                stock,
                sold_count: 0,
                status: ProductStatus::Draft,
                category,
                sort_weight,
                tags_cid,
                sku_cid,
                min_order_quantity,
                max_order_quantity,
                visibility,
                created_at: now,
                updated_at: now,
            };

            // 提取 CID 引用用于后续 IPFS Pin（product 将被 move）
            let pin_name_cid = product.name_cid.clone();
            let pin_images_cid = product.images_cid.clone();
            let pin_detail_cid = product.detail_cid.clone();

            Products::<T>::insert(product_id, product);
            ShopProducts::<T>::try_mutate(shop_id, |ids| ids.try_push(product_id))
                .map_err(|_| Error::<T>::MaxProductsReached)?;
            // L1-fix: checked_add 防止 u64 溢出导致 ID 覆盖
            let next_id = product_id.checked_add(1).ok_or(Error::<T>::ArithmeticOverflow)?;
            NextProductId::<T>::put(next_id);

            // 记录押金信息
            ProductDeposits::<T>::insert(product_id, ProductDepositInfo {
                shop_id,
                amount: deposit,
                source_account: shop_account,
            });

            // P0: 更新 Shop 商品计数
            let _ = T::ShopProvider::increment_product_count(shop_id);

            ProductStats::<T>::mutate(|stats| {
                stats.total_products = stats.total_products.saturating_add(1);
            });

            // IPFS Pin: 固定商品元数据 CID（best-effort，pin 失败不阻断商品创建）
            Self::pin_product_cid(&who, product_id, &pin_name_cid);
            Self::pin_product_cid(&who, product_id, &pin_images_cid);
            Self::pin_product_cid(&who, product_id, &pin_detail_cid);

            Self::deposit_event(Event::ProductCreated {
                product_id,
                shop_id,
                deposit,
            });
            Ok(())
        }

        /// 更新商品信息
        ///
        /// 权限：Owner / Admin(SHOP_MANAGE) / Manager
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn update_product(
            origin: OriginFor<T>,
            product_id: u64,
            name_cid: Option<Vec<u8>>,
            images_cid: Option<Vec<u8>>,
            detail_cid: Option<Vec<u8>>,
            price: Option<BalanceOf<T>>,
            usdt_price: Option<u64>,
            stock: Option<u32>,
            category: Option<ProductCategory>,
            sort_weight: Option<u32>,
            tags_cid: Option<Vec<u8>>,
            sku_cid: Option<Vec<u8>>,
            min_order_quantity: Option<u32>,
            max_order_quantity: Option<u32>,
            visibility: Option<ProductVisibility>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Products::<T>::try_mutate(product_id, |maybe_product| -> DispatchResult {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;

                // 权限检查：Owner / Admin / Manager
                Self::ensure_product_operator(&who, product.shop_id, true)?;

                if let Some(c) = name_cid {
                    // H2: name_cid 不能为空
                    ensure!(!c.is_empty(), Error::<T>::EmptyCid);
                    let old_cid = product.name_cid.clone();
                    product.name_cid = c.try_into().map_err(|_| Error::<T>::CidTooLong)?;
                    // IPFS: unpin old + pin new (best-effort)
                    Self::unpin_product_cid(&who, &old_cid);
                    Self::pin_product_cid(&who, product_id, &product.name_cid);
                }
                if let Some(c) = images_cid {
                    ensure!(!c.is_empty(), Error::<T>::EmptyCid);
                    let old_cid = product.images_cid.clone();
                    product.images_cid = c.try_into().map_err(|_| Error::<T>::CidTooLong)?;
                    Self::unpin_product_cid(&who, &old_cid);
                    Self::pin_product_cid(&who, product_id, &product.images_cid);
                }
                if let Some(c) = detail_cid {
                    ensure!(!c.is_empty(), Error::<T>::EmptyCid);
                    let old_cid = product.detail_cid.clone();
                    product.detail_cid = c.try_into().map_err(|_| Error::<T>::CidTooLong)?;
                    Self::unpin_product_cid(&who, &old_cid);
                    Self::pin_product_cid(&who, product_id, &product.detail_cid);
                }
                if let Some(p) = price {
                    ensure!(!p.is_zero(), Error::<T>::InvalidPrice);
                    product.price = p;
                }
                if let Some(u) = usdt_price {
                    product.usdt_price = u;
                }
                if let Some(s) = stock {
                    // M1: 在售商品不可将库存设为 0（stock=0 双重语义：创建时=无限，运行时=售罄）
                    ensure!(
                        !(s == 0 && product.status == ProductStatus::OnSale),
                        Error::<T>::CannotClearStockWhileOnSale
                    );
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
                if let Some(w) = sort_weight {
                    product.sort_weight = w;
                }
                if let Some(t) = tags_cid {
                    // tags_cid 允许为空（清除标签）
                    product.tags_cid = t.try_into().map_err(|_| Error::<T>::CidTooLong)?;
                }
                if let Some(s) = sku_cid {
                    // sku_cid 允许为空（清除 SKU）
                    product.sku_cid = s.try_into().map_err(|_| Error::<T>::CidTooLong)?;
                }
                if let Some(min_q) = min_order_quantity {
                    product.min_order_quantity = min_q;
                }
                if let Some(max_q) = max_order_quantity {
                    product.max_order_quantity = max_q;
                }
                // 购买数量限制校验（更新后的值）
                if product.max_order_quantity > 0 && product.min_order_quantity > 0 {
                    ensure!(
                        product.max_order_quantity >= product.min_order_quantity,
                        Error::<T>::InvalidOrderQuantity
                    );
                }
                if let Some(v) = visibility {
                    product.visibility = v;
                }

                product.updated_at = <frame_system::Pallet<T>>::block_number();
                Ok(())
            })?;

            Self::deposit_event(Event::ProductUpdated { product_id });
            Ok(())
        }

        /// 上架商品
        ///
        /// 权限：Owner / Admin(SHOP_MANAGE) / Manager
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(120_000_000, 6_000))]
        pub fn publish_product(origin: OriginFor<T>, product_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Products::<T>::try_mutate(product_id, |maybe_product| -> DispatchResult {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;

                Self::ensure_product_operator(&who, product.shop_id, true)?;
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
        ///
        /// 权限：Owner / Admin(SHOP_MANAGE) / Manager
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(120_000_000, 6_000))]
        pub fn unpublish_product(origin: OriginFor<T>, product_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Products::<T>::try_mutate(product_id, |maybe_product| -> DispatchResult {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;

                Self::ensure_product_operator(&who, product.shop_id, true)?;
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
        ///
        /// 权限：Owner / Admin(SHOP_MANAGE)（Manager 不可删除，涉及押金退还）
        /// 前置条件：商品状态必须为 Draft 或 OffShelf
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(200_000_000, 10_000))]
        pub fn delete_product(origin: OriginFor<T>, product_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let product = Products::<T>::get(product_id).ok_or(Error::<T>::ProductNotFound)?;

            // 权限检查：仅 Owner / Admin（Manager 不可删除）
            Self::ensure_product_operator(&who, product.shop_id, false)?;

            // 只能删除草稿或已下架的商品（OnSale/SoldOut 不可直接删除）
            ensure!(
                product.status == ProductStatus::Draft || product.status == ProductStatus::OffShelf,
                Error::<T>::InvalidProductStatus
            );

            // 退还押金到店铺派生账户（best-effort：Pallet 偿付能力不足不阻断删除）
            let deposit_refunded = if let Some(deposit_info) = ProductDeposits::<T>::take(product_id) {
                let pallet_account: T::AccountId = PRODUCT_PALLET_ID.into_account_truncating();
                match T::Currency::transfer(
                    &pallet_account,
                    &deposit_info.source_account,
                    deposit_info.amount,
                    ExistenceRequirement::AllowDeath,
                ) {
                    Ok(_) => deposit_info.amount,
                    Err(e) => {
                        log::warn!(
                            target: "entity-product",
                            "Failed to refund deposit for product {}: {:?}",
                            product_id, e
                        );
                        Zero::zero()
                    }
                }
            } else {
                log::warn!(
                    target: "entity-product",
                    "No deposit record for product {}, proceeding with deletion",
                    product_id
                );
                Zero::zero()
            };

            // IPFS Unpin: 取消固定商品元数据 CID（best-effort）
            Self::unpin_product_cid(&who, &product.name_cid);
            Self::unpin_product_cid(&who, &product.images_cid);
            Self::unpin_product_cid(&who, &product.detail_cid);

            // 删除商品
            Products::<T>::remove(product_id);

            // 从店铺商品列表移除
            ShopProducts::<T>::mutate(product.shop_id, |ids| {
                ids.retain(|&id| id != product_id);
            });

            // P0: 更新 Shop 商品计数
            let _ = T::ShopProvider::decrement_product_count(product.shop_id);

            // 更新统计（已确保状态为 Draft/OffShelf，无需检查 OnSale）
            ProductStats::<T>::mutate(|stats| {
                stats.total_products = stats.total_products.saturating_sub(1);
            });

            Self::deposit_event(Event::ProductDeleted {
                product_id,
                deposit_refunded,
            });
            Ok(())
        }

        /// 强制下架商品（Root/治理）
        ///
        /// 平台内容管控：Root 可强制下架任意在售商品
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(120_000_000, 6_000))]
        pub fn force_unpublish_product(
            origin: OriginFor<T>,
            product_id: u64,
            reason: Option<Vec<u8>>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Products::<T>::try_mutate(product_id, |maybe_product| -> DispatchResult {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;
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

            let bounded_reason = match reason {
                Some(r) => Some(
                    BoundedVec::<u8, T::MaxReasonLength>::try_from(r)
                        .map_err(|_| Error::<T>::ReasonTooLong)?
                ),
                None => None,
            };

            Self::deposit_event(Event::ProductForceUnpublished {
                product_id,
                reason: bounded_reason,
            });
            Ok(())
        }

        /// 批量上架商品（best-effort：部分失败不回滚，返回汇总事件）
        ///
        /// 权限：Owner / Admin(SHOP_MANAGE) / Manager
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(120_000_000u64.saturating_mul(product_ids.len() as u64), 6_000u64.saturating_mul(product_ids.len() as u64)))]
        pub fn batch_publish_products(
            origin: OriginFor<T>,
            product_ids: Vec<u64>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                product_ids.len() <= T::MaxBatchSize::get() as usize,
                Error::<T>::BatchTooLarge
            );

            let mut succeeded = 0u32;
            let mut failed_ids = Vec::new();

            for &pid in &product_ids {
                let result = Products::<T>::try_mutate(pid, |maybe_product| -> DispatchResult {
                    let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;
                    Self::ensure_product_operator(&who, product.shop_id, true)?;
                    ensure!(T::ShopProvider::is_shop_active(product.shop_id), Error::<T>::ShopNotActive);
                    ensure!(
                        product.status == ProductStatus::Draft || product.status == ProductStatus::OffShelf,
                        Error::<T>::InvalidProductStatus
                    );
                    product.status = ProductStatus::OnSale;
                    product.updated_at = <frame_system::Pallet<T>>::block_number();
                    Ok(())
                });

                match result {
                    Ok(()) => {
                        ProductStats::<T>::mutate(|stats| {
                            stats.on_sale_products = stats.on_sale_products.saturating_add(1);
                        });
                        Self::deposit_event(Event::ProductStatusChanged {
                            product_id: pid,
                            status: ProductStatus::OnSale,
                        });
                        succeeded += 1;
                    }
                    Err(_) => {
                        failed_ids.push(pid);
                    }
                }
            }

            Self::deposit_event(Event::BatchCompleted {
                operation: BatchOperation::Publish,
                succeeded,
                failed: failed_ids.len() as u32,
                failed_ids,
            });
            Ok(())
        }

        /// 批量下架商品（best-effort：部分失败不回滚，返回汇总事件）
        ///
        /// 权限：Owner / Admin(SHOP_MANAGE) / Manager
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(120_000_000u64.saturating_mul(product_ids.len() as u64), 6_000u64.saturating_mul(product_ids.len() as u64)))]
        pub fn batch_unpublish_products(
            origin: OriginFor<T>,
            product_ids: Vec<u64>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                product_ids.len() <= T::MaxBatchSize::get() as usize,
                Error::<T>::BatchTooLarge
            );

            let mut succeeded = 0u32;
            let mut failed_ids = Vec::new();

            for &pid in &product_ids {
                let result = Products::<T>::try_mutate(pid, |maybe_product| -> DispatchResult {
                    let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;
                    Self::ensure_product_operator(&who, product.shop_id, true)?;
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
                });

                match result {
                    Ok(()) => {
                        Self::deposit_event(Event::ProductStatusChanged {
                            product_id: pid,
                            status: ProductStatus::OffShelf,
                        });
                        succeeded += 1;
                    }
                    Err(_) => {
                        failed_ids.push(pid);
                    }
                }
            }

            Self::deposit_event(Event::BatchCompleted {
                operation: BatchOperation::Unpublish,
                succeeded,
                failed: failed_ids.len() as u32,
                failed_ids,
            });
            Ok(())
        }

        /// 批量删除商品（best-effort：部分失败不回滚，返回汇总事件）
        ///
        /// 权限：Owner / Admin(SHOP_MANAGE)（Manager 不可删除）
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(200_000_000u64.saturating_mul(product_ids.len() as u64), 10_000u64.saturating_mul(product_ids.len() as u64)))]
        pub fn batch_delete_products(
            origin: OriginFor<T>,
            product_ids: Vec<u64>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                product_ids.len() <= T::MaxBatchSize::get() as usize,
                Error::<T>::BatchTooLarge
            );

            let mut succeeded = 0u32;
            let mut failed_ids = Vec::new();

            for &pid in &product_ids {
                let product = match Products::<T>::get(pid) {
                    Some(p) => p,
                    None => { failed_ids.push(pid); continue; }
                };
                if Self::ensure_product_operator(&who, product.shop_id, false).is_err() {
                    failed_ids.push(pid);
                    continue;
                }
                if product.status != ProductStatus::Draft && product.status != ProductStatus::OffShelf {
                    failed_ids.push(pid);
                    continue;
                }

                // 退还押金（best-effort）
                let deposit_refunded = if let Some(deposit_info) = ProductDeposits::<T>::take(pid) {
                    let pallet_account: T::AccountId = PRODUCT_PALLET_ID.into_account_truncating();
                    T::Currency::transfer(
                        &pallet_account,
                        &deposit_info.source_account,
                        deposit_info.amount,
                        ExistenceRequirement::AllowDeath,
                    ).map(|_| deposit_info.amount).unwrap_or_else(|_| Zero::zero())
                } else {
                    Zero::zero()
                };

                Self::unpin_product_cid(&who, &product.name_cid);
                Self::unpin_product_cid(&who, &product.images_cid);
                Self::unpin_product_cid(&who, &product.detail_cid);

                Products::<T>::remove(pid);
                ShopProducts::<T>::mutate(product.shop_id, |ids| {
                    ids.retain(|&id| id != pid);
                });
                let _ = T::ShopProvider::decrement_product_count(product.shop_id);
                ProductStats::<T>::mutate(|stats| {
                    stats.total_products = stats.total_products.saturating_sub(1);
                });

                Self::deposit_event(Event::ProductDeleted {
                    product_id: pid,
                    deposit_refunded,
                });
                succeeded += 1;
            }

            Self::deposit_event(Event::BatchCompleted {
                operation: BatchOperation::Delete,
                succeeded,
                failed: failed_ids.len() as u32,
                failed_ids,
            });
            Ok(())
        }
    }

    // ==================== 辅助函数 ====================

    impl<T: Config> Pallet<T> {
        /// 权限检查：Owner / Admin(SHOP_MANAGE) / Manager（可选）
        ///
        /// - `allow_manager = true`：允许 Shop Manager 操作（create/update/publish/unpublish）
        /// - `allow_manager = false`：仅 Owner 或 Admin（delete 等涉及资金的操作）
        fn ensure_product_operator(
            who: &T::AccountId,
            shop_id: u64,
            allow_manager: bool,
        ) -> DispatchResult {
            // 0. EntityLocked 检查（全局冻结时拒绝所有 owner/admin/manager 操作）
            if let Some(entity_id) = T::ShopProvider::shop_entity_id(shop_id) {
                ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            }
            // 1. Owner 检查
            if let Some(owner) = T::ShopProvider::shop_owner(shop_id) {
                if owner == *who {
                    return Ok(());
                }
            }
            // 2. Entity Admin 检查
            if let Some(entity_id) = T::ShopProvider::shop_entity_id(shop_id) {
                if T::EntityProvider::is_entity_admin(entity_id, who, AdminPermission::SHOP_MANAGE) {
                    return Ok(());
                }
            }
            // 3. Shop Manager 检查（仅当 allow_manager = true）
            if allow_manager && T::ShopProvider::is_shop_manager(shop_id, who) {
                return Ok(());
            }
            Err(Error::<T>::NotAuthorized.into())
        }

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

        /// IPFS Pin 商品 CID（best-effort：失败仅记录日志，不阻断业务流程）
        fn pin_product_cid(
            caller: &T::AccountId,
            product_id: u64,
            cid: &BoundedVec<u8, T::MaxCidLength>,
        ) {
            let cid_vec: Vec<u8> = cid.clone().into_inner();
            if let Err(e) = T::IpfsPinner::pin_cid_for_subject(
                caller.clone(),
                SubjectType::Product,
                product_id,
                cid_vec,
                Some(PinTier::Standard),
            ) {
                log::warn!(
                    target: "entity-product",
                    "Failed to pin CID for product {}: {:?}",
                    product_id, e
                );
            }
        }

        /// IPFS Unpin 商品 CID（best-effort：失败仅记录日志）
        fn unpin_product_cid(
            caller: &T::AccountId,
            cid: &BoundedVec<u8, T::MaxCidLength>,
        ) {
            let cid_vec: Vec<u8> = cid.clone().into_inner();
            if let Err(e) = T::IpfsPinner::unpin_cid(
                caller.clone(),
                cid_vec,
            ) {
                log::warn!(
                    target: "entity-product",
                    "Failed to unpin CID: {:?}",
                    e
                );
            }
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
                // H1: 包含 OffShelf — 售罄后下架的商品（stock=0, OffShelf）也需恢复库存
                if product.stock > 0 || product.status == ProductStatus::SoldOut || product.status == ProductStatus::OffShelf {
                    let was_sold_out = product.status == ProductStatus::SoldOut;
                    // M3: checked_add 防止 u32 溢出静默截断
                    product.stock = product.stock.checked_add(quantity)
                        .ok_or(Error::<T>::StockOverflow)?;
                    // H2: SoldOut→OnSale 仅在 Shop 激活时自动恢复上架
                    if was_sold_out && T::ShopProvider::is_shop_active(product.shop_id) {
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
                // H2: 发出销量更新事件（非库存事件）
                Self::deposit_event(Event::SoldCountUpdated {
                    product_id,
                    sold_count: product.sold_count,
                });
                Ok(())
            })
        }

        // ==================== 扩展查询实现 ====================

        fn product_status(product_id: u64) -> Option<ProductStatus> {
            Products::<T>::get(product_id).map(|p| p.status)
        }

        fn product_usdt_price(product_id: u64) -> Option<u64> {
            Products::<T>::get(product_id).map(|p| p.usdt_price)
        }

        fn product_owner(product_id: u64) -> Option<T::AccountId> {
            Products::<T>::get(product_id)
                .and_then(|p| T::ShopProvider::shop_owner(p.shop_id))
        }

        fn shop_product_ids(shop_id: u64) -> sp_std::vec::Vec<u64> {
            ShopProducts::<T>::get(shop_id).into_inner()
        }

        fn product_visibility(product_id: u64) -> Option<ProductVisibility> {
            Products::<T>::get(product_id).map(|p| p.visibility)
        }

        fn product_min_order_quantity(product_id: u64) -> Option<u32> {
            Products::<T>::get(product_id).map(|p| p.min_order_quantity)
        }

        fn product_max_order_quantity(product_id: u64) -> Option<u32> {
            Products::<T>::get(product_id).map(|p| p.max_order_quantity)
        }

        // ==================== 治理调用实现 ====================

        fn update_price(product_id: u64, new_price: BalanceOf<T>) -> Result<(), sp_runtime::DispatchError> {
            Products::<T>::try_mutate(product_id, |maybe_product| -> Result<(), sp_runtime::DispatchError> {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;
                ensure!(!new_price.is_zero(), Error::<T>::InvalidPrice);
                product.price = new_price;
                product.updated_at = <frame_system::Pallet<T>>::block_number();
                Self::deposit_event(Event::ProductUpdated { product_id });
                Ok(())
            })
        }

        fn delist_product(product_id: u64) -> Result<(), sp_runtime::DispatchError> {
            Products::<T>::try_mutate(product_id, |maybe_product| -> Result<(), sp_runtime::DispatchError> {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;
                if product.status == ProductStatus::OnSale {
                    product.status = ProductStatus::OffShelf;
                    product.updated_at = <frame_system::Pallet<T>>::block_number();
                    ProductStats::<T>::mutate(|stats| {
                        stats.on_sale_products = stats.on_sale_products.saturating_sub(1);
                    });
                }
                Ok(())
            })
        }

        fn set_inventory(product_id: u64, new_inventory: u32) -> Result<(), sp_runtime::DispatchError> {
            Products::<T>::try_mutate(product_id, |maybe_product| -> Result<(), sp_runtime::DispatchError> {
                let product = maybe_product.as_mut().ok_or(Error::<T>::ProductNotFound)?;
                let old_stock = product.stock;
                product.stock = new_inventory;
                product.updated_at = <frame_system::Pallet<T>>::block_number();

                // 库存变化可能影响商品状态
                if new_inventory > 0 && product.status == ProductStatus::SoldOut {
                    if T::ShopProvider::is_shop_active(product.shop_id) {
                        product.status = ProductStatus::OnSale;
                        ProductStats::<T>::mutate(|stats| {
                            stats.on_sale_products = stats.on_sale_products.saturating_add(1);
                        });
                    }
                } else if new_inventory == 0 && old_stock > 0 && product.status == ProductStatus::OnSale {
                    product.status = ProductStatus::SoldOut;
                    ProductStats::<T>::mutate(|stats| {
                        stats.on_sale_products = stats.on_sale_products.saturating_sub(1);
                    });
                }

                Self::deposit_event(Event::StockUpdated {
                    product_id,
                    new_stock: new_inventory,
                });
                Ok(())
            })
        }
    }
}
