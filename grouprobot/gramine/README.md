# Token Vault — Gramine SGX 部署指南

## 架构

```
┌─────────────────────────────────────────────┐
│  主进程 (grouprobot)                      │
│  ┌─────────────┐  ┌──────────────────────┐  │
│  │ TG Executor  │  │ Discord Executor     │  │
│  │ DC Gateway   │  │ Webhook Server       │  │
│  └──────┬───────┘  └──────────┬───────────┘  │
│         │  VaultClient (IPC)  │              │
│         └─────────┬───────────┘              │
│                   │ Unix Socket              │
├───────────────────┼─────────────────────────┤
│ ┌─────────────────▼───────────────────────┐ │
│ │  token-vault 进程 (Gramine SGX Enclave)  │ │
│ │  ┌──────────────────────────────────┐   │ │
│ │  │ TokenVault (Zeroizing<String>)   │   │ │
│ │  │ Shamir Share 恢复                 │   │ │
│ │  │ VaultServer (Unix socket IPC)    │   │ │
│ │  └──────────────────────────────────┘   │ │
│ │  SGX 内存加密 (MEE/MKTME)               │ │
│ └─────────────────────────────────────────┘ │
└─────────────────────────────────────────────┘
```

## 前置条件

1. **硬件**: Intel CPU with SGX/SGX2 support
2. **驱动**: `sgx_enclave` 内核模块已加载
3. **Gramine**: v1.7+ 已安装 (`gramine-sgx`, `gramine-manifest`, `gramine-sgx-sign`)
4. **AESM**: Intel AESM service 已运行

## 构建

```bash
cd grouprobot

# 1. 编译 release 二进制
cargo build --release
cp target/release/grouprobot gramine/

# 2. 生成 Gramine manifest
cd gramine
gramine-manifest \
  -Dlog_level=error \
  token-vault.manifest.template \
  token-vault.manifest

# 3. SGX 签名
gramine-sgx-sign \
  --manifest token-vault.manifest \
  --output token-vault.manifest.sgx

# 4. 获取 MRENCLAVE token
gramine-sgx-get-token \
  --output token-vault.token \
  --sig token-vault.sig
```

## 运行

### 模式 1: 主进程自动 spawn vault

```bash
# 设置环境变量
export VAULT_MODE=spawn
export DATA_DIR=./data

# 主进程启动时自动 spawn vault 子进程
./grouprobot
```

### 模式 2: 独立 vault 进程 (推荐生产环境)

```bash
# 终端 1: 启动 vault (SGX 模式)
cd gramine
gramine-sgx ./token-vault

# 终端 2: 启动主进程 (连接 vault)
export VAULT_MODE=connect
export VAULT_SOCKET=./data/vault.sock
./grouprobot
```

## 安全属性

| 属性 | Software 模式 | SGX 模式 |
|------|:------------:|:--------:|
| Token 内存加密 | ❌ TDX 内存 | ✅ SGX MEE |
| Token 进程隔离 | ✅ Unix socket | ✅ Unix socket |
| /proc/pid/mem 防护 | ❌ | ✅ Enclave |
| Cold boot 防护 | ❌ | ✅ 内存加密 |
| MRENCLAVE 验证 | ❌ | ✅ 远程证明 |

## 文件结构

```
gramine/
├── token-vault.manifest.template   # Gramine manifest 模板
├── grouprobot                  # 编译后的二进制 (cp from target/)
├── data/                           # 运行时数据
│   ├── vault.sock                  # IPC Unix socket
│   ├── shamir_share.sealed         # 密封的 Shamir share
│   └── enclave_ed25519.sealed      # 密封的 Ed25519 密钥
└── README.md
```
