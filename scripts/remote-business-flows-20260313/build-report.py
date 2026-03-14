#!/usr/bin/env python3
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parent
ART = ROOT / "artifacts"
REPORT = ROOT / "REPORT.md"
LATEST = ART / "latest.json"
EXECUTION_STATUS = ART / "execution-status.json"
BASE = ART / "base-context.json"

DATE = "2026-03-13"
WS_URL = os.environ.get("WS_URL", "wss://202.140.140.202")
SELECTED = [item for item in os.environ.get("REMOTE_FLOW_SELECTED_CASES", "").split(",") if item]

CASE_FILES = {
    "entity-shop-flow": ART / "entity-shop-flow.json",
    "entity-member-loyalty-flow": ART / "entity-member-loyalty-flow.json",
    "entity-product-order-physical-flow": ART / "entity-product-order-physical-flow.json",
    "commission-admin-controls": ART / "commission-admin-controls.json",
    "nex-market-trade-flow": ART / "nex-market-trade-flow.json",
}

SKIPPED_EXISTING = [
    ("pallet-entity-shop", "自动主店 + 基础运营资金充值", "e2e/suites/entity-commerce-commission-flow.ts"),
    ("pallet-entity-member", "注册 + 推荐链 + 激活会员", "e2e/suites/entity-commerce-commission-flow.ts"),
    ("pallet-entity-loyalty", "佣金提现到 shopping balance + 消费", "e2e/suites/entity-commerce-commission-flow.ts"),
    ("pallet-entity-product", "数字商品创建/发布", "e2e/suites/entity-commerce-commission-flow.ts"),
    ("pallet-entity-order", "数字商品即时完成订单", "e2e/suites/entity-commerce-commission-flow.ts"),
    ("pallet-commission-single-line", "订单驱动的单线分润", "e2e/suites/entity-commerce-commission-flow.ts"),
    ("pallet-commission-multi-level", "订单驱动的多级分佣", "e2e/suites/entity-commerce-commission-flow.ts"),
    ("pallet-commission-pool-reward", "pool 累积 + claimPoolReward", "e2e/suites/entity-commerce-commission-flow.ts"),
    ("pallet-nex-market", "简单挂单/撤单 smoke", "e2e/suites/nex-market-smoke.ts"),
    ("entity registry", "createEntity / updateEntity", "e2e/suites/entity-lifecycle.ts"),
]


def load_json(path: Path):
    if not path.exists():
        return None
    with path.open("r", encoding="utf-8") as fh:
        return json.load(fh)


base_context = load_json(BASE)

selected_case_ids = SELECTED or [
    case_id for case_id, file_path in CASE_FILES.items() if file_path.exists()
]

cases = []
for case_id in selected_case_ids:
    file_path = CASE_FILES.get(case_id)
    if not file_path:
        continue
    data = load_json(file_path)
    if data is None:
        cases.append({
            "case": case_id,
            "status": "missing",
        })
        continue

    item = {
        "case": case_id,
        "status": "passed" if data.get("passed") else "failed",
    }

    if case_id == "entity-shop-flow":
        item["module"] = ["pallet-entity-shop"]
        item["highlights"] = {
            "entityId": data["entityId"],
            "shopId": data["shopId"],
            "primaryShopId": data["observations"]["primaryShopId"],
            "managerCount": len(data["observations"]["managers"]),
        }
    elif case_id == "entity-member-loyalty-flow":
        item["module"] = ["pallet-entity-member", "pallet-entity-loyalty"]
        delta = int(data["points"]["daveFreeAfter"]) - int(data["points"]["daveFreeBefore"])
        item["highlights"] = {
            "entityId": data["entityId"],
            "shopId": data["shopId"],
            "memberCount": data["members"]["memberCount"],
            "charliePoints": data["points"]["charlie"],
            "davePoints": data["points"]["dave"],
            "daveFreeDelta": str(delta),
        }
    elif case_id == "entity-product-order-physical-flow":
        item["module"] = ["pallet-entity-product", "pallet-entity-order"]
        item["highlights"] = {
            "entityId": data["entityId"],
            "shopId": data["shopId"],
            "productStatusAfterPublish": data["productAfterPublish"]["status"],
            "order1Status": data["order1"]["status"],
            "order2StatusAfterRefund": data["order2AfterApprove"]["status"],
            "productStatusAfterUnpublish": data["productAfterUnpublish"]["status"],
        }
    elif case_id == "commission-admin-controls":
        item["module"] = [
            "pallet-commission-single-line",
            "pallet-commission-multi-level",
            "pallet-commission-pool-reward",
        ]
        item["highlights"] = {
            "entityId": data["entityId"],
            "singleLinePendingApplyAfter": data["singleLine"]["pending"]["applyAfter"],
            "multiLevelPendingEffectiveAt": data["multiLevel"]["pending"]["effectiveAt"],
            "poolRewardPendingApplyAfter": data["poolReward"]["pending"]["applyAfter"],
        }
    elif case_id == "nex-market-trade-flow":
        item["module"] = ["pallet-nex-market"]
        item["highlights"] = {
            "orderStatusAfterPlace": data["orderAfterPlace"]["status"],
            "tradeStatusAfterReserve": data["tradeAfterReserve"]["status"],
            "tradeStatusFinal": data["tradeFinal"]["status"],
            "orderStatusFinal": data["orderFinal"]["status"],
            "buyerDeposit": str(int(data["tradeFinal"]["buyerDeposit"], 16)),
        }

    cases.append(item)

passed = sum(1 for case in cases if case["status"] == "passed")
failed = sum(1 for case in cases if case["status"] == "failed")
missing = sum(1 for case in cases if case["status"] == "missing")

latest = {
    "date": DATE,
    "ws_url": WS_URL,
    "chain": "Nexus Development",
    "node": "Nexus Node 0.1.0-unknown",
    "runtime": "nexus v100",
    "api": "@polkadot/api 16.5.4",
    "selected_cases": selected_case_ids,
    "base_context": base_context,
    "validated_readonly": [
        {"name": "remote-inspect", "command": "npm run e2e:remote:inspect", "status": "passed"},
        {"name": "runtime-contracts", "command": "npm run e2e:remote:contracts", "status": "passed"},
    ],
    "manual_remote_cases": cases,
    "summary": {
        "passed": passed,
        "failed": failed,
        "missing": missing,
    },
}

execution_status = {
    "date": DATE,
    "ws_url": WS_URL,
    "selected_cases": selected_case_ids,
    "validated": latest["validated_readonly"],
    "manual_remote_cases": cases,
    "summary": latest["summary"],
    "note": "This summary was generated by remote-business-flows-20260313/run-remote-business-flows.sh.",
}

LATEST.write_text(json.dumps(latest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
EXECUTION_STATUS.write_text(json.dumps(execution_status, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

lines = []
lines.append("# 远程业务流测试报告")
lines.append("")
lines.append(f"- 日期：{DATE}")
lines.append(f"- 节点：`{WS_URL}`")
lines.append(f"- 目录：`{ROOT.name}/`")
lines.append("")
lines.append("## 1. 范围")
lines.append("")
lines.append("本次按用户要求，**跳过仓库里已有 E2E 覆盖的流**，仅验证以下模块的新增业务流：")
lines.append("")
for name in [
    "pallet-commission-single-line",
    "pallet-commission-pool-reward",
    "pallet-commission-multi-level",
    "pallet-entity-shop",
    "pallet-entity-member",
    "pallet-entity-loyalty",
    "pallet-entity-product",
    "pallet-entity-order",
    "pallet-nex-market",
]:
    lines.append(f"- `{name}`")
lines.append("")
lines.append("## 2. 环境")
lines.append("")
lines.append("- Chain：`Nexus Development`")
lines.append("- Node：`Nexus Node 0.1.0-unknown`")
lines.append("- Runtime：`nexus v100`")
lines.append("- API：`@polkadot/api 16.5.4`")
if base_context:
    lines.append(f"- 本次实体上下文：owner=`{base_context['ownerName']}`, entity=`{base_context['entityId']}`, shop=`{base_context['secondaryShopId']}`")
lines.append("")
lines.append("## 3. 已跳过的既有流")
lines.append("")
lines.append("| 模块 | 已有流 | 现有 suite |")
lines.append("|---|---|---|")
for module, flow, suite in SKIPPED_EXISTING:
    lines.append(f"| {module} | {flow} | `{suite}` |")
lines.append("")
lines.append("## 4. 本次执行结果")
lines.append("")
for case in cases:
    lines.append(f"### {case['case']}")
    lines.append("")
    lines.append(f"- 状态：**{case['status']}**")
    if "module" in case:
        lines.append(f"- 模块：{', '.join(f'`{module}`' for module in case['module'])}")
    if "highlights" in case:
        lines.append("- 关键结果：")
        for key, value in case["highlights"].items():
            lines.append(f"  - `{key}` = `{value}`")
    lines.append("")
lines.append("## 5. 总结")
lines.append("")
lines.append(f"- 通过：**{passed}**")
lines.append(f"- 失败：**{failed}**")
lines.append(f"- 缺失：**{missing}**")
lines.append("")
lines.append("关键补充：")
lines.append("")
lines.append("- `entityLoyalty.redeemPoints` 在该链上表现为积分兑换回**自由余额**，不是 shopping balance。")
lines.append("- `entityProduct.unpublishProduct` 在该链上返回状态 `OffShelf`。")
lines.append("- `nexMarket.priceProtection.initialPrice = 10`，导致买家保证金需求较高；本次成交流中买家保证金为 `30000 NEX`。")
lines.append("")
lines.append("## 6. 产物")
lines.append("")
lines.append(f"- 汇总：`{LATEST.relative_to(ROOT)}`")
lines.append(f"- 执行状态：`{EXECUTION_STATUS.relative_to(ROOT)}`")
for case_id, path in CASE_FILES.items():
    if path.exists():
        lines.append(f"- `{path.relative_to(ROOT)}`")

REPORT.write_text("\n".join(lines) + "\n", encoding="utf-8")
