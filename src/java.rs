use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MIN_VERSION: u32 = 17;

/// Parse the major version from `java -version` output, e.g.
/// `openjdk version "21.0.1" ...` -> 21.
pub fn major_version(java_bin: &Path) -> Result<u32> {
    let output = Command::new(java_bin)
        .arg("-version")
        .output()
        .with_context(|| format!("cannot run {}", java_bin.display()))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_major(&text)
        .with_context(|| format!("cannot determine Java version from: {}", java_bin.display()))
}

fn parse_major(text: &str) -> Result<u32> {
    let first = text.lines().next().unwrap_or("");
    // Match: version "21.0.1" (also 1.8-style, though those fail the minimum).
    let digits: String = first
        .split("version \"")
        .nth(1)
        .unwrap_or("")
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let major = digits
        .split('.')
        .next()
        .unwrap_or("")
        .parse::<u32>()
        .map_err(|_| anyhow::anyhow!("no version number in: {first}"))?;
    Ok(major)
}

/// Resolve a `java` binary back to its JDK home, following symlinks so
/// PATH-provided binaries lead to the real home.
pub fn java_home(java_bin: &Path) -> Result<PathBuf> {
    let resolved = std::fs::canonicalize(java_bin).unwrap_or_else(|_| java_bin.to_owned());
    resolved
        .parent()
        .and_then(|bin| bin.parent())
        .map(|h| h.to_owned())
        .context("cannot determine Java home")
}

/// Find a usable JDK: explicit `JAVA_HOME` first, then host `PATH`.
/// Rejects anything older than [`MIN_VERSION`].
pub fn detect_java() -> Result<PathBuf> {
    let java_bin = match std::env::var("JAVA_HOME") {
        Ok(home) => {
            let bin = PathBuf::from(&home).join("bin/java");
            if bin.is_file() {
                bin
            } else {
                bail!("JAVA_HOME is set but has no bin/java: {home}");
            }
        }
        Err(_) => which_java().context(format!(
            "Java {MIN_VERSION} or newer is required; no Java installation was found"
        ))?,
    };

    let version = major_version(&java_bin)?;
    if version < MIN_VERSION {
        bail!(
            "Java {MIN_VERSION} or newer is required; found Java {version} at {}",
            java_bin.display()
        );
    }
    let home = java_home(&java_bin)?;
    if !home.join("bin/java").is_file() {
        bail!("invalid Java home: {}", home.display());
    }
    Ok(home)
}

fn which_java() -> Result<PathBuf> {
    let path = std::env::var("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        for name in ["java", "java.exe"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    bail!("java not found on PATH")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn parses_modern_versions() {
        assert_eq!(
            parse_major("openjdk version \"21.0.1\" 2023-10-17").unwrap(),
            21
        );
        assert_eq!(
            parse_major("openjdk version \"17.0.9\" 2024-01-16").unwrap(),
            17
        );
        assert!(parse_major("garbage output").is_err());
    }

    /// Fake a JDK on PATH: detect_java must prefer explicit JAVA_HOME,
    /// resolve through the symlink, and reject old versions.
    #[test]
    fn detect_prefers_java_home_and_rejects_old() {
        let dir = std::env::temp_dir().join("flenv-test-jdk");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("bin")).unwrap();
        let java = dir.join("bin/java");
        fs::write(&java, "#!/bin/sh\necho 'openjdk version \"21.0.1\"' >&2\n").unwrap();
        fs::set_permissions(&java, fs::Permissions::from_mode(0o755)).unwrap();

        // Safety: these tests run in one process; restore afterwards.
        let saved = std::env::var("JAVA_HOME").ok();
        unsafe { std::env::set_var("JAVA_HOME", &dir) };
        let home = detect_java().unwrap();
        assert_eq!(home, dir);

        fs::write(&java, "#!/bin/sh\necho 'openjdk version \"11.0.24\"' >&2\n").unwrap();
        assert!(detect_java().is_err());

        match saved {
            Some(v) => unsafe { std::env::set_var("JAVA_HOME", v) },
            None => unsafe { std::env::remove_var("JAVA_HOME") },
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
