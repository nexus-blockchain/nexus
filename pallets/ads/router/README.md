# pallet-ads-router

广告适配层路由 — 根据 PlacementId 自动分发到 Entity 或 GroupRobot 适配器。

## 概述

`pallet-ads-core` 通过四大 trait 与领域适配层交互：
- `DeliveryVerifier` — 投放收据验证
- `ClickVerifier` — 点击收据验证
- `PlacementAdminProvider` — 广告位管理员解析
- `RevenueDistributor` — 收入分配

此前 runtime 直接将 trait 绑定到 `pallet-ads-grouprobot`，导致 Entity 广告路径未接入。

本 crate 提供 `AdsRouter<T>` 结构体，同时实现四大 trait，根据 PlacementId 路由到正确的适配器。

## 路由规则

```
PlacementId 已在 pallet-ads-entity::RegisteredPlacements 注册？
  ├─ 是 → Entity 路径 (二方分成: Entity Owner + 平台)
  └─ 否 → GroupRobot 路径 (三方分成: 社区 + TEE 节点 + 平台)
```

## Runtime 配置

```rust
impl pallet_ads_core::pallet::Config for Runtime {
    // ...
    type DeliveryVerifier = pallet_ads_router::AdsRouter<Runtime>;
    type ClickVerifier = pallet_ads_router::AdsRouter<Runtime>;
    type PlacementAdmin = pallet_ads_router::AdsRouter<Runtime>;
    type RevenueDistributor = pallet_ads_router::AdsRouter<Runtime>;
}
```

> **注意**: GroupRobot 路径不支持 CPC (点击计费) 模式。`ClickVerifier` 对 GroupRobot 广告位
> 返回 `AdsRouterError::CpcNotSupportedForPath` 结构化错误。

## 设计原则

- **纯路由层**: 无 Storage、无 Extrinsic、无 Event，仅 trait 实现
- **零破坏**: Entity 和 GroupRobot 两个适配层代码不动
- **关注点分离**: Entity 管组织化广告，GroupRobot 管社区化广告
- **可扩展**: 未来新增广告路径只需增加路由分支

## 依赖

- `pallet-ads-primitives` — 共享类型和 trait 定义
- `pallet-ads-entity` — Entity 适配层 (查询 RegisteredPlacements)
- `pallet-ads-grouprobot` — GroupRobot 适配层 (默认回退)

## 测试覆盖

25 个测试用例，覆盖以下场景：

| 类别 | 测试数 | 覆盖内容 |
|------|--------|----------|
| DeliveryVerifier | 4 | Entity/GroupRobot 路由、权限拒绝、非 TEE 拒绝 |
| ClickVerifier | 3 | Entity 路由、GroupRobot 拒绝 CPC、未知广告位拒绝 |
| PlacementAdminProvider | 5 | Entity/GroupRobot admin 解析、状态查询、banned 查询 |
| RevenueDistributor | 3 | Entity 二方分成、GroupRobot 三方分成、零金额 |
| 路由边界 | 1 | 注销后路由切换验证 |
| 性能基准 | 5 | 两条路径各 trait 的 1000 次调用耗时 |

运行: `cargo test -p pallet-ads-router -- --nocapture`

## 版本历史

| 版本 | 变更 |
|------|------|
| 0.1.0 | 初始实现: Entity/GroupRobot 双路径路由 |
| 0.2.0 | 新增 ClickVerifier 路由 + 结构化错误 + 完整测试覆盖 + 性能基准 |
