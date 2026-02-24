//! Multimedia helpers for WeChat data processing.
//!
//! This module focuses on `.dat` image decryption used by several WeChat exports.

use crate::error::{WeChatError, WeChatResult};
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use std::fs;
use std::path::Path;

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type Aes192CbcDec = cbc::Decryptor<aes::Aes192>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// Supported image formats after decryption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// JPEG image.
    Jpeg,
    /// PNG image.
    Png,
    /// GIF image.
    Gif,
    /// WEBP image.
    Webp,
    /// BMP image.
    Bmp,
}

impl ImageFormat {
    /// Suggested extension for the detected format.
    pub fn extension(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Png => "png",
            ImageFormat::Gif => "gif",
            ImageFormat::Webp => "webp",
            ImageFormat::Bmp => "bmp",
        }
    }
}

/// Parameters for `.dat` image decryption.
#[derive(Debug, Clone)]
pub struct DatImageDecryptParams {
    /// Optional XOR key. If empty and `auto_detect_xor` is true, key will be inferred.
    pub xor_key: Option<Vec<u8>>,
    /// Optional AES key for a second decryption pass.
    pub aes_key: Option<Vec<u8>>,
    /// Optional AES IV (must be 16 bytes when provided).
    pub aes_iv: Option<Vec<u8>>,
    /// Whether to auto-detect XOR key when not provided.
    pub auto_detect_xor: bool,
}

impl Default for DatImageDecryptParams {
    fn default() -> Self {
        Self {
            xor_key: None,
            aes_key: None,
            aes_iv: None,
            auto_detect_xor: true,
        }
    }
}

/// Result metadata for `.dat` image decryption.
#[derive(Debug, Clone)]
pub struct DatImageDecryptResult {
    /// Detected final image format.
    pub format: ImageFormat,
    /// XOR key used (if any).
    pub xor_key_used: Option<Vec<u8>>,
    /// Decrypted byte length.
    pub bytes_written: usize,
}

/// Decrypt a `.dat` image file and write output image bytes.
pub fn decrypt_dat_image_file(
    input_path: &Path,
    output_path: &Path,
    params: &DatImageDecryptParams,
) -> WeChatResult<DatImageDecryptResult> {
    let encrypted = fs::read(input_path).map_err(WeChatError::Io)?;
    let (decrypted, meta) = decrypt_dat_image_bytes(&encrypted, params)?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(WeChatError::Io)?;
    }
    fs::write(output_path, &decrypted).map_err(WeChatError::Io)?;

    Ok(DatImageDecryptResult {
        format: meta.0,
        xor_key_used: meta.1,
        bytes_written: decrypted.len(),
    })
}

/// Decrypt `.dat` image bytes in memory.
pub fn decrypt_dat_image_bytes(
    encrypted: &[u8],
    params: &DatImageDecryptParams,
) -> WeChatResult<(Vec<u8>, (ImageFormat, Option<Vec<u8>>))> {
    if encrypted.is_empty() {
        return Err(WeChatError::Decryption(
            "empty .dat payload cannot be decrypted".to_string(),
        ));
    }

    let mut payload = encrypted.to_vec();
    let xor_key_used = resolve_xor_key(&payload, params)?;
    if let Some(xor_key) = xor_key_used.as_ref() {
        apply_xor_in_place(&mut payload, xor_key);
    }

    if let Some(aes_key) = params.aes_key.as_ref() {
        payload = decrypt_aes_cbc_pkcs7(&payload, aes_key, params.aes_iv.as_deref())?;
    }

    let format = detect_image_format(&payload).ok_or_else(|| {
        WeChatError::Decryption(
            "decrypted payload does not match known image signatures".to_string(),
        )
    })?;

    Ok((payload, (format, xor_key_used)))
}

/// Infer a single-byte XOR key by matching common image signatures.
pub fn infer_wechat_dat_xor_key(encrypted: &[u8]) -> Option<Vec<u8>> {
    if encrypted.is_empty() {
        return None;
    }

    let signatures: &[&[u8]] = &[
        &[0xFF, 0xD8, 0xFF],                               // JPEG
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A], // PNG
        b"GIF89a",
        b"GIF87a",
        b"BM",
    ];

    for signature in signatures {
        if encrypted.len() < signature.len() {
            continue;
        }
        let candidate = encrypted[0] ^ signature[0];
        if signature
            .iter()
            .enumerate()
            .all(|(idx, byte)| encrypted[idx] ^ candidate == *byte)
        {
            return Some(vec![candidate]);
        }
    }

    None
}

fn resolve_xor_key(
    payload: &[u8],
    params: &DatImageDecryptParams,
) -> WeChatResult<Option<Vec<u8>>> {
    if let Some(key) = params.xor_key.as_ref() {
        if key.is_empty() {
            return Err(WeChatError::Decryption(
                "xor_key cannot be empty".to_string(),
            ));
        }
        return Ok(Some(key.clone()));
    }

    if params.auto_detect_xor {
        return Ok(infer_wechat_dat_xor_key(payload));
    }

    Ok(None)
}

fn apply_xor_in_place(buffer: &mut [u8], key: &[u8]) {
    if key.is_empty() {
        return;
    }
    for (idx, byte) in buffer.iter_mut().enumerate() {
        *byte ^= key[idx % key.len()];
    }
}

fn decrypt_aes_cbc_pkcs7(data: &[u8], key: &[u8], iv: Option<&[u8]>) -> WeChatResult<Vec<u8>> {
    if data.is_empty() {
        return Err(WeChatError::Decryption(
            "cannot AES-decrypt empty payload".to_string(),
        ));
    }

    let iv_buf = match iv {
        Some(v) if v.len() == 16 => v.to_vec(),
        Some(v) => {
            return Err(WeChatError::Decryption(format!(
                "invalid AES IV length {}, expected 16",
                v.len()
            )))
        }
        None => vec![0u8; 16],
    };

    let mut buffer = data.to_vec();
    let decrypted = match key.len() {
        16 => Aes128CbcDec::new(key.into(), iv_buf.as_slice().into())
            .decrypt_padded_mut::<Pkcs7>(&mut buffer)
            .map_err(|e| WeChatError::Decryption(format!("AES-128 decrypt failed: {}", e)))?
            .to_vec(),
        24 => Aes192CbcDec::new(key.into(), iv_buf.as_slice().into())
            .decrypt_padded_mut::<Pkcs7>(&mut buffer)
            .map_err(|e| WeChatError::Decryption(format!("AES-192 decrypt failed: {}", e)))?
            .to_vec(),
        32 => Aes256CbcDec::new(key.into(), iv_buf.as_slice().into())
            .decrypt_padded_mut::<Pkcs7>(&mut buffer)
            .map_err(|e| WeChatError::Decryption(format!("AES-256 decrypt failed: {}", e)))?
            .to_vec(),
        other => {
            return Err(WeChatError::Decryption(format!(
                "unsupported AES key length {} (expected 16/24/32)",
                other
            )))
        }
    };

    Ok(decrypted)
}

fn detect_image_format(data: &[u8]) -> Option<ImageFormat> {
    if data.len() >= 3 && data[..3] == [0xFF, 0xD8, 0xFF] {
        return Some(ImageFormat::Jpeg);
    }
    if data.len() >= 8 && data[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Some(ImageFormat::Png);
    }
    if data.len() >= 6 && (&data[..6] == b"GIF89a" || &data[..6] == b"GIF87a") {
        return Some(ImageFormat::Gif);
    }
    if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return Some(ImageFormat::Webp);
    }
    if data.len() >= 2 && &data[..2] == b"BM" {
        return Some(ImageFormat::Bmp);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_wechat_dat_xor_key_jpeg() {
        let plain = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00];
        let key = 0xAA_u8;
        let encrypted: Vec<u8> = plain.iter().map(|b| *b ^ key).collect();
        let inferred = infer_wechat_dat_xor_key(&encrypted);
        assert_eq!(inferred, Some(vec![key]));
    }

    #[test]
    fn test_decrypt_dat_image_bytes_xor_only() {
        let plain = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
        let key = vec![0x10];
        let encrypted: Vec<u8> = plain.iter().map(|b| *b ^ key[0]).collect();
        let params = DatImageDecryptParams {
            xor_key: Some(key.clone()),
            aes_key: None,
            aes_iv: None,
            auto_detect_xor: false,
        };

        let (decrypted, (format, used_key)) =
            decrypt_dat_image_bytes(&encrypted, &params).expect("decrypt should succeed");
        assert_eq!(decrypted, plain);
        assert_eq!(format, ImageFormat::Png);
        assert_eq!(used_key, Some(key));
    }

    #[test]
    fn test_detect_image_format_webp() {
        let mut sample = b"RIFF".to_vec();
        sample.extend_from_slice(&[0, 0, 0, 0]);
        sample.extend_from_slice(b"WEBP");
        assert_eq!(detect_image_format(&sample), Some(ImageFormat::Webp));
    }
}
