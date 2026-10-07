use crate::dl::{self, download};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const RELEASES_URL: &str =
    "https://storage.googleapis.com/flutter_infra_release/releases/releases_linux.json";
pub const STORAGE_URL: &str = "https://storage.googleapis.com/flutter_infra_release/releases";

/// A single Flutter release resolved from the release metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub archive: String,
    pub sha256: String,
}

/// Map the host CPU to the architecture key used by Flutter release metadata.
pub fn host_arch() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("x64"),
        "aarch64" => Ok("arm64"),
        other => bail!("unsupported Flutter Linux architecture: {other}"),
    }
}

/// Resolve a release from release-metadata JSON.
///
/// `version_req` is `stable` or a pinned version like `3.38.0` (an
/// optional leading `v` is accepted). Mirrors the Bash-era rule: the
/// stable hash must match a `stable`-channel entry for this arch.
pub fn resolve_release(metadata_json: &str, version_req: &str, arch: &str) -> Result<Release> {
    let data: serde_json::Value =
        serde_json::from_str(metadata_json).context("invalid Flutter release metadata")?;
    let releases = data
        .get("releases")
        .and_then(|r| r.as_array())
        .context("release metadata has no releases list")?;

    let want = version_req.strip_prefix('v').unwrap_or(version_req);
    let entry = if want == "stable" {
        let stable_hash = data
            .pointer("/current_release/stable")
            .and_then(|h| h.as_str())
            .context("release metadata has no stable hash")?;
        releases
            .iter()
            .find(|r| {
                r.get("hash").and_then(|h| h.as_str()) == Some(stable_hash)
                    && r.get("channel").and_then(|c| c.as_str()) == Some("stable")
                    && r.get("dart_sdk_arch").is_none_or(|a| a == arch)
            })
            .with_context(|| format!("no stable Linux Flutter release found for {arch}"))?
    } else {
        releases
            .iter()
            .find(|r| {
                r.get("version")
                    .and_then(|v| v.as_str())
                    .is_some_and(|v| v.strip_prefix('v').unwrap_or(v) == want)
            })
            .with_context(|| format!("no Flutter release found for version {version_req}"))?
    };

    Ok(Release {
        version: entry
            .get("version")
            .and_then(|v| v.as_str())
            .context("release entry has no version")?
            .to_owned(),
        archive: entry
            .get("archive")
            .and_then(|a| a.as_str())
            .context("release entry has no archive")?
            .to_owned(),
        sha256: entry
            .get("sha256")
            .and_then(|s| s.as_str())
            .context("release entry has no sha256")?
            .to_owned(),
    })
}

/// Verify a downloaded file against its expected lowercase hex SHA-256.
pub fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    let mut file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).context("cannot hash download")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != expected.to_lowercase() {
        bail!(
            "checksum mismatch for {}: expected {expected}, got {actual}",
            path.display()
        );
    }
    Ok(())
}

fn staging_dir(environment: &Path) -> PathBuf {
    environment.join("state/staging/flutter")
}

/// Provision the Flutter SDK into `environment`.
///
/// Returns the installed version. Reinstalling the same resolved version
/// is a no-op (no download). The `flutter.ready` marker is written only
/// after `flutter --version` succeeds, so a failed validation never leaves
/// a stale ready marker behind.
pub fn provision(environment: &Path, version_req: &str) -> Result<String> {
    let flutter_bin = environment.join("flutter/bin/flutter");
    let state = environment.join("state");
    let ready = state.join("flutter.ready");
    let recorded = state.join("flutter-version");

    eprintln!("  → Resolving Flutter ({version_req})");
    let metadata = dl::fetch_string(RELEASES_URL, "Flutter release metadata")?;
    let arch = host_arch()?;
    let release = resolve_release(&metadata, version_req, arch)?;

    if ready.exists()
        && flutter_bin.is_file()
        && fs::read_to_string(&recorded)
            .map(|v| v.trim().to_owned())
            .ok()
            == Some(release.version.clone())
    {
        println!("  ✓ Flutter {} already provisioned", release.version);
        return Ok(release.version);
    }

    let staging = staging_dir(environment);
    fs::create_dir_all(&staging).with_context(|| format!("cannot create {}", staging.display()))?;
    let archive_name = release
        .archive
        .rsplit('/')
        .next()
        .context("invalid archive path")?;
    let bundle = staging.join(archive_name);

    eprintln!("  → Downloading Flutter SDK {}", release.version);
    let need_download = !bundle.is_file() || verify_sha256(&bundle, &release.sha256).is_err();
    if need_download {
        let _ = fs::remove_file(&bundle);
        download(
            &format!("{STORAGE_URL}/{}", release.archive),
            &bundle,
            "Flutter SDK",
        )?;
    }
    verify_sha256(&bundle, &release.sha256).context("downloaded Flutter SDK failed checksum")?;

    eprintln!("  → Extracting");
    let extract_dir = staging.join("extracted");
    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir).context("cannot clear staging area")?;
    }
    fs::create_dir_all(&extract_dir)?;
    let status = Command::new("tar")
        .args(["--no-same-owner", "-xf"])
        .arg(&bundle)
        .arg("-C")
        .arg(&extract_dir)
        .status()
        .context("cannot run tar; it is required for extraction")?;
    if !status.success() {
        bail!("Flutter SDK extraction failed");
    }
    if !extract_dir.join("flutter/bin/flutter").is_file() {
        bail!("Flutter archive did not contain a valid SDK");
    }

    // Replace the SDK only after extraction succeeds, leaving staging
    // data available for retries.
    let target = environment.join("flutter");
    if target.exists() {
        fs::remove_dir_all(&target).context("cannot remove previous Flutter SDK")?;
    }
    fs::rename(extract_dir.join("flutter"), &target).context("cannot install Flutter SDK")?;

    // A failed validation must never leave a stale ready marker behind.
    let _ = fs::remove_file(&ready);
    let output = Command::new(&flutter_bin)
        .arg("--version")
        .output()
        .context("cannot run installed flutter for validation")?;
    if !output.status.success() {
        bail!("Flutter validation failed");
    }
    let version_line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("Flutter ready")
        .to_owned();

    fs::create_dir_all(&state)?;
    fs::write(&recorded, format!("{}\n", release.version))?;
    fs::write(&ready, b"")?;
    println!("  ✓ {version_line}");
    Ok(release.version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const FIXTURE: &str = r#"{
        "current_release": {"stable": "abc123"},
        "releases": [
            {"hash": "abc123", "channel": "stable", "version": "3.38.0",
             "dart_sdk_arch": "x64", "archive": "flutter_linux_3.38.0-stable.tar.xz",
             "sha256": "deadbeef"},
            {"hash": "abc123", "channel": "stable", "version": "3.38.0",
             "dart_sdk_arch": "arm64", "archive": "flutter_linux-arm64_3.38.0-stable.tar.xz",
             "sha256": "cafe0001"},
            {"hash": "old999", "channel": "stable", "version": "3.35.0",
             "dart_sdk_arch": "x64", "archive": "flutter_linux_3.35.0-stable.tar.xz",
             "sha256": "cafe0002"}
        ]
    }"#;

    #[test]
    fn stable_resolves_per_arch() {
        let r = resolve_release(FIXTURE, "stable", "x64").unwrap();
        assert_eq!(r.version, "3.38.0");
        assert_eq!(r.archive, "flutter_linux_3.38.0-stable.tar.xz");

        let r = resolve_release(FIXTURE, "stable", "arm64").unwrap();
        assert_eq!(r.archive, "flutter_linux-arm64_3.38.0-stable.tar.xz");
    }

    #[test]
    fn pinned_version_resolves() {
        let r = resolve_release(FIXTURE, "3.35.0", "x64").unwrap();
        assert_eq!(r.archive, "flutter_linux_3.35.0-stable.tar.xz");
        // Optional leading v is accepted.
        let r = resolve_release(FIXTURE, "v3.35.0", "x64").unwrap();
        assert_eq!(r.version, "3.35.0");
    }

    #[test]
    fn unknown_version_fails() {
        assert!(resolve_release(FIXTURE, "9.9.9", "x64").is_err());
        assert!(resolve_release(FIXTURE, "stable", "riscv64").is_err());
        assert!(resolve_release("not json", "stable", "x64").is_err());
    }

    #[test]
    fn sha256_verify_ok_and_corrupt() {
        let dir = std::env::temp_dir();
        let path = dir.join("flenv-test-sha256.bin");
        fs::write(&path, b"hello").unwrap();

        let mut hasher = Sha256::new();
        hasher.update(b"hello");
        let good = hex::encode(hasher.finalize());
        verify_sha256(&path, &good).unwrap();
        verify_sha256(&path, &good.to_uppercase()).unwrap();
        assert!(verify_sha256(&path, &"0".repeat(64)).is_err());

        let mut f = File::options().append(true).open(&path).unwrap();
        f.write_all(b"corrupt").unwrap();
        drop(f);
        assert!(verify_sha256(&path, &good).is_err());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn host_arch_is_known() {
        assert!(matches!(host_arch().unwrap(), "x64" | "arm64"));
    }
}
