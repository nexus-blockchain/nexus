//! # Nexus Media Common Library
//!
//! 为 Nexus 区块链项目提供通用的媒体处理工具库。
//!
//! ## 特性
//!
//! - **零运行时依赖**: 不依赖任何pallet，纯工具函数
//! - **no_std兼容**: 支持WASM编译
//! - **类型安全**: 强类型和编译时检查
//! - **易于测试**: 纯函数，无副作用
//! - **向后兼容**: 稳定的API，语义化版本管理
//!
//! ## 快速开始
//!
//! ```ignore
//! use media_common::{
//!     MediaKind, MediaMetadata,
//!     ImageValidator, VideoValidator, AudioValidator,
//!     HashHelper, IpfsHelper,
//!     MediaError,
//! };
//!
//! // 验证图片
//! let metadata = ImageValidator::validate(&image_data)?;
//!
//! // 计算内容哈希
//! let content_hash = HashHelper::content_hash(&image_data);
//!
//! // 生成IPFS CID
//! let cid = IpfsHelper::compute_cid(&image_data)?;
//!
//! // 判断媒体类型
//! if metadata.kind.is_visual() {
//!     // 需要生成缩略图
//! }
//! ```
//!
//! ## 模块说明
//!
//! - [`types`]: 共享类型定义（MediaKind, MediaMetadata等）
//! - [`error`]: 统一错误类型（MediaError）
//! - [`validation`]: 内容验证工具（ImageValidator, VideoValidator等）
//! - [`hash`]: 哈希计算工具（Blake2-256等）
//! - [`ipfs`]: IPFS工具（CID计算和验证）
//!
//! ## 使用场景
//!
//! ### 1. General Storage - 通用媒体存储
//! ```ignore
//! use media_common::{MediaKind, ImageValidator};
//!
//! // 验证用户上传的照片
//! let metadata = ImageValidator::validate(&photo_data)?;
//! if metadata.kind == MediaKind::Photo {
//!     // 存储照片元数据
//! }
//! ```
//!
//! ### 2. Evidence Pallet - 证据媒体
//! ```ignore
//! use media_common::{HashHelper, IpfsHelper, ContentType};
//!
//! // 计算证据承诺哈希
//! let commitment = HashHelper::evidence_commitment(
//!     &namespace,
//!     subject_id,
//!     &cid,
//!     &salt,
//!     version,
//! );
//!
//! // 验证IPFS CID
//! IpfsHelper::validate_cid(&evidence_cid)?;
//! ```
//!
//! ### 3. GroupChat Pallet - 群聊媒体
//! ```ignore
//! use media_common::{VideoValidator, AudioValidator};
//!
//! // 验证聊天中的视频和音频
//! let video_meta = VideoValidator::validate(&video_data)?;
//! let audio_meta = AudioValidator::validate(&audio_data)?;
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

// 在 no_std 环境中需要 alloc
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::String;

// 导出所有公共模块
pub mod types;
pub mod error;
pub mod validation;
pub mod hash;
pub mod ipfs;

// 重新导出常用类型和结构
pub use types::{
    MediaKind, MediaMetadata, ContentType,
    ImageFormat, VideoFormat, AudioFormat, DocumentFormat,
};

pub use error::MediaError;

pub use validation::{
    ImageValidator, VideoValidator, AudioValidator,
};

pub use hash::HashHelper;

pub use ipfs::{IpfsHelper, CidInfo};

/// 库版本信息
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 支持的最大文件大小常量
pub mod limits {
    /// 图片最大大小 (50MB)
    pub const MAX_IMAGE_SIZE: usize = 50 * 1024 * 1024;

    /// 视频最大大小 (500MB)
    pub const MAX_VIDEO_SIZE: usize = 500 * 1024 * 1024;

    /// 音频最大大小 (100MB)
    pub const MAX_AUDIO_SIZE: usize = 100 * 1024 * 1024;

    /// 最大图片像素数 (1亿像素)
    pub const MAX_IMAGE_PIXELS: u64 = 100_000_000;

    /// 最大视频时长 (1小时)
    pub const MAX_VIDEO_DURATION_SECS: u32 = 3600;

    /// 最大CID长度
    pub const MAX_CID_LENGTH: usize = 128;
}

/// 便利函数集合
pub mod utils {
    use super::*;

    /// 从MIME类型判断媒体类型
    ///
    /// # 参数
    /// - `mime`: MIME类型字符串
    ///
    /// # 返回
    /// - `Ok(MediaKind)`: 识别的媒体类型
    /// - `Err(MediaError)`: 不支持的MIME类型
    ///
    /// # 示例
    /// ```ignore
    /// let kind = utils::media_kind_from_mime("image/jpeg")?;
    /// assert_eq!(kind, MediaKind::Photo);
    /// ```
    pub fn media_kind_from_mime(mime: &str) -> Result<MediaKind, MediaError> {
        MediaKind::from_mime_type(mime.as_bytes())
    }

    /// 验证任意类型的媒体文件
    ///
    /// 根据文件头自动检测格式并进行相应验证
    ///
    /// # 参数
    /// - `data`: 文件二进制数据
    ///
    /// # 返回
    /// - `Ok(MediaMetadata)`: 验证成功的元数据
    /// - `Err(MediaError)`: 验证失败
    ///
    /// # 示例
    /// ```ignore
    /// let metadata = utils::validate_media(&file_data)?;
    /// match metadata.kind {
    ///     MediaKind::Photo => println!("这是图片"),
    ///     MediaKind::Video => println!("这是视频"),
    ///     MediaKind::Audio => println!("这是音频"),
    ///     _ => {},
    /// }
    /// ```
    pub fn validate_media(data: &[u8]) -> Result<MediaMetadata, MediaError> {
        if data.len() < 4 {
            return Err(MediaError::FileTooSmall);
        }

        // 尝试按不同格式验证
        if let Ok(metadata) = ImageValidator::validate(data) {
            return Ok(metadata);
        }

        if let Ok(metadata) = VideoValidator::validate(data) {
            return Ok(metadata);
        }

        if let Ok(metadata) = AudioValidator::validate(data) {
            return Ok(metadata);
        }

        Err(MediaError::UnsupportedFormat)
    }

    /// 计算媒体内容的完整标识符
    ///
    /// 结合内容哈希和CID，提供唯一标识
    ///
    /// # 参数
    /// - `data`: 媒体文件数据
    ///
    /// # 返回
    /// - `Ok((content_hash, cid))`: 内容哈希和IPFS CID
    /// - `Err(MediaError)`: 计算失败
    pub fn compute_media_identity(data: &[u8]) -> Result<([u8; 32], String), MediaError> {
        let content_hash = HashHelper::content_hash(data);
        let cid = IpfsHelper::compute_cid(data)?;
        Ok((content_hash, cid))
    }

    /// 验证媒体内容完整性
    ///
    /// 同时验证内容哈希和IPFS CID
    ///
    /// # 参数
    /// - `data`: 媒体文件数据
    /// - `expected_hash`: 预期的内容哈希
    /// - `expected_cid`: 预期的IPFS CID
    ///
    /// # 返回
    /// - `true`: 验证通过
    /// - `false`: 验证失败
    pub fn verify_media_integrity(
        data: &[u8],
        expected_hash: &[u8; 32],
        expected_cid: &str
    ) -> bool {
        HashHelper::verify_hash(data, expected_hash) &&
        IpfsHelper::verify_content(data, expected_cid)
    }

    /// 检查文件是否为支持的媒体格式
    ///
    /// 基于文件头进行快速检测，不进行完整验证
    ///
    /// # 参数
    /// - `data`: 文件二进制数据
    ///
    /// # 返回
    /// - `true`: 支持的格式
    /// - `false`: 不支持的格式
    pub fn is_supported_media(data: &[u8]) -> bool {
        if data.len() < 4 {
            return false;
        }

        // 检查图片格式
        match &data[0..4] {
            [0xFF, 0xD8, 0xFF, _] => return true, // JPEG
            [0x89, 0x50, 0x4E, 0x47] => return true, // PNG
            [0x47, 0x49, 0x46, 0x38] => return true, // GIF
            [0x52, 0x49, 0x46, 0x46] => {
                // RIFF - 可能是WebP或WAV/AVI
                if data.len() > 12 {
                    let riff_type = &data[8..12];
                    return riff_type == b"WEBP" || riff_type == b"WAVE" || riff_type == b"AVI ";
                }
            },
            _ => {},
        }

        // 检查视频格式
        if data.len() >= 12 {
            // MP4/MOV - ftyp box
            if &data[4..8] == b"ftyp" {
                return true;
            }
            // WebM/MKV - EBML header
            if data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
                return true;
            }
        }

        // 检查音频格式
        match &data[0..4] {
            [0xFF, b, _, _] if b & 0xE0 == 0xE0 => return true, // MP3
            [0xFF, 0xF1, _, _] | [0xFF, 0xF9, _, _] => return true, // AAC
            b"OggS" => return true, // OGG
            b"fLaC" => return true, // FLAC
            _ => {},
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_media_kind_from_mime() {
        assert_eq!(
            utils::media_kind_from_mime("image/jpeg").unwrap(),
            MediaKind::Photo
        );
        assert_eq!(
            utils::media_kind_from_mime("video/mp4").unwrap(),
            MediaKind::Video
        );
        assert!(utils::media_kind_from_mime("application/unknown").is_err());
    }

    #[test]
    fn test_compute_media_identity() {
        let data = b"test media content";
        let result = utils::compute_media_identity(data);
        assert!(result.is_ok());

        let (hash, cid) = result.unwrap();
        assert_eq!(hash.len(), 32);
        assert!(!cid.is_empty());
    }

    #[test]
    fn test_verify_media_integrity() {
        let data = b"test content";
        let hash = HashHelper::content_hash(data);
        let cid = IpfsHelper::compute_cid(data).unwrap();

        assert!(utils::verify_media_integrity(data, &hash, &cid));
        assert!(!utils::verify_media_integrity(b"different content", &hash, &cid));
    }

    #[test]
    fn test_is_supported_media() {
        // JPEG header
        let jpeg_data = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        assert!(utils::is_supported_media(&jpeg_data));

        // PNG header
        let png_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert!(utils::is_supported_media(&png_data));

        // Invalid data
        let invalid_data = vec![0x00, 0x01, 0x02, 0x03];
        assert!(!utils::is_supported_media(&invalid_data));
    }
}

/// 预编译的便利宏
#[macro_export]
macro_rules! validate_and_hash {
    ($data:expr) => {
        {
            let metadata = $crate::utils::validate_media($data)?;
            let hash = $crate::HashHelper::content_hash($data);
            (metadata, hash)
        }
    };
}

/// 验证Evidence承诺的便利宏
#[macro_export]
macro_rules! verify_evidence {
    ($ns:expr, $subject:expr, $cid:expr, $salt:expr, $version:expr, $expected:expr) => {
        {
            let actual = $crate::HashHelper::evidence_commitment(
                $ns, $subject, $cid, $salt, $version
            );
            actual == $expected
        }
    };
}