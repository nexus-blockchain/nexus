//! 错误类型定义模块
//!
//! 提供媒体处理相关的统一错误类型。


/// 媒体工具库错误类型
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaError {
	// === 通用错误 ===
	/// 文件太小
	FileTooSmall,
	/// 文件太大
	FileTooLarge,
	/// 不支持的MIME类型
	UnsupportedMimeType,
	/// 不支持的格式
	UnsupportedFormat,
	/// 无效的文件头
	InvalidHeader,

	// === CID相关错误 ===
	/// CID太长
	CidTooLong,
	/// 无效的CID长度
	InvalidCidLength,
	/// 无效的CIDv0格式
	InvalidCidV0,
	/// 无效的CIDv1格式
	InvalidCidV1,
	/// 无效的CID前缀
	InvalidCidPrefix,
	/// 无效的CID编码
	InvalidCidEncoding,
	/// 无效的CID
	InvalidCid,

	// === 图片相关错误 ===
	/// 无效的PNG头
	InvalidPngHeader,
	/// 元数据提取失败
	MetadataExtractionFailed,
	/// 可疑内容(可能包含恶意代码)
	SuspiciousContent,
	/// 图片炸弹(尺寸过大)
	ImageBomb,

	// === 视频相关错误 ===
	/// 视频太长
	VideoTooLong,

	// === 功能未实现 ===
	/// 缩略图生成未实现
	ThumbnailGenerationNotImplemented,
}

impl MediaError {
	/// 获取错误描述
	pub fn message(&self) -> &'static str {
		match self {
			Self::FileTooSmall => "File too small",
			Self::FileTooLarge => "File too large",
			Self::UnsupportedMimeType => "Unsupported MIME type",
			Self::UnsupportedFormat => "Unsupported format",
			Self::InvalidHeader => "Invalid file header",
			Self::CidTooLong => "CID too long",
			Self::InvalidCidLength => "Invalid CID length",
			Self::InvalidCidV0 => "Invalid CIDv0 format",
			Self::InvalidCidV1 => "Invalid CIDv1 format",
			Self::InvalidCidPrefix => "Invalid CID prefix",
			Self::InvalidCidEncoding => "Invalid CID encoding",
			Self::InvalidCid => "Invalid CID",
			Self::InvalidPngHeader => "Invalid PNG header",
			Self::MetadataExtractionFailed => "Metadata extraction failed",
			Self::SuspiciousContent => "Suspicious content detected",
			Self::ImageBomb => "Image bomb detected",
			Self::VideoTooLong => "Video too long",
			Self::ThumbnailGenerationNotImplemented => "Thumbnail generation not implemented",
		}
	}
}

#[cfg(feature = "std")]
impl std::fmt::Display for MediaError {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{}", self.message())
	}
}

#[cfg(feature = "std")]
impl std::error::Error for MediaError {}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_error_message() {
		let err = MediaError::FileTooLarge;
		assert_eq!(err.message(), "File too large");
	}

	#[test]
	fn test_error_equality() {
		assert_eq!(MediaError::FileTooSmall, MediaError::FileTooSmall);
		assert_ne!(MediaError::FileTooSmall, MediaError::FileTooLarge);
	}
}
