# TEE 安全架构

> 版本: 0.1.0 | 最后更新: 2026-02-24

## 1. 概述

GroupRobot 的安全架构基于 **TEE (Trusted Execution Environment)** 硬件保护，核心目标:

1. **Bot Token 不可泄露** — Token 始终在加密内存中，不落盘明文
2. **签名密钥不可伪造** — Ed25519 密钥仅在 Enclave 内可用
3. **操作不可否认** — 每条管理动作签名 + 序列号上链存证
4. **代码不可篡改** — MRTD/MRENCLAVE 度量值链上注册

```
安全层次:
┌────────────────────────────────────────────────────┐
│ Layer 5: 链上验证                                    │
│   DCAP Quote 链上解析 + ECDSA 4 级签名验证            │
├────────────────────────────────────────────────────┤
│ Layer 4: 远程证明 (DCAP)                             │
│   TDX Quote v4 + nonce 防重放 + PCK 证书链           │
├────────────────────────────────────────────────────┤
│ Layer 3: 密钥分发 (Shamir + ECDH)                    │
│   K-of-N 秘密共享 + X25519 加密传输 + RA-TLS         │
├────────────────────────────────────────────────────┤
│ Layer 2: Token 隔离 (Vault)                          │
│   独立进程 / IPC 加密通道 / Zeroizing 内存            │
├────────────────────────────────────────────────────┤
│ Layer 1: 进程加固                                    │
│   jemalloc zero-on-free + core dump 禁用 + mlock     │
└────────────────────────────────────────────────────┘
```

## 2. TEE 双模式 (EnclaveBridge)

`tee/enclave_bridge.rs` 定义了 TEE 的核心抽象层:

```rust
pub enum TeeMode {
    Hardware,  // TDX/SGX 真实硬件
    Software,  // 软件模拟 (开发/测试)
}
```

### 2.1 自动检测

启动时根据 `TEE_MODE` 环境变量或硬件设备自动选择:

| 条件 | 结果 |
|------|------|
| `TEE_MODE=hardware` / `tdx` / `sgx` | Hardware |
| `TEE_MODE=software` | Software |
| 未设置且 `/dev/attestation/quote` 存在 | Hardware |
| 未设置且设备不存在 | Software (警告) |

### 2.2 EnclaveBridge 接口

| 方法 | 说明 |
|------|------|
| `init(tee_mode, data_dir)` | 初始化 Enclave + 密封存储 + 加载/生成密钥 |
| `mode()` | 获取当前 TEE 模式 |
| `public_key()` / `public_key_bytes()` / `public_key_hex()` | Ed25519 公钥 |
| `sign(message)` | Ed25519 签名 (64 bytes) |
| `verify(message, signature)` | 验证签名 |
| `seal(name, data)` | AES-256-GCM 密封存储 |
| `unseal(name)` | 解封数据 |
| `seal_key()` | 获取密封密钥 (Shamir share 加解密用) |

### 2.3 Ed25519 密钥管理

启动时自动加载或生成:

```
init()
  ├─ 尝试 unseal("enclave_ed25519.sealed")
  │   ├─ 成功 (32 bytes) → SigningKey::from_bytes
  │   └─ 失败 ↓
  └─ OsRng 生成 32 bytes seed → seal("enclave_ed25519.sealed")
```

密钥一旦生成即密封存储，后续启动自动加载，保证同一节点公钥不变。

## 3. 密封存储 (SealedStorage)

`tee/sealed_storage.rs` 实现 AES-256-GCM 加密的本地文件存储。

### 3.1 密钥派生

| 模式 | 派生方式 | 绑定因子 | 安全性 |
|------|---------|---------|--------|
| **Hardware** | SGX MRENCLAVE seal key | CPU 微码 + 代码度量 (MRENCLAVE) | ✅ 硬件级 |
| Hardware 回退 | TDX Quote MRTD 字段 | CPU + TD 度量 (MRTD, 48 bytes) | ✅ 硬件级 |
| **Software** | SHA256(HOSTNAME + machine-id + domain) | 主机名 + machine-id | ⚠️ 仅开发 |

**Hardware 密钥路径**:
1. 优先: `/dev/attestation/keys/_sgx_mrenclave` (Gramine SGX seal key)
2. 回退: `/dev/attestation/quote` 偏移 184..232 (MRTD 48 bytes)

**域分隔符**: `"grouprobot-sealed-storage-hw-v1:"` / `"grouprobot-sealed-storage-mrtd-v1:"`

### 3.2 加密格式

```
[12 bytes nonce][N bytes AES-256-GCM ciphertext + 16 bytes tag]
```

- **Nonce**: 随机 12 bytes (每次 seal 新生成)
- **算法**: AES-256-GCM (AEAD, 带认证加密)

### 3.3 密封文件清单

| 文件 | 内容 | 大小 |
|------|------|------|
| `enclave_ed25519.sealed` | Ed25519 签名密钥 seed | 32 bytes + overhead |
| `shamir_share.sealed` | Shamir secret share | 变长 |
| `ipc_key.sealed` | Vault IPC 加密密钥 | 32 bytes + overhead |
| `sequence.dat` | 序列号计数器 | 8 bytes (明文) |

## 4. TokenVault — Token 安全容器

`tee/token_vault.rs` 确保 Bot Token 永不以原始 String 形式暴露。

### 4.1 安全属性

- Token 使用 `Zeroizing<String>` 包装，drop 时内存自动清零
- **不实现** `Debug` / `Display`，防止日志泄露
- `build_*` 方法返回 `Zeroizing<String>`，调用方用完即清零
- **不提供** `get_token()` 方法，Token 永远不以原始形式暴露
- Token 内存页通过 `mlock()` 锁定，防止 swap 到磁盘

### 4.2 API

| 方法 | 返回 | 说明 |
|------|------|------|
| `set_telegram_token(token)` | — | 注入 TG Token (+ mlock) |
| `set_discord_token(token)` | — | 注入 DC Token (+ mlock) |
| `build_tg_api_url(method)` | `Zeroizing<String>` | `https://api.telegram.org/bot<TOKEN>/<method>` |
| `build_dc_auth_header()` | `Zeroizing<String>` | `Bot <TOKEN>` |
| `build_dc_gateway_identify()` | `Zeroizing<String>` | Gateway IDENTIFY JSON payload |

### 4.3 Vault 三种运行模式

在 `main.rs` 中根据 `VAULT_MODE` 选择:

| 模式 | 说明 | 适用场景 |
|------|------|---------|
| `inprocess` | Token 直接存在当前进程内存 | 单进程部署 (默认) |
| `spawn` | 启动独立 Vault 进程, IPC 通信 | 高安全需求 |
| `connect` | 连接已运行的 Vault 进程 | 多 Bot 共享 Vault |

### 4.4 VaultProvider Trait

```rust
pub trait VaultProvider: Send + Sync {
    fn build_tg_api_url(&self, method: &str) -> BotResult<Zeroizing<String>>;
    fn build_dc_auth_header(&self) -> BotResult<Zeroizing<String>>;
}
```

`TelegramExecutor` 和 `DiscordExecutor` 通过此 trait 获取认证信息，无需直接持有 Token。

## 5. Vault IPC 加密通道

`tee/vault_ipc.rs` + `tee/vault_server.rs` + `tee/vault_client.rs`

当 Vault 以独立进程运行时 (spawn/connect 模式):

```
GroupRobot 进程                    Vault 进程
     │                                │
     │  ──── AES-256-GCM 加密帧 ───►  │
     │  ◄─── AES-256-GCM 加密帧 ────  │
     │                                │
   VaultClient                   VaultServer
   (Unix Socket)                 (Unix Socket)
```

### IPC 协议

```
[4 bytes length][12 bytes nonce][ciphertext + 16 bytes tag]
```

- **传输**: Unix Domain Socket (`/tmp/grouprobot-vault.sock`)
- **加密**: AES-256-GCM，IPC 密钥从密封存储加载 (`ipc_key.sealed`)
- **消息类型**: Request (method + params) → Response (result / error)

## 6. Shamir 秘密共享

`tee/shamir.rs` 实现 K-of-N 门限秘密共享，用于 Token 的安全分发和恢复。

### 6.1 核心函数

| 函数 | 说明 |
|------|------|
| `split_secret(secret, k, n)` | 将 secret 分成 N 个 share, 任意 K 个可恢复 |
| `recover_secret(shares)` | 从 K 个 share 恢复 secret (Lagrange 插值) |
| `encode_secrets(signing_key, tg_token, dc_token)` | 将多个 secret 编码为单个 byte 序列 |
| `decode_secrets(data)` | 解码为 (signing_key, tg_token, dc_token) |

### 6.2 X25519 ECDH 加密传输

Peer 间传递 share 时使用 ECDH 加密:

| 函数 | 说明 |
|------|------|
| `ed25519_to_x25519_secret(ed_sk)` | Ed25519 SK → X25519 StaticSecret (SHA-512 + clamp) |
| `ed25519_pk_to_x25519(ed_pk)` | Ed25519 PK → X25519 PublicKey (Edwards → Montgomery) |
| `encrypt_share_for_recipient(share, recipient_pk, sender_sk)` | 临时 ECDH + AES-256-GCM |
| `decrypt_share_from_sender(encrypted, sender_pk, recipient_sk)` | 解密 |

**加密流程**:
```
发送方:
  1. 生成 ephemeral X25519 keypair
  2. ECDH: shared_secret = ephemeral_sk × recipient_pk
  3. KDF: key = SHA256("grouprobot-share-encrypt-v1" || shared_secret)
  4. AES-256-GCM(key, nonce, share) → ciphertext

接收方:
  1. ECDH: shared_secret = recipient_sk × ephemeral_pk
  2. KDF: key = SHA256("grouprobot-share-encrypt-v1" || shared_secret)
  3. AES-256-GCM-Open(key, nonce, ciphertext) → share
```

### 6.3 EcdhEncryptedShare 格式

```
[32 bytes ephemeral_pk][12 bytes nonce][N bytes ciphertext + 16 bytes tag]
```

## 7. Share Recovery — Token 恢复

`tee/share_recovery.rs` 定义启动时的 Token 恢复流程:

```
recover_token()
  │
  ├─ 尝试 1: 本地密封 share
  │   ├─ unseal("shamir_share.sealed") → share
  │   ├─ K=1: 直接 recover_secret([share])
  │   └─ K>1: 本地 share + 从 peer 收集 K-1 个 share
  │           ├─ 向每个 peer POST /share/request
  │           ├─ peer 验证 AttestationGuard (链上查询)
  │           ├─ peer 返回 ECDH 加密的 share
  │           └─ 收集够 K 个 → recover_secret(shares)
  │
  ├─ 尝试 2: 环境变量 Fallback (过渡模式)
  │   ├─ BOT_TOKEN → set_telegram_token()
  │   ├─ DISCORD_TOKEN → set_discord_token()
  │   └─ ⚠️ 打印安全警告, 自动密封为 share 供下次使用
  │
  └─ 失败: 返回 BotError
```

**RecoverySource 枚举**:

| 值 | 说明 |
|----|------|
| `LocalShare` | 从本地密封 share 恢复 (K=1, 最常见) |
| `PeerShares { collected, threshold }` | 从本地 + peer 收集 share 恢复 (K>1) |
| `EnvironmentVariable` | 环境变量 fallback (不安全, 仅过渡) |

## 8. Ceremony — Share 分发仪式

`tee/ceremony.rs` 实现 Shamir share 的安全分发:

### 8.1 端点

| 路径 | 方法 | 说明 |
|------|------|------|
| `/share/request` | POST | 请求 share (需 AttestationGuard 验证) |
| `/share/distribute` | POST | 主动分发 share 给 peer |
| `/ceremony/status` | GET | 当前仪式状态 |

### 8.2 AttestationGuard 验证

收到 share 请求时:
1. 提取请求方 Ed25519 公钥
2. `SHA256(pk)` → `bot_id_hash`
3. 查询链上 `Attestations` storage
4. 验证 `(is_tee=true, quote_verified=true)`
5. 通过 → 返回 ECDH 加密的 share; 失败 → 403

### 8.3 SharedChainClient

```rust
pub type SharedChainClient = Arc<RwLock<Option<Arc<ChainClient>>>>;
```

Ceremony 路由在 chain 连接完成前就注册, 通过 `SharedChainClient` 延迟注入链客户端。

## 9. 远程证明 (DCAP)

### 9.1 Attestor

`tee/attestor.rs` 生成 TDX + SGX 双证明:

**report_data 格式** (64 bytes):
```
[0..32]  SHA256(Ed25519_public_key)    ← 绑定身份
[32..64] chain_nonce 或 0x00...00      ← 防重放 (硬件模式)
```

**AttestationBundle 字段**:

| 字段 | 类型 | 说明 |
|------|------|------|
| `tdx_quote_hash` | [u8; 32] | TDX Quote SHA-256 |
| `sgx_quote_hash` | [u8; 32] | SGX Quote SHA-256 |
| `mrtd` | [u8; 48] | TDX 度量值 |
| `mrenclave` | [u8; 32] | SGX 度量值 |
| `is_simulated` | bool | 是否软件模拟 |
| `tdx_quote_raw` | Option\<Vec\<u8\>\> | 原始 TDX Quote (硬件) |
| `nonce` | Option\<[u8; 32]\> | 链上 nonce |
| `pck_cert_der` | Option\<Vec\<u8\>\> | PCK 证书 DER (Level 4) |
| `intermediate_cert_der` | Option\<Vec\<u8\>\> | 中间 CA DER (Level 4) |

### 9.2 DCAP 本地验证

`tee/dcap_verify.rs` 实现 ECDSA P-256 四级签名链验证:

```
Level 4: Intel Root CA (硬编码公钥)
    │ 签名 ↓
Level 3: Intermediate CA (PCE 签名)
    │ 签名 ↓
Level 2: PCK Certificate (平台密钥)
    │ 签名 ↓
Level 1: QE Report + AK (Attestation Key)
    │ 签名 ↓
Level 0: TD Quote Body (report_data, MRTD, ...)
```

### 9.3 证明刷新

| 模式 | 刷新周期 | 流程 |
|------|---------|------|
| Software | 24h | `generate_attestation()` → `refresh_attestation()` (链上交易) |
| Hardware | 24h | `request_nonce` → 等 7s → `query_nonce` → `generate_with_nonce` → `submit_dcap_full` |

**Hardware 防重放**: 每次刷新先请求链上 nonce, 嵌入 report_data[32..64], 链上验证 nonce 匹配后标记已使用。

## 10. 进程内存加固

`tee/mem_security.rs` 在 `main()` 最开始执行:

### 10.1 加固措施

| # | 措施 | 系统调用 | 目的 |
|---|------|---------|------|
| 1 | 禁用 core dump | `setrlimit(RLIMIT_CORE, 0)` | 防止崩溃时 Token dump 到磁盘 |
| 2 | 清除 dumpable | `prctl(PR_SET_DUMPABLE, 0)` | 禁止 `/proc/pid/mem` 读取 + ptrace |
| 3 | 提高 mlock 限制 | `setrlimit(RLIMIT_MEMLOCK, hard)` | 允许更多内存页锁定 |

### 10.2 mlock 函数

```rust
pub fn mlock_bytes(data: &[u8]) -> bool
```

对指定内存区域调用 `mlock()`，锁定到物理内存，防止被 swap 到磁盘。用于:
- Telegram Bot Token
- Discord Bot Token
- IPC 密钥

### 10.3 jemalloc zero-on-free

`main.rs` 中配置全局 allocator:

```rust
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
```

配合 jemalloc 的 `junk:true` 或 `zero:true` 选项，堆内存释放后自动填充零值，防止 Token 残留在已释放的堆内存中。

### 10.4 环境变量清除

```rust
// main.rs 中，Token 注入 Vault 后立即清除
std::env::remove_var("BOT_TOKEN");
std::env::remove_var("DISCORD_TOKEN");
std::env::remove_var("CHAIN_SIGNER_SEED");
```

防止 `/proc/pid/environ` 泄露敏感信息。

## 11. RA-TLS Token 注入

`tee/ra_tls.rs` 提供基于 RA-TLS 的 Token 安全注入端点:

```
外部管理工具 ──── RA-TLS (TLS + Quote) ────► GroupRobot /provision
                                              │
                                              ├─ 验证 Quote
                                              ├─ 提取 Token
                                              ├─ 注入 TokenVault
                                              └─ 密封为 Shamir share
```

适用于首次部署时, 由管理员安全地将 Token 注入 TEE 节点。

## 12. Peer 通信

`tee/peer_client.rs` 实现 TEE 节点间的安全通信:

| 方法 | 说明 |
|------|------|
| `request_share(peer_url, my_pk, ceremony_hash)` | 向 peer 请求 Shamir share |
| `verify_peer_attestation(peer_url)` | 验证 peer 的 TEE 证明 |

通信安全保障:
- **传输层**: HTTPS (TLS 1.3)
- **身份验证**: Ed25519 签名 + 链上 AttestationGuard
- **数据加密**: X25519 ECDH + AES-256-GCM (share 内容)

## 13. 安全模型总结

### 威胁与对策

| 威胁 | 对策 |
|------|------|
| 内存 dump (core dump / ptrace) | RLIMIT_CORE=0 + PR_SET_DUMPABLE=0 |
| 内存残留 (use-after-free) | Zeroizing + jemalloc zero-on-free |
| 内存 swap | mlock() 锁定 Token 页 |
| 磁盘泄露 | AES-256-GCM 密封, 不存储明文 Token |
| 日志泄露 | TokenVault 不实现 Debug/Display |
| 环境变量泄露 | 使用后立即 remove_var |
| 代码篡改 | MRTD/MRENCLAVE 链上注册 + DCAP 验证 |
| 重放攻击 | 链上 nonce 防重放 (硬件模式) |
| 中间人 | ECDH 加密 share + 链上 AttestationGuard |
| 密钥丢失 | Shamir K-of-N 门限恢复 |

### Hardware vs Software 安全差异

| 维度 | Hardware | Software |
|------|----------|----------|
| 密封密钥来源 | CPU 硬件 (MRENCLAVE/MRTD) | HOSTNAME + machine-id |
| 证明可信度 | DCAP ECDSA 4 级验证 | 模拟 (无硬件保障) |
| 内存隔离 | TDX/SGX Enclave | 仅进程级隔离 |
| 适用场景 | 生产环境 | 开发/测试 |
