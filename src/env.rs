use anyhow::{Context, Result, bail};
use dotcfg::DotCfg;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Lightweight metadata for one environment. The heavy SDKs live at
/// `path`, possibly on another filesystem; the record itself stays small.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record {
    pub path: PathBuf,
    pub isolated: bool,
}

/// Resolve the metadata home. Explicit `FLENV_HOME` wins; otherwise the
/// conventional dot-dir. `dotcfg`'s default strategy is the same dot-dir,
/// but the env override stays explicit here.
pub fn flenv_home() -> Result<PathBuf> {
    if let Ok(home) = std::env::var("FLENV_HOME")
        && !home.is_empty()
    {
        return Ok(PathBuf::from(home));
    }
    match std::env::var("HOME") {
        Ok(h) => Ok(PathBuf::from(h).join(".flenv")),
        Err(_) => bail!("HOME must be set (or set FLENV_HOME)"),
    }
}

/// Names become record filenames and path components, so separators
/// and traversal are rejected.
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name == "." || name == ".." {
        bail!("invalid environment name: {name}");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        bail!("invalid environment name: {name}");
    }
    Ok(())
}

/// Resolve the environment directory. External roots are storage bases;
/// flenv keeps its own namespace beneath them — never the raw `--root`.
pub fn environment_path(name: &str, root: Option<&Path>, home: &Path) -> Result<PathBuf> {
    validate_name(name)?;
    if let Some(root) = root {
        if !root.is_dir() {
            bail!("root is not a directory: {}", root.display());
        }
        Ok(root.join("flenv/environments").join(name))
    } else {
        Ok(home.join("environments").join(name))
    }
}

fn record_cfg(home: &Path, name: &str) -> DotCfg {
    DotCfg::new("flenv")
        .at_dir(home.join("records"))
        .filename(name)
}

pub fn write_record(home: &Path, name: &str, path: &Path, isolated: bool) -> Result<()> {
    validate_name(name)?;
    fs::create_dir_all(home.join("records"))
        .with_context(|| format!("cannot create {}", home.display()))?;
    record_cfg(home, name)
        .save(&Record {
            path: path.to_owned(),
            isolated,
        })
        .map_err(|e| anyhow::anyhow!("cannot write environment record: {e}"))?;
    Ok(())
}

pub fn read_record(home: &Path, name: &str) -> Result<Option<Record>> {
    validate_name(name)?;
    record_cfg(home, name)
        .load()
        .map_err(|e| anyhow::anyhow!("cannot read environment record: {e}"))
}

/// Resolve a name to a live environment directory.
/// Unknown names fail as "not found"; stale records whose directory
/// vanished fail as "missing" — never activate broken paths.
pub fn resolve(home: &Path, name: &str) -> Result<PathBuf> {
    validate_name(name)?;
    let record =
        read_record(home, name)?.with_context(|| format!("environment not found: {name}"))?;
    if !record.path.is_dir() {
        bail!("environment is missing: {name}");
    }
    Ok(record.path)
}

fn selected_file(home: &Path) -> PathBuf {
    home.join("selected")
}

pub fn selected_name(home: &Path) -> Result<Option<String>> {
    let file = selected_file(home);
    if !file.is_file() {
        return Ok(None);
    }
    let name = fs::read_to_string(&file)?.trim().to_owned();
    Ok(if name.is_empty() { None } else { Some(name) })
}

/// Resolve `--name` or fall back to the selected environment, returning
/// both the name and its live path.
pub fn resolve_or_selected(home: &Path, name: Option<&str>) -> Result<(String, PathBuf)> {
    match name {
        Some(n) => Ok((n.to_owned(), resolve(home, n)?)),
        None => {
            let selected = selected_name(home)?.with_context(|| {
                "no environment selected; use --name NAME or run: flenv use NAME".to_owned()
            })?;
            Ok((selected.clone(), resolve(home, &selected)?))
        }
    }
}

pub fn set_selected(home: &Path, name: &str) -> Result<PathBuf> {
    let path = resolve(home, name)?;
    fs::create_dir_all(home)?;
    fs::write(selected_file(home), format!("{name}\n"))
        .context("cannot save selected environment")?;
    Ok(path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ready,
    Incomplete,
    Missing,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Ready => write!(f, "ready"),
            Status::Incomplete => write!(f, "incomplete"),
            Status::Missing => write!(f, "missing"),
        }
    }
}

/// Provisioning writes `environment.ready` only after validation, so its
/// absence means incomplete — not a separate state to track.
pub fn status(home: &Path, name: &str) -> Status {
    let Ok(Some(record)) = read_record(home, name) else {
        return Status::Missing;
    };
    if !record.path.is_dir() {
        return Status::Missing;
    }
    if record.path.join("state/environment.ready").is_file()
        || record.path.join("state/flutter.ready").is_file()
    {
        Status::Ready
    } else {
        Status::Incomplete
    }
}

pub struct EnvEntry {
    pub name: String,
    pub record: Option<Record>,
    pub status: Status,
    pub selected: bool,
}

/// All known environments: every record file plus staleness detection.
/// Keeps stale records visible instead of hiding them.
pub fn list(home: &Path) -> Result<Vec<EnvEntry>> {
    let selected = selected_name(home)?.unwrap_or_default();
    let mut entries = Vec::new();
    let dir = home.join("records");
    if !dir.is_dir() {
        return Ok(entries);
    }
    let mut names: Vec<String> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter_map(|f| f.strip_suffix(".toml").map(str::to_owned))
        .collect();
    names.sort();
    for name in names {
        let record = read_record(home, &name)?.filter(|_| validate_name(&name).is_ok());
        entries.push(EnvEntry {
            status: status(home, &name),
            selected: name == selected,
            name,
            record,
        });
    }
    Ok(entries)
}

// --- shell activation -----------------------------------------------------

/// Quote a value for POSIX shell: single-quote, escaping embedded quotes.
/// `flenv env` prints exports only, so quoting must be airtight.
pub fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[derive(Debug, Default)]
pub struct HostSnapshot {
    /// Current values; `None` means unset.
    pub vars: HashMap<String, Option<String>>,
    pub active_root: Option<String>,
    pub path: String,
}

impl HostSnapshot {
    pub fn capture() -> Self {
        let mut keys = vec!["JAVA_HOME", "PUB_CACHE", "GRADLE_USER_HOME"];
        keys.extend([
            "FLENV_SAVED_JAVA_HOME",
            "FLENV_SAVED_PUB_CACHE",
            "FLENV_SAVED_GRADLE_USER_HOME",
        ]);
        let vars = keys
            .into_iter()
            .map(|k| (k.to_owned(), std::env::var(k).ok()))
            .collect();
        Self {
            vars,
            active_root: std::env::var("FLENV_ACTIVE_ROOT").ok(),
            path: std::env::var("PATH").unwrap_or_default(),
        }
    }
}

/// Render shell activation: restore-then-apply exports plus PATH surgery.
/// Only paths added by a previous activation are removed; unrelated
/// entries survive. Emits exports only — human text would break `eval`.
pub fn render_activation(
    env_path: &Path,
    name: &str,
    isolated: bool,
    java_home: Option<&Path>,
    host: &HostSnapshot,
) -> String {
    let mut out = String::new();
    let env = env_path.to_string_lossy();

    for var in ["JAVA_HOME", "PUB_CACHE", "GRADLE_USER_HOME"] {
        let saved = format!("FLENV_SAVED_{var}");
        if host.active_root.is_none() {
            match host.vars.get(var).and_then(|v| v.as_deref()) {
                Some(current) => out.push_str(&format!("export {saved}={}\n", sh_quote(current))),
                None => out.push_str(&format!("unset {saved}\n")),
            }
        } else if let Some(saved_val) = host.vars.get(&saved).and_then(|v| v.as_deref()) {
            out.push_str(&format!("export {var}={}\n", sh_quote(saved_val)));
        } else {
            out.push_str(&format!("unset {var}\n"));
        }
    }

    // Remove only paths added by the previous activation, keeping other edits.
    let managed = host.active_root.as_deref().map(|root| {
        [
            format!("{root}/flutter/bin"),
            format!("{root}/android-sdk/platform-tools"),
            format!("{root}/android-sdk/cmdline-tools/latest/bin"),
        ]
    });
    let mut paths = vec![
        format!("{env}/flutter/bin"),
        format!("{env}/android-sdk/platform-tools"),
        format!("{env}/android-sdk/cmdline-tools/latest/bin"),
    ];
    for entry in host.path.split(':') {
        if entry.is_empty() {
            continue;
        }
        if managed
            .as_ref()
            .is_some_and(|m| m.iter().any(|p| p == entry))
        {
            continue;
        }
        paths.push(entry.to_owned());
    }

    out.push_str(&format!("export FLENV_ACTIVE_ROOT={}\n", sh_quote(&env)));
    out.push_str(&format!("export FLENV_ENV={}\n", sh_quote(name)));
    out.push_str(&format!(
        "export FLUTTER_ROOT={}\n",
        sh_quote(&format!("{env}/flutter"))
    ));
    out.push_str(&format!(
        "export ANDROID_HOME={}\n",
        sh_quote(&format!("{env}/android-sdk"))
    ));
    out.push_str(&format!(
        "export ANDROID_SDK_ROOT={}\n",
        sh_quote(&format!("{env}/android-sdk"))
    ));
    if let Some(java) = java_home {
        out.push_str(&format!(
            "export JAVA_HOME={}\n",
            sh_quote(&java.to_string_lossy())
        ));
    }
    if isolated {
        out.push_str(&format!(
            "export PUB_CACHE={}\n",
            sh_quote(&format!("{env}/cache/pub"))
        ));
        out.push_str(&format!(
            "export GRADLE_USER_HOME={}\n",
            sh_quote(&format!("{env}/cache/gradle"))
        ));
    }
    out.push_str(&format!("export PATH={}\n", sh_quote(&paths.join(":"))));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("flenv-test-{tag}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn names_reject_traversal() {
        assert!(validate_name("stable").is_ok());
        assert!(validate_name("a.b_c-d").is_ok());
        for bad in [
            "",
            ".",
            "..",
            "../escape",
            "bad/name",
            "has space",
            "semi;colon",
        ] {
            assert!(validate_name(bad).is_err(), "accepted {bad}");
        }
    }

    #[test]
    fn root_is_a_namespace_not_the_path() {
        let home = tmp_home("ns");
        let base = tmp_home("base");
        let p = environment_path("stable", Some(&base), &home).unwrap();
        assert_eq!(p, base.join("flenv/environments/stable"));
        let p = environment_path("stable", None, &home).unwrap();
        assert_eq!(p, home.join("environments/stable"));
        assert!(environment_path("../x", None, &home).is_err());
    }

    #[test]
    fn records_round_trip_and_stale() {
        let home = tmp_home("records");
        let live = tmp_home("live-env");
        write_record(&home, "live", &live, true).unwrap();
        write_record(&home, "stale", &tmp_home("gone").join("nope"), false).unwrap();

        assert_eq!(resolve(&home, "live").unwrap(), live);
        assert!(
            resolve(&home, "stale")
                .unwrap_err()
                .to_string()
                .contains("missing")
        );
        assert!(
            resolve(&home, "nope")
                .unwrap_err()
                .to_string()
                .contains("not found")
        );

        let entries = list(&home).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .any(|e| e.name == "live" && e.status == Status::Incomplete)
        );
        assert!(
            entries
                .iter()
                .any(|e| e.name == "stale" && e.status == Status::Missing)
        );
    }

    #[test]
    fn activation_switches_and_restores() {
        let host = HostSnapshot {
            vars: [("JAVA_HOME", Some("/host java".to_owned()))]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
            active_root: None,
            path: "/usr/bin:/bin".to_owned(),
        };
        let out = render_activation(
            Path::new("/mnt/flenv/environments/x"),
            "x",
            true,
            None,
            &host,
        );
        assert!(out.contains("export FLENV_SAVED_JAVA_HOME='/host java'"));
        assert!(out.contains("unset FLENV_SAVED_PUB_CACHE"));
        assert!(out.contains("PUB_CACHE='/mnt/flenv/environments/x/cache/pub'"));
        assert!(out.contains("FLUTTER_ROOT='/mnt/flenv/environments/x/flutter'"));
        // No stdout text besides exports.
        assert!(
            out.lines()
                .all(|l| l.starts_with("export ") || l.starts_with("unset "))
        );

        // Switching away restores the saved host value and drops managed PATH.
        let host2 = HostSnapshot {
            vars: [("FLENV_SAVED_JAVA_HOME", Some("/host java".to_owned()))]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
            active_root: Some("/mnt/flenv/environments/x".to_owned()),
            path: "/other/flutter/bin:/mnt/flenv/environments/x/flutter/bin:/usr/bin".to_owned(),
        };
        let out2 = render_activation(
            Path::new("/home/.flenv/environments/y"),
            "y",
            false,
            None,
            &host2,
        );
        assert!(out2.contains("export JAVA_HOME='/host java'"));
        assert!(!out2.contains("/mnt/flenv/environments/x"));
        assert!(out2.contains("/other/flutter/bin"));
        assert!(!out2.contains("PUB_CACHE="));
    }

    #[test]
    fn quoting_is_airtight() {
        assert_eq!(sh_quote("a b"), "'a b'");
        assert_eq!(sh_quote("o'clock"), "'o'\\''clock'");
        assert_eq!(sh_quote("$(rm -rf ~)"), "'$(rm -rf ~)'");
    }
}
