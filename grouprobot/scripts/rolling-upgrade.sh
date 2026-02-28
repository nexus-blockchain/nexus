#!/usr/bin/env bash
# rolling-upgrade.sh — 滚动升级编排脚本
#
# 对 N 个 TEE 节点执行滚动升级, 确保:
#   - 同时停止的节点数 ≤ N - K (K = Shamir 门限)
#   - 每台升级后验证健康状态
#   - 失败时自动暂停, 等待人工干预
#
# 用法:
#   ./scripts/rolling-upgrade.sh <config_file>
#
# 配置文件格式 (JSON):
#   {
#     "nodes": [
#       { "name": "node-a", "host": "10.0.1.1", "port": 3000, "ssh": "user@10.0.1.1" },
#       { "name": "node-b", "host": "10.0.1.2", "port": 3000, "ssh": "user@10.0.1.2" }
#     ],
#     "shamir_k": 2,
#     "binary_path": "/opt/grouprobot/grouprobot",
#     "new_binary": "./target/release/grouprobot",
#     "health_timeout": 60,
#     "health_interval": 5
#   }
#
# 前置条件:
#   1. 新 MRTD 已在链上预批准 (./scripts/extract-mrtd.sh)
#   2. SSH 免密登录已配置
#   3. 所有节点当前健康

set -euo pipefail

# ═══════════════════════════════════════════════════════════════
# 配置解析
# ═══════════════════════════════════════════════════════════════

CONFIG_FILE="${1:-}"
if [ -z "$CONFIG_FILE" ] || [ ! -f "$CONFIG_FILE" ]; then
    echo "Usage: $0 <config.json>"
    echo ""
    echo "Example config:"
    cat <<'EXAMPLE'
{
  "nodes": [
    { "name": "node-a", "host": "10.0.1.1", "port": 3000, "ssh": "user@10.0.1.1" },
    { "name": "node-b", "host": "10.0.1.2", "port": 3000, "ssh": "user@10.0.1.2" }
  ],
  "shamir_k": 2,
  "binary_path": "/opt/grouprobot/grouprobot",
  "new_binary": "./target/release/grouprobot",
  "health_timeout": 60,
  "health_interval": 5
}
EXAMPLE
    exit 1
fi

if ! command -v jq &>/dev/null; then
    echo "ERROR: jq is required. Install: apt install jq" >&2
    exit 1
fi

NODES_COUNT=$(jq '.nodes | length' "$CONFIG_FILE")
SHAMIR_K=$(jq -r '.shamir_k // 2' "$CONFIG_FILE")
BINARY_PATH=$(jq -r '.binary_path // "/opt/grouprobot/grouprobot"' "$CONFIG_FILE")
NEW_BINARY=$(jq -r '.new_binary // "./target/release/grouprobot"' "$CONFIG_FILE")
HEALTH_TIMEOUT=$(jq -r '.health_timeout // 60' "$CONFIG_FILE")
HEALTH_INTERVAL=$(jq -r '.health_interval // 5' "$CONFIG_FILE")

MAX_CONCURRENT=$((NODES_COUNT - SHAMIR_K))
if [ "$MAX_CONCURRENT" -le 0 ]; then
    MAX_CONCURRENT=1
fi

echo "╔══════════════════════════════════════════════════════╗"
echo "║        GroupRobot Rolling Upgrade                    ║"
echo "╠══════════════════════════════════════════════════════╣"
echo "║  Nodes:            $NODES_COUNT"
echo "║  Shamir K:         $SHAMIR_K"
echo "║  Max concurrent:   $MAX_CONCURRENT (N-K = $NODES_COUNT - $SHAMIR_K)"
echo "║  Health timeout:   ${HEALTH_TIMEOUT}s"
echo "║  New binary:       $NEW_BINARY"
echo "╚══════════════════════════════════════════════════════╝"
echo ""

if [ ! -f "$NEW_BINARY" ]; then
    echo "ERROR: New binary not found: $NEW_BINARY" >&2
    echo "请先编译: cargo build --release" >&2
    exit 1
fi

# ═══════════════════════════════════════════════════════════════
# 辅助函数
# ═══════════════════════════════════════════════════════════════

node_field() {
    local idx=$1 field=$2
    jq -r ".nodes[$idx].$field" "$CONFIG_FILE"
}

log() {
    echo "[$(date '+%H:%M:%S')] $*"
}

log_ok() {
    echo "[$(date '+%H:%M:%S')] ✅ $*"
}

log_fail() {
    echo "[$(date '+%H:%M:%S')] ❌ $*" >&2
}

# 检查节点健康
check_health() {
    local host=$1 port=$2 name=$3
    local url="http://${host}:${port}/health"
    local resp
    if resp=$(curl -sf --connect-timeout 5 --max-time 10 "$url" 2>/dev/null); then
        local status
        status=$(echo "$resp" | jq -r '.status // "unknown"')
        if [ "$status" = "ok" ]; then
            return 0
        fi
    fi
    return 1
}

# 等待节点健康
wait_healthy() {
    local host=$1 port=$2 name=$3
    local elapsed=0
    log "等待 $name 健康检查通过 (timeout: ${HEALTH_TIMEOUT}s)..."
    while [ "$elapsed" -lt "$HEALTH_TIMEOUT" ]; do
        if check_health "$host" "$port" "$name"; then
            local version
            version=$(curl -sf "http://${host}:${port}/health" | jq -r '.version // "?"')
            log_ok "$name 健康 (version: $version, ${elapsed}s)"
            return 0
        fi
        sleep "$HEALTH_INTERVAL"
        elapsed=$((elapsed + HEALTH_INTERVAL))
    done
    log_fail "$name 健康检查超时 (${HEALTH_TIMEOUT}s)"
    return 1
}

# 获取节点详细状态
get_status() {
    local host=$1 port=$2
    curl -sf --max-time 10 "http://${host}:${port}/v1/status" 2>/dev/null || echo '{"error": "unreachable"}'
}

# ═══════════════════════════════════════════════════════════════
# Phase 1: 升级前检查
# ═══════════════════════════════════════════════════════════════

log "═══ Phase 1: 升级前检查 ═══"

FAILED_NODES=()
for i in $(seq 0 $((NODES_COUNT - 1))); do
    name=$(node_field "$i" "name")
    host=$(node_field "$i" "host")
    port=$(node_field "$i" "port")

    if check_health "$host" "$port" "$name"; then
        local_status=$(get_status "$host" "$port")
        pk=$(echo "$local_status" | jq -r '.public_key // "?"' | head -c 16)
        version=$(echo "$local_status" | jq -r '.version // "?"')
        log_ok "$name — healthy (v$version, pk=${pk}...)"
    else
        log_fail "$name — UNHEALTHY"
        FAILED_NODES+=("$name")
    fi
done

if [ ${#FAILED_NODES[@]} -gt 0 ]; then
    echo ""
    log_fail "以下节点不健康: ${FAILED_NODES[*]}"
    echo "请修复后重试。不健康节点数: ${#FAILED_NODES[@]}, 最大允许: $MAX_CONCURRENT"
    if [ ${#FAILED_NODES[@]} -ge "$MAX_CONCURRENT" ]; then
        echo "ERROR: 不健康节点数 >= 最大允许并发数, 无法安全升级" >&2
        exit 1
    fi
    echo "WARNING: 继续升级 (跳过不健康节点)"
fi

echo ""
read -p "确认开始滚动升级? (y/N): " confirm
if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
    echo "已取消"
    exit 0
fi

# ═══════════════════════════════════════════════════════════════
# Phase 2: 逐台滚动升级
# ═══════════════════════════════════════════════════════════════

log "═══ Phase 2: 滚动升级 ═══"

UPGRADED=0
FAILED=0

for i in $(seq 0 $((NODES_COUNT - 1))); do
    name=$(node_field "$i" "name")
    host=$(node_field "$i" "host")
    port=$(node_field "$i" "port")
    ssh_target=$(node_field "$i" "ssh")

    echo ""
    log "────────────────────────────────────────"
    log "Round $((i + 1))/$NODES_COUNT: 升级 $name ($host)"
    log "────────────────────────────────────────"

    # 记录升级前公钥 (用于验证身份保持)
    pre_status=$(get_status "$host" "$port")
    pre_pk=$(echo "$pre_status" | jq -r '.public_key // "none"')

    # Step 1: 上传新二进制
    log "[$name] 上传新二进制..."
    if ! scp -q "$NEW_BINARY" "${ssh_target}:${BINARY_PATH}.new" 2>/dev/null; then
        log_fail "[$name] 二进制上传失败"
        FAILED=$((FAILED + 1))
        continue
    fi

    # Step 2: 停止旧版本 + 替换 + 启动新版本
    log "[$name] 停止旧版本, 替换二进制, 启动新版本..."
    ssh "$ssh_target" bash -s <<REMOTE_CMD
set -e
# 停止
systemctl stop grouprobot 2>/dev/null || pkill -f "$BINARY_PATH" 2>/dev/null || true
sleep 2
# 备份
cp "$BINARY_PATH" "${BINARY_PATH}.bak" 2>/dev/null || true
# 替换
mv "${BINARY_PATH}.new" "$BINARY_PATH"
chmod +x "$BINARY_PATH"
# 启动
systemctl start grouprobot 2>/dev/null || nohup "$BINARY_PATH" &>/dev/null &
REMOTE_CMD

    # Step 3: 等待健康
    if ! wait_healthy "$host" "$port" "$name"; then
        log_fail "[$name] 升级后健康检查失败!"
        echo ""
        echo "选项:"
        echo "  1. 回滚 (恢复旧二进制)"
        echo "  2. 跳过 (继续下一节点)"
        echo "  3. 中止 (停止升级)"
        read -p "选择 (1/2/3): " choice
        case "$choice" in
            1)
                log "[$name] 回滚..."
                ssh "$ssh_target" bash -s <<ROLLBACK
systemctl stop grouprobot 2>/dev/null || pkill -f "$BINARY_PATH" 2>/dev/null || true
sleep 2
mv "${BINARY_PATH}.bak" "$BINARY_PATH" 2>/dev/null || true
systemctl start grouprobot 2>/dev/null || nohup "$BINARY_PATH" &>/dev/null &
ROLLBACK
                wait_healthy "$host" "$port" "$name" || true
                ;;
            2)
                log "[$name] 跳过"
                ;;
            3)
                log_fail "升级中止"
                exit 1
                ;;
        esac
        FAILED=$((FAILED + 1))
        continue
    fi

    # Step 4: 验证身份保持
    post_status=$(get_status "$host" "$port")
    post_pk=$(echo "$post_status" | jq -r '.public_key // "none"')
    post_version=$(echo "$post_status" | jq -r '.version // "?"')

    if [ "$pre_pk" = "$post_pk" ] && [ "$pre_pk" != "none" ]; then
        log_ok "[$name] 身份保持 ✓ (pk=${pre_pk:0:16}..., v$post_version)"
    elif [ "$pre_pk" = "none" ]; then
        log "[$name] 升级完成 (无法验证身份: 升级前状态不可用)"
    else
        log "[$name] ⚠️ 公钥变更: ${pre_pk:0:16}... → ${post_pk:0:16}... (可能需要 re-register)"
    fi

    UPGRADED=$((UPGRADED + 1))
done

# ═══════════════════════════════════════════════════════════════
# Phase 3: 升级后验证
# ═══════════════════════════════════════════════════════════════

echo ""
log "═══ Phase 3: 升级后全局验证 ═══"

ALL_HEALTHY=true
for i in $(seq 0 $((NODES_COUNT - 1))); do
    name=$(node_field "$i" "name")
    host=$(node_field "$i" "host")
    port=$(node_field "$i" "port")

    if check_health "$host" "$port" "$name"; then
        status=$(get_status "$host" "$port")
        version=$(echo "$status" | jq -r '.version // "?"')
        pk=$(echo "$status" | jq -r '.public_key // "?"' | head -c 16)
        tee=$(echo "$status" | jq -r '.tee_mode // "?"')
        log_ok "$name — v$version, tee=$tee, pk=${pk}..."
    else
        log_fail "$name — UNHEALTHY"
        ALL_HEALTHY=false
    fi
done

# ═══════════════════════════════════════════════════════════════
# 总结
# ═══════════════════════════════════════════════════════════════

echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║                  升级总结                            ║"
echo "╠══════════════════════════════════════════════════════╣"
echo "║  总节点:    $NODES_COUNT"
echo "║  已升级:    $UPGRADED"
echo "║  失败:      $FAILED"
if $ALL_HEALTHY; then
echo "║  状态:      ✅ 全部健康"
else
echo "║  状态:      ⚠️  部分不健康"
fi
echo "╚══════════════════════════════════════════════════════╝"

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
