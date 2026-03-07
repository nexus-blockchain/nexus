import { ProductStatus } from '@/lib/types/enums';

/**
 * 商品状态流转映射表
 * Draft → [OnSale]
 * OnSale → [SoldOut, OffShelf]
 * OffShelf → [OnSale]
 * SoldOut → [] (无合法流转)
 */
const PRODUCT_TRANSITIONS: Record<ProductStatus, ProductStatus[]> = {
  [ProductStatus.Draft]: [ProductStatus.OnSale],
  [ProductStatus.OnSale]: [ProductStatus.SoldOut, ProductStatus.OffShelf],
  [ProductStatus.OffShelf]: [ProductStatus.OnSale],
  [ProductStatus.SoldOut]: [],
};

/**
 * 获取当前商品状态的合法流转目标状态列表
 */
export function getValidProductTransitions(currentStatus: ProductStatus): ProductStatus[] {
  return PRODUCT_TRANSITIONS[currentStatus] ?? [];
}
