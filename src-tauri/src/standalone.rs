use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

include!(concat!(env!("OUT_DIR"), "/standalone_assets.rs"));

pub fn prepare_runtime() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                "Windows LOCALAPPDATA is unavailable; embedded runtime cannot be released."
                    .to_string()
            })?;
        prepare_runtime_at(
            &local_app_data.join("Jev-Switch").join("runtime"),
            env!("CARGO_PKG_VERSION"),
            EMBEDDED_ASSETS,
        )
    }

    #[cfg(not(windows))]
    {
        Err("The standalone desktop package is currently supported on Windows only.".into())
    }
}

fn prepare_runtime_at(
    runtime_root: &Path,
    version: &str,
    assets: &[EmbeddedAsset],
) -> Result<PathBuf, String> {
    validate_assets(assets)?;
    let fingerprint = runtime_fingerprint(assets);
    let version_root = runtime_root.join(version);
    fs::create_dir_all(&version_root).map_err(|error| {
        format!(
            "Cannot create standalone runtime cache {}: {error}",
            version_root.display()
        )
    })?;

    let runtime_dir = version_root.join(fingerprint);
    if cache_matches(&runtime_dir, assets) {
        return Ok(runtime_dir);
    }

    let suffix = unique_suffix();
    let staging_dir = version_root.join(format!(".staging-{suffix}"));
    let backup_dir = version_root.join(format!(".corrupt-{suffix}"));
    if staging_dir.exists() || backup_dir.exists() {
        return Err(
            "A standalone runtime repair path already exists; close Jev-Switch and retry.".into(),
        );
    }

    if let Err(error) = extract_assets(&staging_dir, assets) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(error);
    }
    if !cache_matches(&staging_dir, assets) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(
            "The embedded runtime did not pass its integrity check after extraction.".into(),
        );
    }

    let moved_existing = if runtime_dir.exists() {
        fs::rename(&runtime_dir, &backup_dir).map_err(|error| {
            let _ = fs::remove_dir_all(&staging_dir);
            format!(
                "Cannot replace the damaged standalone runtime at {}. Close Jev-Switch and retry: {error}",
                runtime_dir.display()
            )
        })?;
        true
    } else {
        false
    };

    if let Err(error) = fs::rename(&staging_dir, &runtime_dir) {
        if moved_existing {
            let _ = fs::rename(&backup_dir, &runtime_dir);
        }
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!(
            "Cannot activate the standalone runtime cache at {}: {error}",
            runtime_dir.display()
        ));
    }

    if moved_existing {
        if let Err(error) = fs::remove_dir_all(&backup_dir) {
            eprintln!(
                "standalone runtime repaired; could not remove damaged cache {}: {error}",
                backup_dir.display()
            );
        }
    }

    Ok(runtime_dir)
}

fn validate_assets(assets: &[EmbeddedAsset]) -> Result<(), String> {
    if assets.is_empty() {
        return Err("The standalone executable contains no embedded runtime assets.".into());
    }

    let mut paths = std::collections::HashSet::new();
    for asset in assets {
        let relative = safe_relative_path(asset.path)?;
        if !paths.insert(relative) {
            return Err(format!(
                "The standalone executable contains duplicate asset path '{}'.",
                asset.path
            ));
        }
    }

    for required in ["jev-switch-daemon.exe", "ui/dist/index.html"] {
        if !assets.iter().any(|asset| asset.path == required) {
            return Err(format!(
                "The standalone executable is missing required asset '{required}'."
            ));
        }
    }
    Ok(())
}

fn safe_relative_path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "The standalone executable contains an unsafe asset path '{value}'."
        ));
    }
    Ok(path.to_path_buf())
}

fn extract_assets(destination: &Path, assets: &[EmbeddedAsset]) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| {
        format!(
            "Cannot prepare runtime staging directory {}: {error}",
            destination.display()
        )
    })?;

    for asset in assets {
        let relative = safe_relative_path(asset.path)?;
        let target = destination.join(relative);
        let parent = target
            .parent()
            .ok_or_else(|| format!("Invalid standalone asset path '{}'.", asset.path))?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Cannot create runtime asset directory {}: {error}",
                parent.display()
            )
        })?;
        fs::write(&target, asset.bytes).map_err(|error| {
            format!(
                "Cannot release embedded runtime file {}: {error}",
                target.display()
            )
        })?;
    }
    Ok(())
}

fn cache_matches(directory: &Path, assets: &[EmbeddedAsset]) -> bool {
    assets.iter().all(|asset| {
        safe_relative_path(asset.path)
            .ok()
            .and_then(|relative| fs::read(directory.join(relative)).ok())
            .is_some_and(|bytes| bytes == asset.bytes)
    })
}

fn runtime_fingerprint(assets: &[EmbeddedAsset]) -> String {
    let mut hasher = Sha256::new();
    for asset in assets {
        hasher.update((asset.path.len() as u64).to_le_bytes());
        hasher.update(asset.path.as_bytes());
        hasher.update((asset.bytes.len() as u64).to_le_bytes());
        hasher.update(asset.bytes);
    }
    format!("{:x}", hasher.finalize())
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::{prepare_runtime_at, validate_assets, EmbeddedAsset, EMBEDDED_ASSETS};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_ASSETS: &[EmbeddedAsset] = &[
        EmbeddedAsset {
            path: "jev-switch-daemon.exe",
            bytes: b"trusted daemon bytes",
        },
        EmbeddedAsset {
            path: "ui/dist/index.html",
            bytes: b"<html>trusted UI</html>",
        },
    ];

    fn isolated_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "jev-standalone-cache-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn cold_start_extracts_once_and_reuses_identical_cache() {
        let root = isolated_root();
        let first = prepare_runtime_at(&root, env!("CARGO_PKG_VERSION"), TEST_ASSETS).unwrap();
        let second = prepare_runtime_at(&root, env!("CARGO_PKG_VERSION"), TEST_ASSETS).unwrap();

        assert_eq!(first, second);
        assert!(first.starts_with(&root.join(env!("CARGO_PKG_VERSION"))));
        assert_eq!(
            fs::read(first.join("jev-switch-daemon.exe")).unwrap(),
            TEST_ASSETS[0].bytes
        );
        assert_eq!(
            fs::read(first.join("ui/dist/index.html")).unwrap(),
            TEST_ASSETS[1].bytes
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_or_corrupt_cache_files_are_released_again() {
        let root = isolated_root();
        let runtime = prepare_runtime_at(&root, env!("CARGO_PKG_VERSION"), TEST_ASSETS).unwrap();
        fs::write(runtime.join("jev-switch-daemon.exe"), b"damaged daemon").unwrap();

        let repaired = prepare_runtime_at(&root, env!("CARGO_PKG_VERSION"), TEST_ASSETS).unwrap();
        assert_eq!(
            fs::read(repaired.join("jev-switch-daemon.exe")).unwrap(),
            TEST_ASSETS[0].bytes
        );
        assert!(repaired.join("ui/dist/index.html").is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_required_embedded_assets_fail_before_writing_cache() {
        let root = isolated_root();
        let error = validate_assets(&[TEST_ASSETS[0]]).unwrap_err();
        assert!(error.contains("ui/dist/index.html"));
        assert!(!root.exists());
    }

    #[test]
    fn asset_paths_cannot_escape_the_runtime_cache() {
        let error = validate_assets(&[
            TEST_ASSETS[0],
            TEST_ASSETS[1],
            EmbeddedAsset {
                path: "../outside.exe",
                bytes: b"bad",
            },
        ])
        .unwrap_err();
        assert!(error.contains("unsafe asset path"));
    }

    #[test]
    fn packaged_assets_include_valid_daemon_and_complete_ui() {
        validate_assets(EMBEDDED_ASSETS).unwrap();
        let root = isolated_root();
        let runtime =
            prepare_runtime_at(&root, env!("CARGO_PKG_VERSION"), EMBEDDED_ASSETS).unwrap();
        let daemon = fs::read(runtime.join("jev-switch-daemon.exe")).unwrap();
        let html = fs::read_to_string(runtime.join("ui/dist/index.html")).unwrap();

        assert!(daemon.starts_with(b"MZ"));
        assert!(html.contains("<div id=\"root\""));
        assert!(EMBEDDED_ASSETS.len() >= 3);
        fs::remove_dir_all(root).unwrap();
    }
}
