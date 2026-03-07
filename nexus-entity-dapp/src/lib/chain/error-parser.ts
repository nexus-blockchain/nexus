import type { ApiPromise } from '@polkadot/api';
import type { DispatchError } from '@polkadot/types/interfaces';
import type { ChainError } from '@/lib/types';

/** Common error message translations (fallback when i18n not available) */
const ERROR_MESSAGES: Record<string, Record<string, string>> = {
  entityRegistry: {
    EntityNotFound: '实体不存在',
    NotEntityOwner: '非实体所有者',
    EntitySuspended: '实体已被暂停',
    EntityClosed: '实体已关闭',
    InsufficientFund: '资金不足',
    PermissionDenied: '权限不足',
    AlreadyAdmin: '已是管理员',
    NotAdmin: '非管理员',
    GovernanceLocked: '治理已锁定',
  },
  entityShop: {
    ShopNotFound: '店铺不存在',
    ShopClosed: '店铺已关闭',
    FundDepleted: '运营资金已耗尽',
    NotShopOwner: '非店铺所有者',
  },
  entityProduct: {
    ProductNotFound: '商品不存在',
    InvalidPrice: '价格无效',
    OutOfStock: '库存不足',
  },
  entityOrder: {
    OrderNotFound: '订单不存在',
    InvalidOrderStatus: '订单状态无效',
    PaymentFailed: '支付失败',
    OrderExpired: '订单已过期',
  },
  entityToken: {
    TokenNotConfigured: '代币未配置',
    ExceedsMaxSupply: '超出最大供应量',
    TransferRestricted: '转账受限',
    InsufficientBalance: '余额不足',
  },
  entityMarket: {
    MarketNotActive: '市场未激活',
    CircuitBreakerActive: '熔断器已触发',
    InsufficientLiquidity: '流动性不足',
    PriceDeviationExceeded: '价格偏差超限',
  },
  entityMember: {
    NotMember: '非会员',
    MemberBanned: '会员已被封禁',
    MemberFrozen: '会员已被冻结',
    RegistrationClosed: '注册已关闭',
  },
  entityGovernance: {
    ProposalNotFound: '提案不存在',
    VotingEnded: '投票已结束',
    AlreadyVoted: '已投票',
    QuorumNotReached: '未达法定人数',
  },
};

/**
 * Parse a DispatchError into a user-friendly ChainError.
 * Falls back to raw error info if no translation is available.
 */
export function parseDispatchError(api: ApiPromise, error: DispatchError): ChainError {
  if (error.isModule) {
    const decoded = api.registry.findMetaError(error.asModule);
    const moduleName = decoded.section;
    const errorName = decoded.name;

    const translatedMessage = ERROR_MESSAGES[moduleName]?.[errorName];
    const message = translatedMessage || decoded.docs.join(' ') || `${moduleName}.${errorName}`;

    return { module: moduleName, name: errorName, message };
  }

  if (error.isBadOrigin) {
    return { module: 'system', name: 'BadOrigin', message: '调用来源无效' };
  }
  if (error.isCannotLookup) {
    return { module: 'system', name: 'CannotLookup', message: '无法查找账户' };
  }
  if (error.isOther) {
    return { module: 'system', name: 'Other', message: '未知错误' };
  }

  return { module: 'unknown', name: 'Unknown', message: '交易执行失败' };
}
