//! Backup & Restore (spec task 11.2, R12.1-12.6).
//!
//! Two bundle formats:
//! - **Unencrypted**: a plain directory (`manifest.json` + a `files/` tree mirroring each
//!   source's relative path) — R12.4's "unencrypted backups SHALL produce a bundle with a
//!   manifest".
//! - **Encrypted**: a single `*.walkmanbackup.enc` file: a 16-byte random salt, a
//!   PBKDF2-HMAC-SHA256 (100,000 iterations)-derived AES-256-GCM key, then the JSON manifest and
//!   every source file each sealed as their own AES-GCM ciphertext with a fresh random nonce.
//!   Encrypting file-by-file (rather than the whole backup as one ciphertext) is this port's
//!   reading of "streamed" — it never needs to hold more than one source file in memory at a
//!   time, while staying within the simpler one-shot AEAD API (`aes-gcm`'s chunked `stream`
//!   construction was judged not worth the added API-surface risk for what this port needs).
//!
//! Fast copies go through `platform::fast_copy` (`CopyFileExW` + `FlushFileBuffers`, R12.6);
//! free-space verification goes through `platform::VolumeProvider` (R12.6).

use std::io::Write;
use std::path::{Path, PathBuf};

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use walkman_platform::VolumeProvider;

use crate::error::{AppError, Result};

/// R12.2's five backup scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupScope {
    FullLibrary,
    Playlists,
    LanguageStudy,
    SelectedAlbums,
    SelectedTracks,
}

/// One file to back up: its real location plus the relative path it should occupy inside the
/// bundle (and be restored back to, under the restore destination).
#[derive(Debug, Clone)]
pub struct BackupSource {
    pub full_path: PathBuf,
    pub relative_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackupEntry {
    pub relative_path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackupManifest {
    pub scope: BackupScope,
    pub created_at: String,
    pub entries: Vec<BackupEntry>,
    pub total_bytes: u64,
    pub encrypted: bool,
}

/// `%USERPROFILE%\Music\Walkman Studio Backups` (R12.3's default local destination), falling
/// back to the system temp directory if `USERPROFILE` isn't set (never expected on a real
/// Windows session, but keeps this function total).
pub fn default_backup_dir() -> PathBuf {
    let base = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Music").join("Walkman Studio Backups")
}

/// Verifies `dest`'s volume has at least `required_bytes` free (R12.5/12.6).
pub fn verify_free_space(volumes: &dyn VolumeProvider, dest: &Path, required_bytes: u64) -> Result<()> {
    let free = volumes.free_space(dest)?;
    if (free.free_bytes as u64) < required_bytes {
        return Err(AppError::Backup(format!(
            "not enough free space at {}: need {required_bytes} bytes, have {} free",
            dest.display(),
            free.free_bytes
        )));
    }
    Ok(())
}

fn build_manifest(sources: &[BackupSource], scope: BackupScope, encrypted: bool) -> Result<BackupManifest> {
    let mut entries = Vec::with_capacity(sources.len());
    let mut total_bytes = 0u64;
    for source in sources {
        let size = std::fs::metadata(&source.full_path)?.len();
        total_bytes += size;
        entries.push(BackupEntry { relative_path: source.relative_path.clone(), size_bytes: size });
    }
    Ok(BackupManifest { scope, created_at: Utc::now().to_rfc3339(), entries, total_bytes, encrypted })
}

/// A cancelation check the caller polls between files; returning `true` aborts the operation
/// with [`AppError::Backup`] (R12.5: "backup/restore SHALL be cancelable").
pub type CancelCheck<'a> = &'a dyn Fn() -> bool;

fn check_canceled(cancel: CancelCheck) -> Result<()> {
    if cancel() {
        return Err(AppError::Backup("canceled".to_string()));
    }
    Ok(())
}

/// Writes an unencrypted bundle to `dest_dir`: `manifest.json` plus a `files/` tree. Uses
/// `platform::fast_copy` on Windows (R12.6); falls back to `std::fs::copy` elsewhere (e.g. this
/// crate's own non-Windows test runs, if any — the production target is Windows-only).
pub fn create_unencrypted_backup(
    sources: &[BackupSource],
    dest_dir: &Path,
    scope: BackupScope,
    cancel: CancelCheck,
) -> Result<BackupManifest> {
    std::fs::create_dir_all(dest_dir)?;
    let files_dir = dest_dir.join("files");
    let manifest = build_manifest(sources, scope, false)?;

    for source in sources {
        check_canceled(cancel)?;
        let dest_path = files_dir.join(&source.relative_path);
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        copy_file(&source.full_path, &dest_path)?;
    }

    std::fs::write(dest_dir.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
    Ok(manifest)
}

/// Restores an unencrypted bundle created by [`create_unencrypted_backup`] into `dest_root`,
/// then runs junk-file cleanup (R12.5).
pub fn restore_unencrypted_backup(bundle_dir: &Path, dest_root: &Path, cancel: CancelCheck) -> Result<BackupManifest> {
    let manifest_bytes = std::fs::read(bundle_dir.join("manifest.json"))?;
    let manifest: BackupManifest = serde_json::from_slice(&manifest_bytes)?;

    for entry in &manifest.entries {
        check_canceled(cancel)?;
        let src = bundle_dir.join("files").join(&entry.relative_path);
        let dst = dest_root.join(&entry.relative_path);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        copy_file(&src, &dst)?;
    }

    crate::clean_eject::clean_junk(dest_root);
    Ok(manifest)
}

#[cfg(windows)]
fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    walkman_platform::fast_copy(src, dst)?;
    Ok(())
}
#[cfg(not(windows))]
fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    std::fs::copy(src, dst)?;
    Ok(())
}

const MAGIC: &[u8; 4] = b"WSBK";
const VERSION: u8 = 1;
const PBKDF2_ROUNDS: u32 = 100_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

fn derive_key(passphrase: &str, salt: &[u8; SALT_LEN]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), salt, PBKDF2_ROUNDS, &mut key);
    key
}

fn random_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

fn write_block(out: &mut impl Write, nonce: &[u8; NONCE_LEN], ciphertext: &[u8]) -> Result<()> {
    out.write_all(nonce)?;
    out.write_all(&(ciphertext.len() as u32).to_le_bytes())?;
    out.write_all(ciphertext)?;
    Ok(())
}

/// Writes an encrypted `*.walkmanbackup.enc` bundle to `dest_file` (R12.4).
pub fn create_encrypted_backup(
    sources: &[BackupSource],
    dest_file: &Path,
    scope: BackupScope,
    passphrase: &str,
    cancel: CancelCheck,
) -> Result<BackupManifest> {
    let manifest = build_manifest(sources, scope, true)?;

    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let key = derive_key(passphrase, &salt);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

    let mut out = std::fs::File::create(dest_file)?;
    out.write_all(MAGIC)?;
    out.write_all(&[VERSION])?;
    out.write_all(&salt)?;

    let manifest_json = serde_json::to_vec(&manifest)?;
    let manifest_nonce = random_nonce();
    let manifest_ct = cipher
        .encrypt(Nonce::from_slice(&manifest_nonce), manifest_json.as_slice())
        .map_err(|e| AppError::Backup(format!("failed to encrypt manifest: {e}")))?;
    write_block(&mut out, &manifest_nonce, &manifest_ct)?;

    for source in sources {
        check_canceled(cancel)?;
        let data = std::fs::read(&source.full_path)?; // one source file in memory at a time
        let nonce = random_nonce();
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), data.as_slice())
            .map_err(|e| AppError::Backup(format!("failed to encrypt {}: {e}", source.relative_path)))?;
        write_block(&mut out, &nonce, &ciphertext)?;
    }

    out.flush()?;
    Ok(manifest)
}

/// Opens `path`, derives the key from `passphrase`, and returns `(cipher, manifest, body_bytes,
/// body_start_offset)` — `body_bytes`/`body_start_offset` let the caller walk the per-file
/// blocks that follow the manifest without re-reading the file.
fn open_encrypted(path: &Path, passphrase: &str) -> Result<(Aes256Gcm, BackupManifest, Vec<u8>, usize)> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 4 + 1 + SALT_LEN || &bytes[0..4] != MAGIC {
        return Err(AppError::Backup("not a Walkman Studio encrypted backup".to_string()));
    }
    let version = bytes[4];
    if version != VERSION {
        return Err(AppError::Backup(format!("unsupported encrypted backup version {version}")));
    }
    let salt: [u8; SALT_LEN] = bytes[5..5 + SALT_LEN].try_into().unwrap();
    let key = derive_key(passphrase, &salt);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

    let mut pos = 5 + SALT_LEN;
    let manifest_plain = decrypt_block(&cipher, &bytes, &mut pos)?;
    let manifest: BackupManifest = serde_json::from_slice(&manifest_plain)
        .map_err(|e| AppError::Backup(format!("failed to parse manifest (wrong passphrase?): {e}")))?;

    Ok((cipher, manifest, bytes, pos))
}

fn decrypt_block(cipher: &Aes256Gcm, bytes: &[u8], pos: &mut usize) -> Result<Vec<u8>> {
    if *pos + NONCE_LEN + 4 > bytes.len() {
        return Err(AppError::Backup("truncated encrypted backup (block header)".to_string()));
    }
    let nonce_bytes: [u8; NONCE_LEN] = bytes[*pos..*pos + NONCE_LEN].try_into().unwrap();
    *pos += NONCE_LEN;
    let len = u32::from_le_bytes(bytes[*pos..*pos + 4].try_into().unwrap()) as usize;
    *pos += 4;
    if *pos + len > bytes.len() {
        return Err(AppError::Backup("truncated encrypted backup (block body)".to_string()));
    }
    let ciphertext = &bytes[*pos..*pos + len];
    *pos += len;
    cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), ciphertext)
        .map_err(|_| AppError::Backup("decryption failed — wrong passphrase or corrupt backup".to_string()))
}

/// Reads an encrypted bundle's manifest without restoring anything (for showing backup contents
/// before committing to a restore).
pub fn read_encrypted_manifest(path: &Path, passphrase: &str) -> Result<BackupManifest> {
    Ok(open_encrypted(path, passphrase)?.1)
}

/// Restores an encrypted bundle created by [`create_encrypted_backup`] into `dest_root`, then
/// runs junk-file cleanup (R12.5).
pub fn restore_encrypted_backup(path: &Path, passphrase: &str, dest_root: &Path, cancel: CancelCheck) -> Result<BackupManifest> {
    let (cipher, manifest, bytes, mut pos) = open_encrypted(path, passphrase)?;

    for entry in &manifest.entries {
        check_canceled(cancel)?;
        let plaintext = decrypt_block(&cipher, &bytes, &mut pos)?;
        let dst = dest_root.join(&entry.relative_path);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dst, &plaintext)?;
    }

    crate::clean_eject::clean_junk(dest_root);
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sources(dir: &Path) -> Vec<BackupSource> {
        let a = dir.join("Artist/Album/01 - Song.flac");
        let b = dir.join("Playlists/Mix.m3u8");
        std::fs::create_dir_all(a.parent().unwrap()).unwrap();
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, b"fake flac bytes").unwrap();
        std::fs::write(&b, "#EXTM3U\r\n").unwrap();
        vec![
            BackupSource { full_path: a, relative_path: "Artist/Album/01 - Song.flac".to_string() },
            BackupSource { full_path: b, relative_path: "Playlists/Mix.m3u8".to_string() },
        ]
    }

    #[test]
    fn unencrypted_backup_round_trips_files_and_manifest() {
        let source_dir = tempfile::tempdir().unwrap();
        let bundle_dir = tempfile::tempdir().unwrap();
        let restore_dir = tempfile::tempdir().unwrap();
        let sources = make_sources(source_dir.path());

        let manifest = create_unencrypted_backup(&sources, bundle_dir.path(), BackupScope::FullLibrary, &|| false).unwrap();
        assert_eq!(manifest.entries.len(), 2);
        assert!(!manifest.encrypted);
        assert!(bundle_dir.path().join("manifest.json").exists());

        let restored = restore_unencrypted_backup(bundle_dir.path(), restore_dir.path(), &|| false).unwrap();
        assert_eq!(restored, manifest);
        assert_eq!(
            std::fs::read(restore_dir.path().join("Artist/Album/01 - Song.flac")).unwrap(),
            b"fake flac bytes"
        );
        assert!(restore_dir.path().join("Playlists/Mix.m3u8").exists());
    }

    #[test]
    fn encrypted_backup_round_trips_and_rejects_the_wrong_passphrase() {
        let source_dir = tempfile::tempdir().unwrap();
        let restore_dir = tempfile::tempdir().unwrap();
        let bundle_dir = tempfile::tempdir().unwrap();
        let sources = make_sources(source_dir.path());
        let bundle_path = bundle_dir.path().join("backup.walkmanbackup.enc");

        let manifest =
            create_encrypted_backup(&sources, &bundle_path, BackupScope::SelectedTracks, "correct horse", &|| false).unwrap();
        assert!(manifest.encrypted);

        assert!(read_encrypted_manifest(&bundle_path, "wrong passphrase").is_err());

        let read_back = read_encrypted_manifest(&bundle_path, "correct horse").unwrap();
        assert_eq!(read_back, manifest);

        let restored = restore_encrypted_backup(&bundle_path, "correct horse", restore_dir.path(), &|| false).unwrap();
        assert_eq!(restored, manifest);
        assert_eq!(
            std::fs::read(restore_dir.path().join("Artist/Album/01 - Song.flac")).unwrap(),
            b"fake flac bytes"
        );
    }

    #[test]
    fn backup_is_cancelable_mid_operation() {
        let source_dir = tempfile::tempdir().unwrap();
        let bundle_dir = tempfile::tempdir().unwrap();
        let sources = make_sources(source_dir.path());

        // `&dyn Fn() -> bool` can't capture a plain `&mut` counter, so the mutable call count
        // lives in a `Cell` instead.
        let counter = std::cell::Cell::new(0);
        let cancel = || {
            counter.set(counter.get() + 1);
            counter.get() > 1 // cancel starting from the second file
        };

        let result = create_unencrypted_backup(&sources, bundle_dir.path(), BackupScope::FullLibrary, &cancel);
        assert!(result.is_err());
    }

    #[test]
    fn verify_free_space_reports_an_error_when_insufficient() {
        use walkman_platform::fake::FakeVolumeProvider;
        use walkman_platform::FreeSpace;

        let dir = tempfile::tempdir().unwrap();
        let provider = FakeVolumeProvider::new();
        provider.set_free_space(dir.path(), FreeSpace { total_bytes: 1000, free_bytes: 100 });

        assert!(verify_free_space(&provider, dir.path(), 50).is_ok());
        assert!(verify_free_space(&provider, dir.path(), 500).is_err());
    }

    #[test]
    fn default_backup_dir_is_under_music() {
        let dir = default_backup_dir();
        assert!(dir.ends_with("Music/Walkman Studio Backups") || dir.ends_with(r"Music\Walkman Studio Backups"));
    }
}
