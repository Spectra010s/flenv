use crate::{env, java};
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Newest entry of a versioned SDK dir (`platforms`, `build-tools`),
/// e.g. `android-36` or `36.0.0`. Sorts lexically; good enough for
/// Google's zero-padded numbering.
pub fn latest_dir(dir: &Path) -> Option<String> {
    fs::read_dir(dir).ok().and_then(|entries| {
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names.pop()
    })
}

/// `Android Debug Bridge version 1.0.41\nVersion 37.0.1-...` -> `37.0.1-...`.
pub fn parse_adb_version(output: &str) -> Option<String> {
    output
        .lines()
        .find_map(|l| l.strip_prefix("Version "))
        .map(|v| v.trim().to_owned())
}

/// `Dart SDK version: 3.13.3 (stable) ...` -> `3.13.3`.
pub fn parse_dart_version(output: &str) -> Option<String> {
    output
        .lines()
        .next()?
        .strip_prefix("Dart SDK version: ")?
        .split_whitespace()
        .next()
        .map(str::to_owned)
}

/// `{"frameworkVersion":"3.47.4",...}` -> `3.47.4`.
pub fn parse_flutter_version(output: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(output)
        .ok()?
        .get("frameworkVersion")?
        .as_str()
        .map(str::to_owned)
}

fn run_capture(bin: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(bin).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

/// KVM presence. Absence is a warning (slow emulator), never a failure.
pub fn kvm_present() -> bool {
    #[cfg(target_os = "linux")]
    return Path::new("/dev/kvm").exists();
    #[cfg(not(target_os = "linux"))]
    return true;
}

pub fn kvm_hint() -> &'static str {
    match std::env::consts::OS {
        "linux" => {
            "no /dev/kvm: emulator will be slow (enable nested virtualization or use a KVM host)"
        }
        "windows" => "ensure WHPX/Hyper-V is enabled for emulator acceleration",
        "macos" => "emulator uses HVF; no extra setup needed",
        _ => "hardware acceleration status unknown on this OS",
    }
}

/// Diagnose an environment. Returns `Ok` when ready, errors with
/// "Environment has problems." otherwise (exit 1 via main).
/// Every check runs even after failures so the report is complete.
pub fn run(home: &Path, name: Option<&str>, verbose: bool) -> Result<()> {
    let (env_name, path) = env::resolve_or_selected(home, name)?;
    let record = env::read_record(home, &env_name)?.context("environment not found")?;

    let mut failed = false;
    let mut pass = |ok: bool, label: String| {
        println!("  {} {label}", if ok { "✓" } else { "✗" });
        if !ok {
            failed = true;
        }
    };

    println!("Host");
    let host_ok = std::env::consts::OS == "linux";
    pass(
        host_ok,
        if host_ok {
            format!("Linux {}", std::env::consts::ARCH)
        } else {
            format!(
                "Unsupported host: {} (v0.1 targets Linux)",
                std::env::consts::OS
            )
        },
    );

    let java_file = path.join("state/java-home");
    let java_home = fs::read_to_string(&java_file)
        .ok()
        .map(|s| s.trim().to_owned());
    match java_home {
        Some(home) if Path::new(&home).join("bin/java").is_file() => {
            match java::major_version(&Path::new(&home).join("bin/java")) {
                Ok(v) => pass(true, format!("Java {v}")),
                Err(_) => pass(false, "Java (unreadable version)".to_owned()),
            }
        }
        _ => pass(false, "Java (run: flenv setup)".to_owned()),
    }

    println!("\nEnvironment: {env_name}");
    let flutter = path.join("flutter/bin/flutter");
    let dart = path.join("flutter/bin/dart");
    if flutter.is_file() {
        let version = run_capture(&flutter, &["--version", "--machine"])
            .and_then(|o| parse_flutter_version(&o))
            .unwrap_or_else(|| "installed".to_owned());
        pass(true, format!("Flutter {version}"));
    } else {
        pass(false, "Flutter".to_owned());
    }
    if dart.is_file() {
        let version = run_capture(&dart, &["--version"])
            .and_then(|o| parse_dart_version(&o))
            .unwrap_or_else(|| "installed".to_owned());
        pass(true, format!("Dart {version}"));
    } else {
        pass(false, "Dart".to_owned());
    }

    let sdk = path.join("android-sdk");
    pass(sdk.is_dir(), "Android SDK".to_owned());
    match latest_dir(&sdk.join("platforms")) {
        Some(p) => pass(
            true,
            format!("Android API {}", p.strip_prefix("android-").unwrap_or(&p)),
        ),
        None => pass(false, "Android API".to_owned()),
    }
    match latest_dir(&sdk.join("build-tools")) {
        Some(b) => pass(true, format!("Build Tools {b}")),
        None => pass(false, "Build Tools".to_owned()),
    }
    let adb_bin = sdk.join("platform-tools/adb");
    if adb_bin.is_file() {
        let version = run_capture(&adb_bin, &["version"])
            .and_then(|o| parse_adb_version(&o))
            .unwrap_or_else(|| "installed".to_owned());
        pass(true, format!("ADB {version}"));
    } else {
        pass(false, "ADB".to_owned());
    }

    if record.isolated {
        pass(path.join("workspace").is_dir(), "Workspace".to_owned());
        pass(
            path.join("cache/pub").is_dir() && path.join("cache/gradle").is_dir(),
            "Isolated caches".to_owned(),
        );
    }

    // Provisioning writes this marker only after full validation.
    pass(
        path.join("state/environment.ready").is_file()
            || path.join("state/flutter.ready").is_file(),
        "Flutter Android toolchain".to_owned(),
    );

    if !kvm_present() {
        println!("  → {}", kvm_hint());
    }

    if verbose {
        println!("\nPaths");
        println!("  FLENV_HOME: {}", home.display());
        println!("  Environment: {}", path.display());
        println!("  FLUTTER_ROOT: {}/flutter", path.display());
        println!("  ANDROID_HOME: {}", sdk.display());
        println!(
            "  JAVA_HOME: {}",
            java_file
                .exists()
                .then(|| fs::read_to_string(&java_file).unwrap_or_default())
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "missing".to_owned())
        );
        println!("  Workspace: {}/workspace", path.display());
        if record.isolated {
            println!("  PUB_CACHE: {}/cache/pub", path.display());
            println!("  GRADLE_USER_HOME: {}/cache/gradle", path.display());
        }
        println!("\nState");
        println!("  Isolated: {}", record.isolated);
        let state = |marker: &str| {
            if path.join(format!("state/{marker}.ready")).is_file() {
                "ready"
            } else {
                "incomplete"
            }
        };
        println!("  Flutter: {}", state("flutter"));
        println!("  Android: {}", state("android"));
        println!(
            "  Environment: {}",
            if path.join("state/environment.ready").is_file() {
                "ready"
            } else {
                "incomplete"
            }
        );
    }

    println!();
    if failed {
        bail!("Environment has problems.");
    }
    println!("Environment is ready.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tool_versions() {
        assert_eq!(
            parse_adb_version("Android Debug Bridge version 1.0.41\nVersion 37.0.1-15733141\n"),
            Some("37.0.1-15733141".to_owned())
        );
        assert_eq!(parse_adb_version("garbage"), None);
        assert_eq!(
            parse_dart_version("Dart SDK version: 3.13.3 (stable)"),
            Some("3.13.3".to_owned())
        );
        assert_eq!(parse_dart_version("nope"), None);
        assert_eq!(
            parse_flutter_version(r#"{"frameworkVersion":"3.47.4"}"#),
            Some("3.47.4".to_owned())
        );
        assert_eq!(parse_flutter_version("not json"), None);
    }

    #[test]
    fn latest_dir_picks_newest() {
        let dir = std::env::temp_dir().join("flenv-test-latest");
        let _ = fs::remove_dir_all(&dir);
        for d in ["android-34", "android-36", "android-35"] {
            fs::create_dir_all(dir.join(d)).unwrap();
        }
        // A stray file must not win over directories.
        fs::write(dir.join("android-99"), b"x").unwrap();
        assert_eq!(latest_dir(&dir), Some("android-36".to_owned()));
        assert_eq!(latest_dir(&dir.join("missing")), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn kvm_hint_exists_everywhere() {
        assert!(!kvm_hint().is_empty());
    }
}
