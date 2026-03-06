# nexus-media-common

共享媒体处理工具库,为 Nexus 区块链项目提供通用的媒体验证、哈希计算、IPFS工具等功能。

## 特性

- ✅ **零运行时依赖**: 不依赖任何pallet,纯工具函数
- ✅ **no_std兼容**: 支持WASM编译
- ✅ **类型安全**: 强类型和编译时检查
- ✅ **易于测试**: 纯函数,无副作用
- ✅ **向后兼容**: 稳定的API,语义化版本管理

## 模块

### types.rs
共享类型定义:
- `MediaKind`: 媒体类型枚举(Photo/Video/Audio/Document)
- `ContentType`: 内容类型(更细粒度)
- `MediaMetadata`: 通用媒体元数据结构
- `ImageFormat`, `VideoFormat`, `AudioFormat`, `DocumentFormat`: 格式枚举

### validation.rs
内容验证工具:
- `ImageValidator`: 图片格式检测、尺寸提取、安全检查
- `VideoValidator`: 视频格式检测、元数据提取
- `AudioValidator`: 音频格式检测、元数据提取

### hash.rs
哈希工具:
- `HashHelper::content_hash()`: Blake2-256内容哈希
- `HashHelper::evidence_commitment()`: Evidence承诺哈希
- `HashHelper::verify_hash()`: 哈希验证

### ipfs.rs
IPFS辅助工具:
- `IpfsHelper::compute_cid()`: CID计算
- `IpfsHelper::validate_cid()`: CID验证

### error.rs
错误类型定义:
- `MediaError`: 统一的错误枚举

## 使用示例

```rust
use nexus_media_common::{
    MediaKind,
    ImageValidator,
    HashHelper,
    MediaError,
};

// 验证图片
let metadata = ImageValidator::validate(&image_data)?;

// 计算哈希
let content_hash = HashHelper::content_hash(&image_data);

// 判断媒体类型
if metadata.kind.is_visual() {
    // 生成缩略图
}
```

## 集成到pallet

在pallet的 `Cargo.toml` 中添加依赖:

```toml
[dependencies]
nexus-media-common = { path = "../../nexus-media-common", default-features = false }

[features]
std = [
    # ... 其他依赖
    "nexus-media-common/std",
]
```

## 开发

```bash
# 构建
cargo build

# 测试
cargo test

# 生成文档
cargo doc --open

# 检查
cargo check --all-features
```

## 版本历史

- v0.1.0 (2025-01-25): 初始版本,基础类型和错误定义
- 计划中: v0.2.0 完整验证功能, v1.0.0 稳定版本

## 许可证

MIT
