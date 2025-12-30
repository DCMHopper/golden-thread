use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::FileExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::path::Path;
use std::sync::OnceLock;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use hkdf::Hkdf;
use keyring::Error as KeyringError;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::error::CoreError;

const KEYCHAIN_SERVICE: &str = "com.goldenthread.app";
const KEYCHAIN_ACCOUNT: &str = "archive-master-key";
const MASTER_KEY_ENV: &str = "GT_MASTER_KEY_HEX";

const MAGIC: [u8; 4] = *b"GTAT";
const VERSION: u8 = 1;
const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;
const MAX_CHUNK_SIZE: usize = 8 * 1024 * 1024;
const TAG_LEN: usize = 16;
const HEADER_LEN: u64 = 21;

pub struct MasterKey(Zeroizing<[u8; 32]>);

impl MasterKey {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Purpose-specific derived key for domain separation.
pub struct DerivedKey(Zeroizing<[u8; 32]>);

impl DerivedKey {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Key purposes for HKDF domain separation.
/// Each purpose produces a cryptographically independent key from the master key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPurpose {
    /// Key for SQLCipher database encryption
    Database,
    /// Key for attachment file encryption
    Attachments,
}

impl KeyPurpose {
    fn info(&self) -> &'static [u8] {
        match self {
            KeyPurpose::Database => b"golden-thread-db-v1",
            KeyPurpose::Attachments => b"golden-thread-attachments-v1",
        }
    }
}

/// Derives a purpose-specific key from the master key using HKDF-SHA256.
///
/// This provides cryptographic domain separation so that compromising one
/// derived key (e.g., through a side-channel attack on file encryption)
/// does not directly compromise other uses of the master key.
pub fn derive_key(master: &MasterKey, purpose: KeyPurpose) -> Result<DerivedKey, CoreError> {
    let hk = Hkdf::<Sha256>::new(None, master.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(purpose.info(), &mut okm)
        .map_err(|_| CoreError::Crypto("HKDF expand failed".to_string()))?;
    Ok(DerivedKey(Zeroizing::new(okm)))
}

static MASTER_KEY_CACHE: OnceLock<[u8; 32]> = OnceLock::new();

/// Test helper: derive a deterministic master key from a passphrase and install it
/// for this process. Intended for tests only.
pub fn set_test_key_from_passphrase(passphrase: &str) {
    let digest = Sha256::digest(passphrase.as_bytes());
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&digest);
    let _ = MASTER_KEY_CACHE.set(bytes);
    std::env::set_var(MASTER_KEY_ENV, hex::encode(bytes));
}

pub fn load_or_create_master_key() -> Result<MasterKey, CoreError> {
    if let Some(bytes) = MASTER_KEY_CACHE.get() {
        return Ok(MasterKey(Zeroizing::new(*bytes)));
    }
    // Environment variable override is only available in debug/test builds.
    // In release builds, environment variables are visible via `ps eww` which would
    // expose the master key to other processes on the system.
    #[cfg(any(debug_assertions, test))]
    if let Ok(hex) = std::env::var(MASTER_KEY_ENV) {
        let bytes = parse_hex_key(&hex)?;
        let _ = MASTER_KEY_CACHE.set(bytes);
        return Ok(MasterKey(Zeroizing::new(bytes)));
    }

    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| CoreError::Crypto(format!("keychain init failed: {e}")))?;

    match entry.get_password() {
        Ok(secret) => {
            let bytes = parse_hex_key(&secret)?;
            let _ = MASTER_KEY_CACHE.set(bytes);
            Ok(MasterKey(Zeroizing::new(bytes)))
        }
        Err(KeyringError::NoEntry) => {
            let mut key = [0u8; 32];
            OsRng.fill_bytes(&mut key);
            let hex = hex::encode(key);
            entry
                .set_password(&hex)
                .map_err(|e| CoreError::Crypto(format!("keychain store failed: {e}")))?;
            let _ = MASTER_KEY_CACHE.set(key);
            Ok(MasterKey(Zeroizing::new(key)))
        }
        Err(err) => Err(CoreError::Crypto(format!("keychain read failed: {err}"))),
    }
}

/// Applies the SQLCipher encryption key to a database connection.
///
/// NOTE: This currently uses the master key directly for backward compatibility
/// with existing databases. A future version should migrate to using
/// `derive_key(key, KeyPurpose::Database)` with a proper re-keying migration.
pub fn apply_sqlcipher_key(conn: &rusqlite::Connection, key: &MasterKey) -> Result<(), CoreError> {
    let hex_key = hex::encode(key.as_bytes());
    let pragma = format!("PRAGMA key = \"x'{hex_key}'\";");
    conn.execute_batch(&pragma)?;
    Ok(())
}

/// Applies a derived SQLCipher key to a database connection.
/// Use this for new databases that should use key derivation from the start.
pub fn apply_sqlcipher_key_derived(conn: &rusqlite::Connection, key: &MasterKey) -> Result<(), CoreError> {
    let db_key = derive_key(key, KeyPurpose::Database)?;
    let hex_key = hex::encode(db_key.as_bytes());
    let pragma = format!("PRAGMA key = \"x'{hex_key}'\";");
    conn.execute_batch(&pragma)?;
    Ok(())
}

/// Returns a derived key for encrypting attachments.
/// Use this instead of the raw master key to provide domain separation.
pub fn attachment_key(master: &MasterKey) -> Result<DerivedKey, CoreError> {
    derive_key(master, KeyPurpose::Attachments)
}

pub fn encrypt_stream_with_hash<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    key: &MasterKey,
) -> Result<(String, u64), CoreError> {
    let mut hasher = Sha256::new();
    let total = encrypt_stream_internal(reader, writer, key, DEFAULT_CHUNK_SIZE, Some(&mut hasher))?;
    Ok((hex::encode(hasher.finalize()), total))
}

pub fn encrypt_stream_with_hash_chunk<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    key: &MasterKey,
    chunk_size: usize,
) -> Result<(String, u64), CoreError> {
    let mut hasher = Sha256::new();
    let total = encrypt_stream_internal(reader, writer, key, chunk_size, Some(&mut hasher))?;
    Ok((hex::encode(hasher.finalize()), total))
}

pub fn encrypt_stream<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    key: &MasterKey,
) -> Result<u64, CoreError> {
    encrypt_stream_internal(reader, writer, key, DEFAULT_CHUNK_SIZE, None)
}

pub fn encrypt_stream_chunk<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    key: &MasterKey,
    chunk_size: usize,
) -> Result<u64, CoreError> {
    encrypt_stream_internal(reader, writer, key, chunk_size, None)
}

fn encrypt_stream_internal<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    key: &MasterKey,
    chunk_size: usize,
    mut hasher: Option<&mut Sha256>,
) -> Result<u64, CoreError> {
    if chunk_size == 0 || chunk_size > MAX_CHUNK_SIZE {
        return Err(CoreError::Crypto("invalid chunk size".to_string()));
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_bytes()));
    let mut base_nonce = [0u8; 12];
    OsRng.fill_bytes(&mut base_nonce);
    write_header(writer, &base_nonce, chunk_size)?;

    let mut buf = vec![0u8; chunk_size];
    let mut counter: u64 = 0;
    let mut total: u64 = 0;

    loop {
        let n = reader.read(&mut buf).map_err(|e| CoreError::Crypto(e.to_string()))?;
        if n == 0 {
            break;
        }
        if let Some(ref mut h) = hasher {
            h.update(&buf[..n]);
        }
        let nonce = nonce_for_chunk(&base_nonce, counter);
        let ct = cipher
            .encrypt(Nonce::from_slice(&nonce), &buf[..n])
            .map_err(|e| CoreError::Crypto(format!("encrypt failed: {e}")))?;
        writer
            .write_all(&ct)
            .map_err(|e| CoreError::Crypto(e.to_string()))?;
        total = total.saturating_add(n as u64);
        counter = counter.saturating_add(1);
    }
    Ok(total)
}

pub fn decrypt_stream<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    key: &MasterKey,
) -> Result<u64, CoreError> {
    let (chunk_size, base_nonce) = read_header(reader)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_bytes()));
    let mut counter: u64 = 0;
    let mut total: u64 = 0;

    let ct_chunk_size = chunk_size
        .checked_add(TAG_LEN)
        .ok_or_else(|| CoreError::Crypto("chunk size overflow".to_string()))?;
    let mut buf = vec![0u8; ct_chunk_size];

    loop {
        let mut read = 0;
        while read < buf.len() {
            let n = reader
                .read(&mut buf[read..])
                .map_err(|e| CoreError::Crypto(e.to_string()))?;
            if n == 0 {
                break;
            }
            read += n;
        }
        if read == 0 {
            break;
        }
        let nonce = nonce_for_chunk(&base_nonce, counter);
        let pt = cipher
            .decrypt(Nonce::from_slice(&nonce), &buf[..read])
            .map_err(|e| CoreError::Crypto(format!("decrypt failed: {e}")))?;
        writer
            .write_all(&pt)
            .map_err(|e| CoreError::Crypto(e.to_string()))?;
        total = total.saturating_add(pt.len() as u64);
        counter = counter.saturating_add(1);
        if read < buf.len() {
            break;
        }
    }

    Ok(total)
}

pub fn encrypt_file_to_path(src: &Path, dest: &Path, key: &MasterKey) -> Result<u64, CoreError> {
    let mut reader = File::open(src).map_err(|e| CoreError::Crypto(e.to_string()))?;
    let mut writer = File::create(dest).map_err(|e| CoreError::Crypto(e.to_string()))?;
    encrypt_stream(&mut reader, &mut writer, key)
}

pub fn decrypt_file_to_path(src: &Path, dest: &Path, key: &MasterKey) -> Result<u64, CoreError> {
    let mut reader = File::open(src).map_err(|e| CoreError::Crypto(e.to_string()))?;
    let mut writer = File::create(dest).map_err(|e| CoreError::Crypto(e.to_string()))?;
    decrypt_stream(&mut reader, &mut writer, key)
}

fn parse_hex_key(hex: &str) -> Result<[u8; 32], CoreError> {
    let bytes = hex::decode(hex).map_err(|e| CoreError::Crypto(format!("invalid key: {e}")))?;
    if bytes.len() != 32 {
        return Err(CoreError::Crypto("invalid key length".to_string()));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

fn write_header<W: Write>(writer: &mut W, base_nonce: &[u8; 12], chunk_size: usize) -> Result<(), CoreError> {
    writer
        .write_all(&MAGIC)
        .map_err(|e| CoreError::Crypto(e.to_string()))?;
    writer
        .write_all(&[VERSION])
        .map_err(|e| CoreError::Crypto(e.to_string()))?;
    let chunk_size = (chunk_size as u32).to_le_bytes();
    writer
        .write_all(&chunk_size)
        .map_err(|e| CoreError::Crypto(e.to_string()))?;
    writer
        .write_all(base_nonce)
        .map_err(|e| CoreError::Crypto(e.to_string()))?;
    Ok(())
}

fn read_header<R: Read>(reader: &mut R) -> Result<(usize, [u8; 12]), CoreError> {
    let mut magic = [0u8; 4];
    reader
        .read_exact(&mut magic)
        .map_err(|e| CoreError::Crypto(e.to_string()))?;
    if magic != MAGIC {
        return Err(CoreError::Crypto("invalid attachment header".to_string()));
    }
    let mut version = [0u8; 1];
    reader
        .read_exact(&mut version)
        .map_err(|e| CoreError::Crypto(e.to_string()))?;
    if version[0] != VERSION {
        return Err(CoreError::Crypto("unsupported attachment version".to_string()));
    }
    let mut chunk = [0u8; 4];
    reader
        .read_exact(&mut chunk)
        .map_err(|e| CoreError::Crypto(e.to_string()))?;
    let chunk_size = u32::from_le_bytes(chunk) as usize;
    if chunk_size == 0 || chunk_size > MAX_CHUNK_SIZE {
        return Err(CoreError::Crypto("invalid chunk size".to_string()));
    }
    let mut nonce = [0u8; 12];
    reader
        .read_exact(&mut nonce)
        .map_err(|e| CoreError::Crypto(e.to_string()))?;
    Ok((chunk_size, nonce))
}

pub fn encrypted_plaintext_len(path: &Path) -> Result<u64, CoreError> {
    let mut file = File::open(path).map_err(|e| CoreError::Crypto(e.to_string()))?;
    let meta = file.metadata().map_err(|e| CoreError::Crypto(e.to_string()))?;
    let total_len = meta.len();
    if total_len < HEADER_LEN + TAG_LEN as u64 {
        return Err(CoreError::Crypto("encrypted file too small".to_string()));
    }
    let (chunk_size, _nonce) = read_header(&mut file)?;
    let ct_chunk_size = (chunk_size as u64)
        .checked_add(TAG_LEN as u64)
        .ok_or_else(|| CoreError::Crypto("chunk size overflow".to_string()))?;
    let payload_len = total_len
        .checked_sub(HEADER_LEN)
        .ok_or_else(|| CoreError::Crypto("invalid encrypted length".to_string()))?;
    if payload_len == 0 {
        return Ok(0);
    }
    let full_chunks = payload_len / ct_chunk_size;
    let remainder = payload_len % ct_chunk_size;
    if remainder > 0 && remainder < TAG_LEN as u64 {
        return Err(CoreError::Crypto("invalid encrypted length".to_string()));
    }
    let last_plain = if remainder == 0 { 0 } else { remainder - TAG_LEN as u64 };
    Ok(full_chunks * chunk_size as u64 + last_plain)
}

pub fn decrypt_file_parallel(
    input: &Path,
    output: &Path,
    key: &MasterKey,
    workers: usize,
) -> Result<u64, CoreError> {
    if workers == 0 {
        return Err(CoreError::Crypto("workers must be >= 1".to_string()));
    }
    let mut header_file = File::open(input).map_err(|e| CoreError::Crypto(e.to_string()))?;
    let (chunk_size, base_nonce) = read_header(&mut header_file)?;
    let total_plain = encrypted_plaintext_len(input)?;
    if total_plain == 0 {
        File::create(output).map_err(|e| CoreError::Crypto(e.to_string()))?;
        return Ok(0);
    }
    let ct_chunk_size = chunk_size + TAG_LEN;
    let total_chunks = ((total_plain + chunk_size as u64 - 1) / chunk_size as u64) as usize;

    let out_file = File::create(output).map_err(|e| CoreError::Crypto(e.to_string()))?;
    out_file
        .set_len(total_plain)
        .map_err(|e| CoreError::Crypto(e.to_string()))?;

    let next_index = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for _ in 0..workers {
        let input_file = File::open(input).map_err(|e| CoreError::Crypto(e.to_string()))?;
        let output_file = out_file.try_clone().map_err(|e| CoreError::Crypto(e.to_string()))?;
        let next_index = Arc::clone(&next_index);
        let key_bytes = *key.as_bytes();
        let base_nonce = base_nonce;
        let handle = thread::spawn(move || -> Result<(), CoreError> {
            let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
            loop {
                let idx = next_index.fetch_add(1, Ordering::Relaxed);
                if idx >= total_chunks {
                    break;
                }
                let plain_len = if idx == total_chunks - 1 {
                    (total_plain - (idx as u64 * chunk_size as u64)) as usize
                } else {
                    chunk_size
                };
                let ct_len = plain_len + TAG_LEN;
                let offset = HEADER_LEN + idx as u64 * ct_chunk_size as u64;

                let mut ct_buf = vec![0u8; ct_len];
                read_exact_at(&input_file, &mut ct_buf, offset)?;
                let nonce = nonce_for_chunk(&base_nonce, idx as u64);
                let pt = cipher
                    .decrypt(Nonce::from_slice(&nonce), ct_buf.as_ref())
                    .map_err(|e| CoreError::Crypto(format!("decrypt failed: {e}")))?;
                write_exact_at(&output_file, &pt, idx as u64 * chunk_size as u64)?;
            }
            Ok(())
        });
        handles.push(handle);
    }

    for handle in handles {
        match handle.join() {
            Ok(res) => res?,
            Err(_) => return Err(CoreError::Crypto("decrypt thread panicked".to_string())),
        }
    }

    Ok(total_plain)
}

fn read_exact_at(file: &File, buf: &mut [u8], mut offset: u64) -> Result<(), CoreError> {
    let mut filled = 0;
    while filled < buf.len() {
        let read = file
            .read_at(&mut buf[filled..], offset)
            .map_err(|e| CoreError::Crypto(e.to_string()))?;
        if read == 0 {
            return Err(CoreError::Crypto("unexpected EOF".to_string()));
        }
        filled += read;
        offset += read as u64;
    }
    Ok(())
}

fn write_exact_at(file: &File, buf: &[u8], mut offset: u64) -> Result<(), CoreError> {
    let mut written = 0;
    while written < buf.len() {
        let wrote = file
            .write_at(&buf[written..], offset)
            .map_err(|e| CoreError::Crypto(e.to_string()))?;
        if wrote == 0 {
            return Err(CoreError::Crypto("failed to write output".to_string()));
        }
        written += wrote;
        offset += wrote as u64;
    }
    Ok(())
}

fn nonce_for_chunk(base: &[u8; 12], counter: u64) -> [u8; 12] {
    let mut nonce = *base;
    nonce[4..].copy_from_slice(&counter.to_be_bytes());
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        set_test_key_from_passphrase("golden-thread-tests");
        let key = load_or_create_master_key().expect("key");
        let dir = tempdir().expect("temp");
        let src = dir.path().join("src.bin");
        let enc = dir.path().join("enc.bin");
        let out = dir.path().join("out.bin");
        fs::write(&src, b"hello world").expect("write");
        encrypt_file_to_path(&src, &enc, &key).expect("encrypt");
        decrypt_file_to_path(&enc, &out, &key).expect("decrypt");
        let roundtrip = fs::read(&out).expect("read");
        assert_eq!(roundtrip, b"hello world");
    }

    #[test]
    fn encrypted_plaintext_len_matches() {
        set_test_key_from_passphrase("golden-thread-tests");
        let key = load_or_create_master_key().expect("key");
        let dir = tempdir().expect("temp");
        let src = dir.path().join("src.bin");
        let enc = dir.path().join("enc.bin");
        let data = vec![42u8; 4096];
        fs::write(&src, &data).expect("write");
        let mut reader = File::open(&src).expect("open");
        let mut writer = File::create(&enc).expect("create");
        encrypt_stream_with_hash_chunk(&mut reader, &mut writer, &key, DEFAULT_CHUNK_SIZE).expect("encrypt");
        let len = encrypted_plaintext_len(&enc).expect("len");
        assert_eq!(len, data.len() as u64);
    }

    #[test]
    fn decrypt_file_parallel_roundtrip() {
        set_test_key_from_passphrase("golden-thread-tests");
        let key = load_or_create_master_key().expect("key");
        let dir = tempdir().expect("temp");
        let src = dir.path().join("src.bin");
        let enc = dir.path().join("enc.bin");
        let out = dir.path().join("out.bin");
        let data = vec![7u8; 2 * 1024 * 1024 + 123];
        fs::write(&src, &data).expect("write");
        encrypt_file_to_path(&src, &enc, &key).expect("encrypt");
        decrypt_file_parallel(&enc, &out, &key, 4).expect("decrypt");
        let roundtrip = fs::read(&out).expect("read");
        assert_eq!(roundtrip, data);
    }

    #[test]
    fn derive_key_produces_different_keys_per_purpose() {
        set_test_key_from_passphrase("golden-thread-tests");
        let master = load_or_create_master_key().expect("key");

        let db_key = derive_key(&master, KeyPurpose::Database).expect("db key");
        let attach_key = derive_key(&master, KeyPurpose::Attachments).expect("attach key");

        // Derived keys should be different from each other
        assert_ne!(db_key.as_bytes(), attach_key.as_bytes());
        // Derived keys should be different from the master key
        assert_ne!(db_key.as_bytes(), master.as_bytes());
        assert_ne!(attach_key.as_bytes(), master.as_bytes());
    }

    #[test]
    fn derive_key_is_deterministic() {
        set_test_key_from_passphrase("golden-thread-tests");
        let master = load_or_create_master_key().expect("key");

        let key1 = derive_key(&master, KeyPurpose::Database).expect("key1");
        let key2 = derive_key(&master, KeyPurpose::Database).expect("key2");

        // Same master + purpose should produce same derived key
        assert_eq!(key1.as_bytes(), key2.as_bytes());
    }
}
