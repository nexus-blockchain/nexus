//! 内容验证模块
//!
//! 提供图片、视频、音频的格式检测和验证功能。


extern crate alloc;

use sp_core::blake2_256;

use crate::{
	error::MediaError,
	types::{AudioFormat, ImageFormat, MediaKind, MediaMetadata, VideoFormat},
};

/// 图片验证器
pub struct ImageValidator;

impl ImageValidator {
	/// 验证图片内容
	///
	/// # 参数
	/// - `data`: 原始图片数据
	///
	/// # 返回
	/// - `Ok(MediaMetadata)`: 验证成功,返回元数据
	/// - `Err(MediaError)`: 验证失败
	///
	/// # 示例
	/// ```ignore
	/// let metadata = ImageValidator::validate(&photo_data)?;
	/// ```
	pub fn validate(data: &[u8]) -> Result<MediaMetadata, MediaError> {
		// 1. 检查最小大小
		if data.len() < 100 {
			return Err(MediaError::FileTooSmall);
		}

		// 2. 检查最大大小(50MB)
		if data.len() > 50 * 1024 * 1024 {
			return Err(MediaError::FileTooLarge);
		}

		// 3. 检测图片格式
		let format = Self::detect_format(data)?;

		// 4. 提取元数据
		let mut metadata = Self::extract_metadata(data, format)?;

		// 5. 安全检查
		Self::security_check(data)?;

		// 6. 检查图片炸弹
		if let (Some(w), Some(h)) = (metadata.width, metadata.height) {
			Self::check_image_bomb(w, h)?;
		}

		// 7. 计算内容哈希
		metadata.content_hash = blake2_256(data);

		Ok(metadata)
	}

	/// 检测图片格式
	fn detect_format(data: &[u8]) -> Result<ImageFormat, MediaError> {
		if data.len() < 4 {
			return Err(MediaError::InvalidHeader);
		}

		// 检查文件头魔数
		match &data[0..4] {
			[0xFF, 0xD8, 0xFF, _] => Ok(ImageFormat::JPEG),
			[0x89, 0x50, 0x4E, 0x47] => Ok(ImageFormat::PNG),
			[0x47, 0x49, 0x46, 0x38] => Ok(ImageFormat::GIF),
			[0x52, 0x49, 0x46, 0x46] => {
				// RIFF header, 检查是否为WebP
				if data.len() > 12 && &data[8..12] == b"WEBP" {
					Ok(ImageFormat::WebP)
				} else {
					Err(MediaError::UnsupportedFormat)
				}
			},
			_ => Err(MediaError::UnsupportedFormat),
		}
	}

	/// 提取图片元数据
	fn extract_metadata(data: &[u8], format: ImageFormat) -> Result<MediaMetadata, MediaError> {
		use crate::types::ContentType;

		let mut metadata = MediaMetadata::new(MediaKind::Photo);
		metadata.content_type = ContentType::Image(format);
		metadata.file_size = data.len() as u64;

		// 根据格式提取宽高
		match format {
			ImageFormat::JPEG => {
				if let Ok((width, height)) = Self::extract_jpeg_dimensions(data) {
					metadata.width = Some(width);
					metadata.height = Some(height);
				}
			},
			ImageFormat::PNG => {
				if let Ok((width, height)) = Self::extract_png_dimensions(data) {
					metadata.width = Some(width);
					metadata.height = Some(height);
				}
			},
			_ => {
				// 其他格式暂不提取
			},
		}

		Ok(metadata)
	}

	/// 提取JPEG尺寸
	fn extract_jpeg_dimensions(data: &[u8]) -> Result<(u32, u32), MediaError> {
		// 简化实现: 查找SOF0标记(0xFFC0)
		for i in 0..data.len().saturating_sub(9) {
			if data[i] == 0xFF && data[i + 1] == 0xC0 {
				// SOF0 marker found
				let height = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
				let width = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
				return Ok((width, height));
			}
		}
		Err(MediaError::MetadataExtractionFailed)
	}

	/// 提取PNG尺寸
	fn extract_png_dimensions(data: &[u8]) -> Result<(u32, u32), MediaError> {
		// PNG的IHDR chunk在文件开头(8字节签名后)
		if data.len() < 24 {
			return Err(MediaError::InvalidPngHeader);
		}

		// 跳过PNG签名(8字节)和IHDR chunk长度/类型(8字节)
		let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
		let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);

		Ok((width, height))
	}

	/// 安全检查
	fn security_check(data: &[u8]) -> Result<(), MediaError> {
		// 检查是否包含可执行代码
		if data.windows(4).any(|w| w == b"\x7FELF" || w == b"MZ\x90\x00") {
			return Err(MediaError::SuspiciousContent);
		}

		Ok(())
	}

	/// 检查是否为图片炸弹
	pub fn check_image_bomb(width: u32, height: u32) -> Result<(), MediaError> {
		const MAX_PIXELS: u64 = 100_000_000; // 1亿像素

		let pixels = width as u64 * height as u64;
		if pixels > MAX_PIXELS {
			return Err(MediaError::ImageBomb);
		}

		Ok(())
	}
}

/// 视频验证器
pub struct VideoValidator;

impl VideoValidator {
	/// 验证视频内容
	pub fn validate(data: &[u8]) -> Result<MediaMetadata, MediaError> {
		// 1. 检查最小大小(100KB)
		if data.len() < 100 * 1024 {
			return Err(MediaError::FileTooSmall);
		}

		// 2. 检查最大大小(500MB)
		if data.len() > 500 * 1024 * 1024 {
			return Err(MediaError::FileTooLarge);
		}

		// 3. 检测视频格式
		let format = Self::detect_format(data)?;

		// 4. 提取元数据
		let mut metadata = Self::extract_metadata(data, format)?;

		// 5. 安全检查
		Self::security_check(data)?;

		// 6. 计算内容哈希
		metadata.content_hash = blake2_256(data);

		Ok(metadata)
	}

	/// 检测视频格式
	fn detect_format(data: &[u8]) -> Result<VideoFormat, MediaError> {
		if data.len() < 12 {
			return Err(MediaError::InvalidHeader);
		}

		// 检查ftyp box (MP4/MOV)
		if &data[4..8] == b"ftyp" {
			let brand = &data[8..12];
			match brand {
				b"isom" | b"iso2" | b"mp41" | b"mp42" => Ok(VideoFormat::MP4),
				b"qt  " => Ok(VideoFormat::MOV),
				_ => Ok(VideoFormat::Unknown),
			}
		} else if data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
			// WebM/MKV的EBML header
			Ok(VideoFormat::WebM)
		} else if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"AVI " {
			Ok(VideoFormat::AVI)
		} else {
			Err(MediaError::UnsupportedFormat)
		}
	}

	/// 提取视频元数据
	fn extract_metadata(data: &[u8], format: VideoFormat) -> Result<MediaMetadata, MediaError> {
		use crate::types::ContentType;

		let mut metadata = MediaMetadata::new(MediaKind::Video);
		metadata.content_type = ContentType::Video(format);
		metadata.file_size = data.len() as u64;

		// 实际实现应使用FFmpeg或专业库提取完整元数据
		// 这里返回基础元数据

		Ok(metadata)
	}

	/// 安全检查
	fn security_check(_data: &[u8]) -> Result<(), MediaError> {
		// 视频安全检查(简化实现)
		Ok(())
	}

	/// 检查视频时长是否合理
	pub fn check_duration(duration_secs: u32) -> Result<(), MediaError> {
		const MAX_DURATION: u32 = 3600; // 1小时

		if duration_secs > MAX_DURATION {
			return Err(MediaError::VideoTooLong);
		}

		Ok(())
	}
}

/// 音频验证器
pub struct AudioValidator;

impl AudioValidator {
	/// 验证音频内容
	pub fn validate(data: &[u8]) -> Result<MediaMetadata, MediaError> {
		// 1. 检查最小大小(10KB)
		if data.len() < 10 * 1024 {
			return Err(MediaError::FileTooSmall);
		}

		// 2. 检查最大大小(100MB)
		if data.len() > 100 * 1024 * 1024 {
			return Err(MediaError::FileTooLarge);
		}

		// 3. 检测音频格式
		let format = Self::detect_format(data)?;

		// 4. 提取元数据
		let mut metadata = Self::extract_metadata(data, format)?;

		// 5. 计算内容哈希
		metadata.content_hash = blake2_256(data);

		Ok(metadata)
	}

	/// 检测音频格式
	fn detect_format(data: &[u8]) -> Result<AudioFormat, MediaError> {
		if data.len() < 4 {
			return Err(MediaError::InvalidHeader);
		}

		match &data[0..4] {
			[0xFF, b, _, _] if b & 0xE0 == 0xE0 => Ok(AudioFormat::MP3), // MP3 sync word
			[0xFF, 0xF1, _, _] | [0xFF, 0xF9, _, _] => Ok(AudioFormat::AAC), // AAC ADTS
			b"OggS" => Ok(AudioFormat::OGG),
			b"RIFF" if data.len() > 12 && &data[8..12] == b"WAVE" => Ok(AudioFormat::WAV),
			b"fLaC" => Ok(AudioFormat::FLAC),
			_ => Err(MediaError::UnsupportedFormat),
		}
	}

	/// 提取音频元数据
	fn extract_metadata(data: &[u8], format: AudioFormat) -> Result<MediaMetadata, MediaError> {
		use crate::types::ContentType;

		let mut metadata = MediaMetadata::new(MediaKind::Audio);
		metadata.content_type = ContentType::Audio(format);
		metadata.file_size = data.len() as u64;

		// 实际实现应使用专业音频库提取元数据
		Ok(metadata)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_image_validator_too_small() {
		let data = vec![0u8; 50];
		let result = ImageValidator::validate(&data);
		assert_eq!(result, Err(MediaError::FileTooSmall));
	}

	#[test]
	fn test_image_bomb_check() {
		let result = ImageValidator::check_image_bomb(50000, 50000);
		assert_eq!(result, Err(MediaError::ImageBomb));

		let result = ImageValidator::check_image_bomb(1024, 768);
		assert!(result.is_ok());
	}

	#[test]
	fn test_video_duration_check() {
		let result = VideoValidator::check_duration(7200);
		assert_eq!(result, Err(MediaError::VideoTooLong));

		let result = VideoValidator::check_duration(300);
		assert!(result.is_ok());
	}
}
