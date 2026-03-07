import { ProductCategory, OrderStatus } from '@/lib/types/enums';

/**
 * 订单流程按商品类别分化
 *
 * Digital:  Created → Paid → Completed（即时完成）
 * Physical: Created → Paid → Shipped → Completed（含发货确认）
 * Service:  Created → Paid → ServiceStarted → ServiceCompleted → Confirmed
 * Subscription/Bundle/Other: 使用 Physical 流程作为默认
 */

const DIGITAL_TRANSITIONS: Partial<Record<OrderStatus, OrderStatus[]>> = {
  [OrderStatus.Created]: [OrderStatus.Paid],
  [OrderStatus.Paid]: [OrderStatus.Completed],
};

const PHYSICAL_TRANSITIONS: Partial<Record<OrderStatus, OrderStatus[]>> = {
  [OrderStatus.Created]: [OrderStatus.Paid],
  [OrderStatus.Paid]: [OrderStatus.Shipped],
  [OrderStatus.Shipped]: [OrderStatus.Completed],
};

const SERVICE_TRANSITIONS: Partial<Record<OrderStatus, OrderStatus[]>> = {
  [OrderStatus.Created]: [OrderStatus.Paid],
  [OrderStatus.Paid]: [OrderStatus.ServiceStarted],
  [OrderStatus.ServiceStarted]: [OrderStatus.ServiceCompleted],
  [OrderStatus.ServiceCompleted]: [OrderStatus.Confirmed],
};

function getTransitionMap(category: ProductCategory): Partial<Record<OrderStatus, OrderStatus[]>> {
  switch (category) {
    case ProductCategory.Digital:
      return DIGITAL_TRANSITIONS;
    case ProductCategory.Service:
      return SERVICE_TRANSITIONS;
    case ProductCategory.Physical:
    case ProductCategory.Subscription:
    case ProductCategory.Bundle:
    case ProductCategory.Other:
    default:
      return PHYSICAL_TRANSITIONS;
  }
}

/**
 * 获取指定商品类别和当前订单状态下的合法下一步状态列表
 */
export function getValidOrderTransitions(
  category: ProductCategory,
  currentStatus: OrderStatus,
): OrderStatus[] {
  const map = getTransitionMap(category);
  return map[currentStatus] ?? [];
}

/**
 * 订单状态对应的操作按钮标签（卖家视角）
 */
export const SELLER_ACTION_LABELS: Partial<Record<OrderStatus, string>> = {
  [OrderStatus.Shipped]: '确认发货',
  [OrderStatus.Completed]: '完成订单',
  [OrderStatus.ServiceStarted]: '开始服务',
  [OrderStatus.ServiceCompleted]: '完成服务',
};

/**
 * 订单状态对应的操作按钮标签（买家视角）
 */
export const BUYER_ACTION_LABELS: Partial<Record<OrderStatus, string>> = {
  [OrderStatus.Completed]: '确认收货',
  [OrderStatus.Confirmed]: '确认完成',
};
