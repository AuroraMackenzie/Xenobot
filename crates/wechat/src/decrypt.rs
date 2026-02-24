//! WeChat database decryption (V4 algorithm).

use crate::error::{WeChatError, WeChatResult};
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2;
use sha2::Sha512;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// V4 database decryption parameters.
#[derive(Debug, Clone)]
pub struct V4DecryptionParams {
    /// Data key (32 bytes).
    pub data_key: Vec<u8>,
    /// Image key (16 bytes).
    pub img_key: Vec<u8>,
    /// Salt extracted from database header.
    pub salt: Vec<u8>,
    /// Page size (default 4096).
    pub page_size: usize,
    /// Reserve size (default 48).
    pub reserve_size: usize,
}

impl V4DecryptionParams {
    /// Create new decryption parameters.
    pub fn new(data_key: Vec<u8>, img_key: Vec<u8>, salt: Vec<u8>) -> Self {
        Self {
            data_key,
            img_key,
            salt,
            page_size: 4096,
            reserve_size: 48,
        }
    }

    /// Derive encryption key using PBKDF2.
    pub fn derive_encryption_key(&self) -> Vec<u8> {
        let mut key = vec![0u8; 32];
        let _ = pbkdf2::<Hmac<Sha512>>(&self.data_key, &self.salt, 256_000, &mut key);
        key
    }

    /// Derive MAC key using PBKDF2.
    pub fn derive_mac_key(&self, enc_key: &[u8]) -> Vec<u8> {
        let mut mac_salt = self.salt.clone();
        for byte in mac_salt.iter_mut() {
            *byte ^= 0x3a;
        }

        let mut mac_key = vec![0u8; 32];
        let _ = pbkdf2::<Hmac<Sha512>>(enc_key, &mac_salt, 2, &mut mac_key);
        mac_key
    }
}

/// Decrypt a WeChat V4 database file.
pub fn decrypt_v4_database(
    input_path: &Path,
    output_path: &Path,
    params: &V4DecryptionParams,
) -> WeChatResult<()> {
    let mut input_file = File::open(input_path).map_err(|e| WeChatError::Io(e))?;

    let mut output_file = File::create(output_path).map_err(|e| WeChatError::Io(e))?;

    // Derive keys
    let enc_key = params.derive_encryption_key();
    let mac_key = params.derive_mac_key(&enc_key);

    let file_size = input_file.metadata().map_err(|e| WeChatError::Io(e))?.len() as usize;

    let mut page_number = 1;
    let mut position = 0;

    while position < file_size {
        let page_start = position;
        let page_end = std::cmp::min(page_start + params.page_size, file_size);
        let page_len = page_end - page_start;

        // Read page
        let mut page = vec![0u8; page_len];
        input_file
            .read_exact(&mut page)
            .map_err(|e| WeChatError::Io(e))?;

        // Process page
        let processed_page = process_v4_page(&page, page_number, &enc_key, &mac_key, params)
            .map_err(|e| WeChatError::Decryption(e.to_string()))?;

        // Write decrypted page
        output_file
            .write_all(&processed_page)
            .map_err(|e| WeChatError::Io(e))?;

        position = page_end;
        page_number += 1;
    }

    Ok(())
}

/// Process a single V4 database page.
fn process_v4_page(
    page: &[u8],
    page_number: usize,
    enc_key: &[u8],
    mac_key: &[u8],
    params: &V4DecryptionParams,
) -> Result<Vec<u8>, String> {
    if page.len() < params.reserve_size {
        return Err(format!("Page too small: {} bytes", page.len()));
    }

    let data_len = page.len() - params.reserve_size;

    // Extract IV from reserve area
    let iv_start = data_len;
    let iv_end = iv_start + 16;
    if iv_end > page.len() {
        return Err("Invalid page structure".to_string());
    }

    let iv = &page[iv_start..iv_end];

    // Verify HMAC if this is the first page
    if page_number == 1 {
        verify_v4_hmac(page, mac_key, params)
            .map_err(|e| format!("HMAC verification failed: {}", e))?;
    }

    // Decrypt data
    let cipher = Aes256CbcDec::new(enc_key.into(), iv.into());
    let mut buffer = page[..data_len].to_vec();
    let decrypted_data = cipher
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|e| format!("AES decryption failed: {}", e))?;

    // Build output page
    let mut output = decrypted_data.to_vec();

    // For first page, we need to reconstruct SQLite header
    if page_number == 1 {
        // SQLite header is 16 bytes of salt + 32 bytes of HMAC
        let mut header = Vec::with_capacity(48);
        header.extend_from_slice(&params.salt);
        header.extend_from_slice(&page[data_len..data_len + 32]); // Keep original HMAC
        output.splice(0..0, header);
    }

    Ok(output)
}

/// Verify V4 HMAC for first page.
fn verify_v4_hmac(page: &[u8], mac_key: &[u8], params: &V4DecryptionParams) -> Result<(), String> {
    let data_len = page.len() - params.reserve_size;

    // HMAC covers: page_data[16:data_len-reserve+16] || page_number as u32
    let hmac_start = 16;
    let hmac_end = data_len - params.reserve_size + 16;

    if hmac_end > data_len {
        return Err("Invalid HMAC range".to_string());
    }

    let mut mac =
        Hmac::<Sha512>::new_from_slice(mac_key).map_err(|e| format!("HMAC init failed: {}", e))?;

    mac.update(&page[hmac_start..hmac_end]);
    mac.update(&(1u32.to_be_bytes())); // Page number 1

    let computed_tag = mac.finalize().into_bytes();
    let stored_tag = &page[data_len..data_len + 32]; // First 32 bytes of reserve

    if computed_tag[..32] != stored_tag[..] {
        return Err("HMAC mismatch".to_string());
    }

    Ok(())
}

/// Extract salt from V4 database file.
pub fn extract_v4_salt(file_path: &Path) -> WeChatResult<Vec<u8>> {
    let mut file = File::open(file_path).map_err(|e| WeChatError::Io(e))?;

    let mut header = [0u8; 16];
    file.read_exact(&mut header)
        .map_err(|e| WeChatError::Io(e))?;

    Ok(header.to_vec())
}

/// Validate key against database file.
pub fn validate_v4_key(file_path: &Path, data_key: &[u8], img_key: &[u8]) -> WeChatResult<bool> {
    let salt = extract_v4_salt(file_path)?;
    let params = V4DecryptionParams::new(data_key.to_vec(), img_key.to_vec(), salt);

    // Try to decrypt first page
    let mut file = File::open(file_path).map_err(|e| WeChatError::Io(e))?;

    let page_size = params.page_size;
    let mut first_page = vec![0u8; page_size];
    file.read_exact(&mut first_page)
        .map_err(|e| WeChatError::Io(e))?;

    let enc_key = params.derive_encryption_key();
    let mac_key = params.derive_mac_key(&enc_key);

    match process_v4_page(&first_page, 1, &enc_key, &mac_key, &params) {
        Ok(_) => Ok(true),
        Err(e) => {
            tracing::debug!("Key validation failed: {}", e);
            Ok(false)
        }
    }
}
