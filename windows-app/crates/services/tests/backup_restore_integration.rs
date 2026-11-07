//! Integration tests for spec task 11.3: backup→restore round trips for both the unencrypted
//! and encrypted bundle formats, asserting manifest contents and the free-space check — against
//! the public `backup_restore` API only, as a consumer of the crate would use it (distinct from
//! `backup_restore.rs`'s own unit tests, which exercise internal round trips and edge cases like
//! the wrong-passphrase case).

use std::path::Path;

use walkman_platform::fake::FakeVolumeProvider;
use walkman_platform::FreeSpace;
use walkman_services::backup_restore::{
    create_encrypted_backup, create_unencrypted_backup, restore_encrypted_backup, restore_unencrypted_backup,
    verify_free_space, BackupScope, BackupSource,
};

fn write_fixture_library(root: &Path) -> Vec<BackupSource> {
    let files = [
        ("Nova/Horizon/01 - Dawn.flac", "dawn audio bytes"),
        ("Nova/Horizon/02 - Dusk.flac", "dusk audio bytes, a bit longer"),
        ("Echo/Reverb/01 - Signal.flac", "signal audio bytes"),
        ("Playlists/Favorites.m3u8", "#EXTM3U\r\n"),
    ];
    let mut sources = Vec::new();
    for (relative, content) in files {
        let full_path = root.join(relative);
        std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        std::fs::write(&full_path, content).unwrap();
        sources.push(BackupSource { full_path, relative_path: relative.replace('\\', "/") });
    }
    sources
}

#[test]
fn unencrypted_round_trip_matches_manifest_and_free_space_gate_passes() {
    let library_dir = tempfile::tempdir().unwrap();
    let bundle_dir = tempfile::tempdir().unwrap();
    let restore_dir = tempfile::tempdir().unwrap();
    let sources = write_fixture_library(library_dir.path());
    let expected_total: u64 = sources.iter().map(|s| std::fs::metadata(&s.full_path).unwrap().len()).sum();

    let volumes = FakeVolumeProvider::new();
    volumes.set_free_space(restore_dir.path(), FreeSpace { total_bytes: 1_000_000, free_bytes: 500_000 });
    verify_free_space(&volumes, restore_dir.path(), expected_total).expect("plenty of free space for tiny fixtures");

    let backup_manifest =
        create_unencrypted_backup(&sources, bundle_dir.path(), BackupScope::FullLibrary, &|| false).unwrap();
    assert_eq!(backup_manifest.entries.len(), 4);
    assert_eq!(backup_manifest.total_bytes, expected_total);
    assert!(!backup_manifest.encrypted);

    let restore_manifest = restore_unencrypted_backup(bundle_dir.path(), restore_dir.path(), &|| false).unwrap();
    assert_eq!(restore_manifest, backup_manifest);

    for source in &sources {
        let restored = restore_dir.path().join(&source.relative_path);
        assert_eq!(
            std::fs::read(&restored).unwrap(),
            std::fs::read(&source.full_path).unwrap(),
            "{} did not round-trip byte-for-byte",
            source.relative_path
        );
    }
}

#[test]
fn encrypted_round_trip_matches_manifest_and_free_space_gate_rejects_too_little_space() {
    let library_dir = tempfile::tempdir().unwrap();
    let bundle_dir = tempfile::tempdir().unwrap();
    let restore_dir = tempfile::tempdir().unwrap();
    let sources = write_fixture_library(library_dir.path());
    let expected_total: u64 = sources.iter().map(|s| std::fs::metadata(&s.full_path).unwrap().len()).sum();
    let bundle_path = bundle_dir.path().join("library.walkmanbackup.enc");

    let volumes = FakeVolumeProvider::new();
    volumes.set_free_space(restore_dir.path(), FreeSpace { total_bytes: 1000, free_bytes: 1 });
    assert!(
        verify_free_space(&volumes, restore_dir.path(), expected_total).is_err(),
        "1 free byte must fail a multi-file restore"
    );

    let backup_manifest =
        create_encrypted_backup(&sources, &bundle_path, BackupScope::Playlists, "s3cret passphrase", &|| false).unwrap();
    assert_eq!(backup_manifest.entries.len(), 4);
    assert_eq!(backup_manifest.total_bytes, expected_total);
    assert!(backup_manifest.encrypted);

    // Now simulate the gate passing (plenty of room) and actually restore.
    volumes.set_free_space(restore_dir.path(), FreeSpace { total_bytes: 1_000_000, free_bytes: 500_000 });
    verify_free_space(&volumes, restore_dir.path(), expected_total).unwrap();

    let restore_manifest = restore_encrypted_backup(&bundle_path, "s3cret passphrase", restore_dir.path(), &|| false).unwrap();
    assert_eq!(restore_manifest, backup_manifest);

    for source in &sources {
        let restored = restore_dir.path().join(&source.relative_path);
        assert_eq!(std::fs::read(&restored).unwrap(), std::fs::read(&source.full_path).unwrap());
    }
}
