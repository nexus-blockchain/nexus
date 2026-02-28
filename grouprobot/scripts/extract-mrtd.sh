#!/usr/bin/env bash
# extract-mrtd.sh — 从 Gramine sigstruct 提取 MRTD/MRENCLAVE 和 MRSIGNER
#
# 用途:
#   CI/CD pipeline 中编译新版本后, 自动提取 TEE 度量值
#   输出 JSON 格式, 可直接用于链上 approve_mrtd 调用
#
# 用法:
#   ./scripts/extract-mrtd.sh [sigstruct_file]
#   ./scripts/extract-mrtd.sh                          # 默认 gramine/token-vault.sig
#   ./scripts/extract-mrtd.sh gramine/token-vault.sig  # 指定文件
#
# 输出 (JSON):
#   {
#     "mr_enclave": "0xaabb...",
#     "mr_signer": "0xccdd...",
#     "isv_prod_id": 0,
#     "isv_svn": 0,
#     "timestamp": "2026-02-28T12:00:00Z"
#   }
#
# 依赖: gramine-sgx-sigstruct-view, jq (可选)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 默认 sigstruct 文件路径
SIG_FILE="${1:-${PROJECT_ROOT}/gramine/token-vault.sig}"

# ═══════════════════════════════════════════════════════════════
# 前置检查
# ═══════════════════════════════════════════════════════════════

if [ ! -f "$SIG_FILE" ]; then
    echo "ERROR: Sigstruct file not found: $SIG_FILE" >&2
    echo "" >&2
    echo "请先编译 Gramine SGX enclave:" >&2
    echo "  cd gramine" >&2
    echo "  gramine-manifest -Dlog_level=error token-vault.manifest.template token-vault.manifest" >&2
    echo "  gramine-sgx-sign --manifest token-vault.manifest --output token-vault.manifest.sgx" >&2
    exit 1
fi

if ! command -v gramine-sgx-sigstruct-view &>/dev/null; then
    echo "ERROR: gramine-sgx-sigstruct-view not found" >&2
    echo "请安装 Gramine: https://gramine.readthedocs.io/en/latest/installation.html" >&2
    exit 1
fi

# ═══════════════════════════════════════════════════════════════
# 提取度量值
# ═══════════════════════════════════════════════════════════════

SIGSTRUCT_OUTPUT=$(gramine-sgx-sigstruct-view "$SIG_FILE" 2>&1)

extract_field() {
    local field="$1"
    echo "$SIGSTRUCT_OUTPUT" | grep -i "$field" | head -1 | awk '{print $NF}'
}

MR_ENCLAVE=$(extract_field "mr_enclave")
MR_SIGNER=$(extract_field "mr_signer")
ISV_PROD_ID=$(extract_field "isv_prod_id")
ISV_SVN=$(extract_field "isv_svn")
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

if [ -z "$MR_ENCLAVE" ]; then
    echo "ERROR: Failed to extract mr_enclave from $SIG_FILE" >&2
    echo "Raw output:" >&2
    echo "$SIGSTRUCT_OUTPUT" >&2
    exit 1
fi

# ═══════════════════════════════════════════════════════════════
# 输出
# ═══════════════════════════════════════════════════════════════

# 确保 hex 有 0x 前缀
[[ "$MR_ENCLAVE" != 0x* ]] && MR_ENCLAVE="0x${MR_ENCLAVE}"
[[ "$MR_SIGNER" != 0x* ]] && MR_SIGNER="0x${MR_SIGNER}"

if command -v jq &>/dev/null; then
    jq -n \
        --arg mr_enclave "$MR_ENCLAVE" \
        --arg mr_signer "$MR_SIGNER" \
        --arg isv_prod_id "${ISV_PROD_ID:-0}" \
        --arg isv_svn "${ISV_SVN:-0}" \
        --arg timestamp "$TIMESTAMP" \
        --arg sig_file "$SIG_FILE" \
        '{
            mr_enclave: $mr_enclave,
            mr_signer: $mr_signer,
            isv_prod_id: ($isv_prod_id | tonumber),
            isv_svn: ($isv_svn | tonumber),
            timestamp: $timestamp,
            sig_file: $sig_file
        }'
else
    cat <<EOF
{
  "mr_enclave": "${MR_ENCLAVE}",
  "mr_signer": "${MR_SIGNER}",
  "isv_prod_id": ${ISV_PROD_ID:-0},
  "isv_svn": ${ISV_SVN:-0},
  "timestamp": "${TIMESTAMP}",
  "sig_file": "${SIG_FILE}"
}
EOF
fi

# 同时写入文件 (CI artifact)
OUTPUT_FILE="${PROJECT_ROOT}/gramine/mrtd-measurements.json"
if command -v jq &>/dev/null; then
    jq -n \
        --arg mr_enclave "$MR_ENCLAVE" \
        --arg mr_signer "$MR_SIGNER" \
        --arg isv_prod_id "${ISV_PROD_ID:-0}" \
        --arg isv_svn "${ISV_SVN:-0}" \
        --arg timestamp "$TIMESTAMP" \
        '{
            mr_enclave: $mr_enclave,
            mr_signer: $mr_signer,
            isv_prod_id: ($isv_prod_id | tonumber),
            isv_svn: ($isv_svn | tonumber),
            timestamp: $timestamp
        }' > "$OUTPUT_FILE"
else
    cat > "$OUTPUT_FILE" <<EOF
{
  "mr_enclave": "${MR_ENCLAVE}",
  "mr_signer": "${MR_SIGNER}",
  "isv_prod_id": ${ISV_PROD_ID:-0},
  "isv_svn": ${ISV_SVN:-0},
  "timestamp": "${TIMESTAMP}"
}
EOF
fi

echo "" >&2
echo "Measurements saved to: $OUTPUT_FILE" >&2
echo "" >&2
echo "下一步: 在链上预批准新 MRTD:" >&2
echo "  substrate-cli tx groupRobotRegistry approve_mrtd --mrtd ${MR_ENCLAVE}" >&2
