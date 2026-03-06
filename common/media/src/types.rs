//! 共享类型定义模块
//!
//! 提供媒体处理相关的通用类型定义,供所有pallet使用。


extern crate alloc;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::BoundedVec;
use scale_info::TypeInfo;
use sp_core::ConstU32;

use crate::error::MediaError;

/// 共享的媒体类型枚举
///
/// 用途:
/// - General: Photo/Video/Audio(通用媒体)
/// - GroupChat: Image/Video/Audio(聊天媒体)
/// - Evidence: Image/Video/Document(证据媒体)
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug)]
pub enum MediaKind {
	/// 图片/照片
	Photo,
	/// 视频
	Video,
	/// 音频
	Audio,
	/// 文档
	Document,
}

impl MediaKind {
	/// 从MIME类型推断媒体类型
	pub fn from_mime_type(mime: &[u8]) -> Result<Self, MediaError> {
		match mime {
			b"image/jpeg" | b"image/png" | b"image/gif" | b"image/webp" | b"image/avif" =>
				Ok(Self::Photo),
			b"video/mp4" | b"video/webm" | b"video/quicktime" => Ok(Self::Video),
			b"audio/mpeg" | b"audio/wav" | b"audio/ogg" | b"audio/aac" | b"audio/flac" =>
				Ok(Self::Audio),
			b"application/pdf" | b"text/plain" => Ok(Self::Document),
			_ => Err(MediaError::UnsupportedMimeType),
		}
	}

	/// 获取推荐的文件扩展名
	pub fn recommended_extension(&self) -> &'static str {
		match self {
			Self::Photo => "jpg",
			Self::Video => "mp4",
			Self::Audio => "mp3",
			Self::Document => "pdf",
		}
	}

	/// 检查是否为视觉媒体(需要缩略图)
	pub fn is_visual(&self) -> bool {
		matches!(self, Self::Photo | Self::Video)
	}

	/// 检查是否为音频媒体
	pub fn is_audio(&self) -> bool {
		matches!(self, Self::Audio)
	}
}

/// 图片格式
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug)]
pub enum ImageFormat {
	JPEG,
	PNG,
	GIF,
	WebP,
	AVIF,
	Unknown,
}

/// 视频格式
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug)]
pub enum VideoFormat {
	MP4,
	WebM,
	MOV,
	AVI,
	Unknown,
}

/// 音频格式
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug)]
pub enum AudioFormat {
	MP3,
	AAC,
	OGG,
	WAV,
	FLAC,
	Unknown,
}

/// 文档格式
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug)]
pub enum DocumentFormat {
	PDF,
	TXT,
	MD,
	HTML,
	Unknown,
}

/// 内容类型枚举(更细粒度)
///
/// 用途: Evidence需要区分具体的内容类型
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug)]
pub enum ContentType {
	/// 图片类型
	Image(ImageFormat),
	/// 视频类型
	Video(VideoFormat),
	/// 音频类型
	Audio(AudioFormat),
	/// 文档类型
	Document(DocumentFormat),
	/// 混合类型
	Mixed,
}

impl ContentType {
	/// 从MediaKind创建ContentType
	pub fn from_kind(kind: MediaKind) -> Self {
		match kind {
			MediaKind::Photo => Self::Image(ImageFormat::Unknown),
			MediaKind::Video => Self::Video(VideoFormat::Unknown),
			MediaKind::Audio => Self::Audio(AudioFormat::Unknown),
			MediaKind::Document => Self::Document(DocumentFormat::Unknown),
		}
	}

	/// 获取对应的MediaKind
	pub fn to_kind(&self) -> MediaKind {
		match self {
			Self::Image(_) => MediaKind::Photo,
			Self::Video(_) => MediaKind::Video,
			Self::Audio(_) => MediaKind::Audio,
			Self::Document(_) => MediaKind::Document,
			Self::Mixed => MediaKind::Document, // 默认为Document
		}
	}
}

/// 通用媒体元数据结构
///
/// 用途: 从媒体文件中提取的标准化元数据
#[derive(Clone, Encode, Decode, PartialEq, Eq, TypeInfo, Debug)]
pub struct MediaMetadata {
	/// 媒体类型
	pub kind: MediaKind,
	/// 内容类型(更细粒度)
	pub content_type: ContentType,
	/// 文件大小(字节)
	pub file_size: u64,
	/// MIME类型
	pub mime_type: BoundedVec<u8, ConstU32<128>>,
	/// 内容哈希(Blake2-256)
	pub content_hash: [u8; 32],
	/// 图片/视频的宽度
	pub width: Option<u32>,
	/// 图片/视频的高度
	pub height: Option<u32>,
	/// 视频/音频的时长(秒)
	pub duration_secs: Option<u32>,
	/// 视频/音频的比特率(kbps)
	pub bitrate: Option<u32>,
	/// 帧率(fps,仅视频)
	pub fps: Option<u32>,
}

impl MediaMetadata {
	/// 创建空元数据
	pub fn new(kind: MediaKind) -> Self {
		Self {
			kind,
			content_type: ContentType::from_kind(kind),
			file_size: 0,
			mime_type: BoundedVec::default(),
			content_hash: [0u8; 32],
			width: None,
			height: None,
			duration_secs: None,
			bitrate: None,
			fps: None,
		}
	}

	/// 计算预估的缩略图大小
	pub fn estimated_thumbnail_size(&self) -> Option<(u32, u32)> {
		if !self.kind.is_visual() {
			return None;
		}

		let (w, h) = (self.width?, self.height?);
		let max_thumb_size = 320u32;

		if w <= max_thumb_size && h <= max_thumb_size {
			return Some((w, h));
		}

		let scale = (max_thumb_size as f32) / w.max(h) as f32;
		Some(((w as f32 * scale) as u32, (h as f32 * scale) as u32))
	}

	/// 检查是否需要转码
	pub fn needs_transcoding(&self) -> bool {
		match self.content_type {
			ContentType::Video(VideoFormat::AVI) => true, // AVI不支持
			ContentType::Audio(AudioFormat::WAV) if self.file_size > 10_000_000 => true, // WAV太大
			_ => false,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_media_kind_from_mime() {
		assert_eq!(MediaKind::from_mime_type(b"image/jpeg").unwrap(), MediaKind::Photo);
		assert_eq!(MediaKind::from_mime_type(b"video/mp4").unwrap(), MediaKind::Video);
		assert_eq!(MediaKind::from_mime_type(b"audio/mpeg").unwrap(), MediaKind::Audio);
		assert_eq!(MediaKind::from_mime_type(b"application/pdf").unwrap(), MediaKind::Document);
	}

	#[test]
	fn test_media_kind_is_visual() {
		assert!(MediaKind::Photo.is_visual());
		assert!(MediaKind::Video.is_visual());
		assert!(!MediaKind::Audio.is_visual());
		assert!(!MediaKind::Document.is_visual());
	}

	#[test]
	fn test_content_type_conversion() {
		let ct = ContentType::from_kind(MediaKind::Photo);
		assert_eq!(ct.to_kind(), MediaKind::Photo);
	}

	#[test]
	fn test_metadata_creation() {
		let metadata = MediaMetadata::new(MediaKind::Photo);
		assert_eq!(metadata.kind, MediaKind::Photo);
		assert_eq!(metadata.file_size, 0);
	}
}
