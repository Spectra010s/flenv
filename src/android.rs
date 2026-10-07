use crate::dl::download;
use crate::java;
use anyhow::{Context, Result, bail};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Pinned command-line-tools revision (bumped deliberately, never floating).
pub const CMDTOOLS_REV: &str = "11076708";
pub const DEFAULT_API: &str = "36";
pub const DEFAULT_BUILD_TOOLS: &str = "36.0.0";

/// Official Google command-line-tools URL for an OS key
/// (`linux`, `macos`, `windows` as in [`std::env::consts::OS`]).
pub fn cmdtools_url(os: &str) -> Result<String> {
    let infix = match os {
        "linux" => "linux",
        "macos" => "mac",
        "windows" => "win",
        _ => bail!("unsupported OS for Android SDK: {os}"),
    };
    Ok(format!(
        "https://dl.google.com/android/repository/commandlinetools-{infix}-{CMDTOOLS_REV}_latest.zip"
    ))
}

fn sdkmanager_name(os: &str) -> &'static str {
    if os == "windows" {
        "sdkmanager.bat"
    } else {
        "sdkmanager"
    }
}

/// Packages installed for every environment. AVD system images are
/// #40's job; this is the build toolchain only.
pub fn package_list(api: &str, build_tools: &str) -> Vec<String> {
    vec![
        "platform-tools".to_owned(),
        format!("platforms;android-{api}"),
        format!("build-tools;{build_tools}"),
        "emulator".to_owned(),
    ]
}

fn sdk_manager(sdk: &Path) -> PathBuf {
    sdk.join("cmdline-tools/latest/bin")
        .join(sdkmanager_name(std::env::consts::OS))
}

fn adb(sdk: &Path) -> PathBuf {
    let name = if std::env::consts::OS == "windows" {
        "adb.exe"
    } else {
        "adb"
    };
    sdk.join("platform-tools").join(name)
}

/// Extract a cmdline-tools zip into `$sdk/cmdline-tools/latest`.
/// The zip contains a top-level `cmdline-tools/` directory which must be
/// nested one level deeper (Google's required layout).
pub fn extract_cmdtools(zip_path: &Path, sdk: &Path) -> Result<()> {
    let staging = sdk.join("state-staging-cmdtools");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;
    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file).context("invalid cmdline-tools zip")?;
    archive
        .extract(&staging)
        .context("cannot extract cmdline-tools")?;

    let extracted = staging.join("cmdline-tools");
    if !extracted.join("bin/sdkmanager").is_file()
        && !extracted.join("bin/sdkmanager.bat").is_file()
    {
        bail!("cmdline-tools zip did not contain sdkmanager");
    }
    let target = sdk.join("cmdline-tools/latest");
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&extracted, &target).context("cannot install cmdline-tools")?;
    let _ = fs::remove_dir_all(&staging);
    Ok(())
}

fn run_with_log(cmd: &mut Command, log: &Path, what: &str) -> Result<()> {
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = cmd.output().with_context(|| format!("cannot run {what}"))?;
    let mut logged = std::mem::take(&mut output.stdout);
    logged.extend_from_slice(&output.stderr);
    fs::write(log, logged)?;
    if !output.status.success() {
        let tail = String::from_utf8_lossy(&output.stderr);
        let tail: String = tail
            .lines()
            .rev()
            .take(15)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        bail!("{what} failed (full log: {}):\n{tail}", log.display());
    }
    Ok(())
}

/// Pipe `y` answers into a license prompt until the process exits.
/// Portable alternative to `yes | sdkmanager --licenses`.
fn accept_licenses(sdkmanager: &Path, java_home: &Path, home: &Path, log: &Path) -> Result<()> {
    let mut child = Command::new(sdkmanager)
        .arg("--licenses")
        .env("JAVA_HOME", java_home)
        .env("HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("cannot run sdkmanager --licenses")?;
    {
        let mut stdin = child.stdin.take().context("cannot pipe to sdkmanager")?;
        // sdkmanager asks a bounded number of questions; keep answering
        // until it closes stdin (broken pipe ends the loop).
        for _ in 0..256 {
            if stdin.write_all(b"y\n").is_err() || stdin.flush().is_err() {
                break;
            }
        }
    }
    let output = child
        .wait_with_output()
        .context("cannot read sdkmanager output")?;
    let mut logged = output.stdout.clone();
    logged.extend_from_slice(&output.stderr);
    fs::write(log, logged)?;
    if !output.status.success() {
        bail!("sdkmanager --licenses failed (full log: {})", log.display());
    }
    Ok(())
}

/// Provision the Android SDK toolchain into `environment`.
///
/// Skips when `android.ready` plus the key binaries already exist.
/// The marker is written only after validation, so a failed run never
/// leaves a stale ready marker behind.
pub fn provision(environment: &Path, api: &str, build_tools: &str) -> Result<()> {
    let sdk = environment.join("android-sdk");
    let state = environment.join("state");
    let ready = state.join("android.ready");
    let manager = sdk_manager(&sdk);
    let adb_bin = adb(&sdk);

    if ready.is_file() && manager.is_file() && adb_bin.is_file() {
        println!("  ✓ Android SDK already provisioned");
        return Ok(());
    }

    let java_home = java::detect_java()?;
    eprintln!("  → Checking installed JDK");

    let staging = environment.join("state/staging/android");
    let log = staging.join("install.log");
    // Give SDK tools an environment-local HOME so Android state stays
    // inside the managed environment.
    let cli_home = state.join("android-home");
    fs::create_dir_all(&staging)?;
    fs::create_dir_all(&cli_home)?;
    fs::create_dir_all(&sdk)?;

    if !manager.is_file() {
        let url = cmdtools_url(std::env::consts::OS)?;
        let zip_path = staging.join("cmdtools.zip");
        eprintln!("  → Downloading Android command-line tools");
        download(&url, &zip_path, "Android command-line tools")?;
        eprintln!("  → Extracting command-line tools");
        extract_cmdtools(&zip_path, &sdk)?;
    }

    eprintln!("  → Accepting SDK licenses");
    accept_licenses(&manager, &java_home, &cli_home, &log)?;

    eprintln!("  → Installing SDK packages");
    let mut cmd = Command::new(&manager);
    cmd.arg(format!("--sdk_root={}", sdk.display()));
    for package in package_list(api, build_tools) {
        cmd.arg(package);
    }
    cmd.env("JAVA_HOME", &java_home).env("HOME", &cli_home);
    run_with_log(&mut cmd, &log, "Android SDK package installation")?;

    if !adb_bin.is_file() {
        bail!("Android platform-tools validation failed");
    }
    let status = Command::new(&adb_bin)
        .arg("version")
        .stdout(std::process::Stdio::null())
        .status()
        .context("cannot run adb for validation")?;
    if !status.success() {
        bail!("adb validation failed");
    }

    // A failed validation must never leave a stale ready marker behind.
    let _ = fs::remove_file(&ready);
    fs::create_dir_all(&state)?;
    fs::write(
        state.join("java-home"),
        format!("{}\n", java_home.display()),
    )?;
    fs::write(&ready, b"")?;
    println!("  ✓ Android SDK ready");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_per_os() {
        assert_eq!(
            cmdtools_url("linux").unwrap(),
            format!(
                "https://dl.google.com/android/repository/commandlinetools-linux-{CMDTOOLS_REV}_latest.zip"
            )
        );
        assert!(
            cmdtools_url("macos")
                .unwrap()
                .contains("commandlinetools-mac-")
        );
        assert!(
            cmdtools_url("windows")
                .unwrap()
                .contains("commandlinetools-win-")
        );
        assert!(cmdtools_url("freebsd").is_err());
        assert_eq!(sdkmanager_name("windows"), "sdkmanager.bat");
        assert_eq!(sdkmanager_name("linux"), "sdkmanager");
    }

    #[test]
    fn packages_cover_toolchain() {
        let pkgs = package_list("36", "36.0.0");
        for want in [
            "platform-tools",
            "platforms;android-36",
            "build-tools;36.0.0",
            "emulator",
        ] {
            assert!(pkgs.iter().any(|p| p == want), "missing {want}");
        }
    }

    /// Build a minimal cmdline-tools-shaped zip and check it lands in
    /// the required `cmdline-tools/latest` nesting.
    #[test]
    fn extract_nests_latest() {
        let dir = std::env::temp_dir().join("flenv-test-cmdtools");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("tools.zip");
        {
            let f = File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(f);
            zip.start_file(
                "cmdline-tools/bin/sdkmanager",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"#!/bin/sh\n").unwrap();
            zip.finish().unwrap();
        }
        let sdk = dir.join("sdk");
        extract_cmdtools(&zip_path, &sdk).unwrap();
        assert!(sdk.join("cmdline-tools/latest/bin/sdkmanager").is_file());
        let _ = fs::remove_dir_all(&dir);
    }
}
