use crate::env;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

/// One line of `adb devices` output.
#[derive(Debug, PartialEq, Eq)]
pub struct Device {
    pub serial: String,
    pub state: String,
}

/// Parse `adb devices` output: skip the header and daemon noise, then
/// split each line into serial + state on any whitespace.
pub fn parse_devices(output: &str) -> Vec<Device> {
    output
        .lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with("List of devices")
                && !l.starts_with('*')
                && !l.starts_with("adb ")
        })
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let serial = parts.next()?;
            let state = parts.next()?;
            Some(Device {
                serial: serial.to_owned(),
                state: state.to_owned(),
            })
        })
        .collect()
}

/// Only devices actually usable by `adb`; `offline` and `unauthorized`
/// are running but not ready yet.
pub fn ready(devices: &[Device]) -> Vec<&Device> {
    devices.iter().filter(|d| d.state == "device").collect()
}

pub fn ssh_template(target: &str) -> String {
    format!("ssh -L 6080:localhost:6080 -L 5900:localhost:5900 {target}")
}

/// Default SSH target for the tunnel, e.g. `user@myhost`.
pub fn default_ssh_target() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_owned());
    let host = std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "host".to_owned());
    format!("{user}@{host}")
}

pub(crate) fn adb_path(env_path: &Path) -> Result<PathBuf> {
    let adb = env_path.join("android-sdk/platform-tools/adb");
    if !adb.is_file() {
        bail!("ADB not installed; run: flenv setup");
    }
    Ok(adb)
}

/// Run adb with the environment's SDK vars, returning stdout.
pub(crate) fn adb_run(env_path: &Path, args: &[&str]) -> Result<String> {
    let adb = adb_path(env_path)?;
    let sdk = env_path.join("android-sdk");
    let output = Command::new(&adb)
        .args(args)
        .env("ANDROID_SDK_ROOT", &sdk)
        .env("ANDROID_HOME", &sdk)
        .output()
        .with_context(|| format!("cannot run adb {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "adb {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn adb_devices(env_path: &Path) -> Result<Vec<Device>> {
    Ok(parse_devices(&adb_run(env_path, &["devices"])?))
}

/// Start command to suggest when nothing is running. Uses a real AVD
/// when the environment has one, otherwise the default name so the
/// line still works after `flenv emulator create`.
pub fn start_hint(env_path: &Path) -> String {
    let emulator = env_path
        .join("android-sdk/emulator/emulator")
        .display()
        .to_string();
    let avd = crate::emulator::list_avds(env_path)
        .ok()
        .and_then(|avds| avds.into_iter().next())
        .unwrap_or_else(|| "Pixel_API_36".to_owned());
    format!("{emulator} -avd {avd}")
}

/// Shared viewer output for both backends.
pub fn print_viewer(env_name: &str, devices: &[&Device], ssh: Option<&str>) {
    println!("Environment: {env_name}");
    for device in devices {
        println!("  ✓ {} ({})", device.serial, device.state);
    }
    let target = ssh.map(str::to_owned).unwrap_or_else(default_ssh_target);
    println!();
    println!("noVNC:  http://localhost:6080/vnc.html?autoconnect=1&resize=remote");
    println!("VNC:    localhost:5900");
    println!("SSH:    {}", ssh_template(&target));
    println!();
    println!("Ports assume a noVNC/VNC viewer is running in the environment.");
}

/// Backend 1 (`auto`/`adb`): the emulator runs on this host (or a remote
/// KVM host) and a viewer exposes noVNC/VNC. Backend 2 (`redroid`) runs
/// Android in Docker and reuses the same viewer output.
pub fn run(
    home: &Path,
    name: Option<&str>,
    backend: &str,
    ssh: Option<&str>,
    image: &str,
    port: u16,
) -> Result<()> {
    match backend {
        "auto" | "adb" => {}
        "redroid" => {
            let (env_name, path) = env::resolve_or_selected(home, name)?;
            return crate::redroid::run(&env_name, &path, image, port, ssh);
        }
        other => bail!("unknown backend: {other} (expected auto, adb, or redroid)"),
    }

    let (env_name, path) = env::resolve_or_selected(home, name)?;
    let devices = adb_devices(&path)?;
    let ready = ready(&devices);

    if ready.is_empty() {
        if devices.is_empty() {
            eprintln!("No devices found for {env_name}.");
        } else {
            eprintln!("Devices are not ready for {env_name}:");
            for device in &devices {
                eprintln!("  {} ({})", device.serial, device.state);
            }
            eprintln!("Wait for boot to finish, or check: adb devices");
        }
        eprintln!("\nStart an emulator with:");
        eprintln!("  flenv emulator start");
        eprintln!("or directly:");
        eprintln!("  {}", start_hint(&path));
        bail!("no ready devices");
    }

    print_viewer(&env_name, &ready, ssh);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_devices_and_skips_noise() {
        let out = "* daemon not running; starting now at tcp:5037\n\
                   * daemon started successfully\n\
                   List of devices attached\n\
                   emulator-5554\tdevice\n\
                   emulator-5556\toffline\n\
                   192.168.1.5:5555\tunauthorized\n\n";
        let devices = parse_devices(out);
        assert_eq!(devices.len(), 3);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[0].state, "device");
        assert_eq!(devices[1].state, "offline");
        assert_eq!(devices[2].state, "unauthorized");
    }

    #[test]
    fn only_device_state_is_ready() {
        let devices = vec![
            Device {
                serial: "emulator-5554".to_owned(),
                state: "device".to_owned(),
            },
            Device {
                serial: "emulator-5556".to_owned(),
                state: "offline".to_owned(),
            },
        ];
        let ready_devices = ready(&devices);
        assert_eq!(ready_devices.len(), 1);
        assert_eq!(ready_devices[0].serial, "emulator-5554");
        assert!(ready(&[]).is_empty());
    }

    #[test]
    fn parses_empty_output() {
        assert!(parse_devices("List of devices attached\n").is_empty());
        assert!(parse_devices("").is_empty());
    }

    #[test]
    fn ssh_template_forwards_viewer_ports() {
        let line = ssh_template("ubuntu@kvm-box");
        assert!(line.starts_with("ssh "));
        assert!(line.contains("-L 6080:localhost:6080"));
        assert!(line.contains("-L 5900:localhost:5900"));
        assert!(line.ends_with("ubuntu@kvm-box"));
    }

    #[test]
    fn backend_validation() {
        let home = std::env::temp_dir();
        // No environment needed: the backend is rejected before lookup.
        // Unknown backends fail before any environment lookup.
        assert!(run(&home, Some("missing-env"), "bogus", None, "", 5555).is_err());
        // redroid resolves the environment first and fails cleanly.
        assert!(run(&home, Some("missing-env"), "redroid", None, "", 5555).is_err());
    }
}
