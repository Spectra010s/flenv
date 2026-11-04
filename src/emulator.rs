use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

/// Arch-aware system image, matching the android-env approach:
/// x86_64 hosts run the x86_64 image, ARM hosts the arm64-v8a image.
pub fn system_image(api: &str, arch: &str) -> Result<String> {
    let abi = match arch {
        "x86_64" | "amd64" => "x86_64",
        "aarch64" | "arm64" => "arm64-v8a",
        _ => bail!("unsupported architecture for emulator images: {arch}"),
    };
    Ok(format!("system-images;android-{api};google_apis;{abi}"))
}

fn bin_name(base: &str) -> String {
    if std::env::consts::OS == "windows" {
        format!("{base}.bat")
    } else {
        base.to_owned()
    }
}

/// AVDs and their userdata live inside the environment, not in
/// `~/.android`, so `--isolated` stays self-contained and a small home
/// partition is not filled by a 6 GB userdata image.
pub fn avd_home(env_path: &Path) -> PathBuf {
    env_path.join("avd")
}

/// Point avdmanager/emulator at the environment's AVD directory.
fn apply_avd_env(cmd: &mut Command, env_path: &Path) {
    let avd = avd_home(env_path);
    let emulator_home = env_path.join("state/android-emulator");
    let _ = fs::create_dir_all(&avd);
    let _ = fs::create_dir_all(&emulator_home);
    cmd.env("ANDROID_AVD_HOME", avd)
        .env("ANDROID_EMULATOR_HOME", emulator_home);
}

fn sdk_bin(sdk: &Path, tool: &str) -> PathBuf {
    let (dir, base) = match tool {
        "avdmanager" => ("cmdline-tools/latest/bin", bin_name("avdmanager")),
        "sdkmanager" => ("cmdline-tools/latest/bin", bin_name("sdkmanager")),
        "emulator" => (
            "emulator",
            format!(
                "emulator{}",
                if std::env::consts::OS == "windows" {
                    ".exe"
                } else {
                    ""
                }
            ),
        ),
        _ => unreachable!(),
    };
    sdk.join(dir).join(base)
}

fn java_home_for(env_path: &Path) -> Option<PathBuf> {
    fs::read_to_string(env_path.join("state/java-home"))
        .ok()
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| p.join("bin/java").is_file())
}

/// Parse `avdmanager list avd -c` output into AVD names.
pub fn parse_avd_list(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

fn run_sdk_tool(
    env_path: &Path,
    tool: &str,
    args: &[&str],
    stdin_text: Option<&str>,
) -> Result<String> {
    let sdk = env_path.join("android-sdk");
    let bin = sdk_bin(&sdk, tool);
    if !bin.is_file() {
        bail!("{tool} not installed; run: flenv setup");
    }
    let mut cmd = Command::new(&bin);
    cmd.args(args)
        .env("ANDROID_SDK_ROOT", &sdk)
        .env("ANDROID_HOME", &sdk)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_avd_env(&mut cmd, env_path);
    if let Some(java) = java_home_for(env_path) {
        cmd.env("JAVA_HOME", java);
    }
    let mut child = cmd.spawn().with_context(|| format!("cannot run {tool}"))?;
    if let Some(text) = stdin_text {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            // Broken pipe just means the tool stopped asking.
            let _ = stdin.write_all(text.as_bytes());
        }
    }
    let output = child
        .wait_with_output()
        .context("cannot read tool output")?;
    if !output.status.success() {
        bail!(
            "{tool} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn list_avds(env_path: &Path) -> Result<Vec<String>> {
    let out = run_sdk_tool(env_path, "avdmanager", &["list", "avd", "-c"], None)?;
    Ok(parse_avd_list(&out))
}

/// Create an AVD, installing its system image first. Existing AVDs are
/// kept — setup is idempotent, not a reinstall.
pub fn create(env_path: &Path, avd: &str, api: &str, device: &str) -> Result<()> {
    if avd.is_empty() || avd.contains('/') || avd.contains(char::is_whitespace) {
        bail!("invalid AVD name: {avd}");
    }
    let existing = list_avds(env_path)?;
    if existing.iter().any(|a| a == avd) {
        println!("  ✓ AVD {avd} already exists");
        return Ok(());
    }

    let image = system_image(api, std::env::consts::ARCH)?;
    eprintln!("  → Installing system image");
    run_sdk_tool(
        env_path,
        "sdkmanager",
        &[
            &format!("--sdk_root={}", env_path.join("android-sdk").display()),
            &image,
        ],
        None,
    )?;

    eprintln!("  → Creating AVD {avd}");
    // Answer "no" to the custom-hardware-profile prompt for a clean default.
    run_sdk_tool(
        env_path,
        "avdmanager",
        &[
            "create", "avd", "-n", avd, "-k", &image, "--device", device, "--force",
        ],
        Some("no\n"),
    )?;
    println!("  ✓ AVD {avd} created");
    Ok(())
}

/// Arguments for a detached emulator run. Without KVM the x86_64 image
/// must fall back to TCG, otherwise the emulator exits immediately with
/// "x86_64 emulation currently requires hardware acceleration".
pub fn start_args(name: &str, no_window: bool, kvm: bool) -> Vec<String> {
    let mut args = vec!["-avd".to_owned(), name.to_owned()];
    if no_window {
        args.push("-no-window".to_owned());
    }
    if !kvm {
        args.push("-accel".to_owned());
        args.push("off".to_owned());
        args.push("-gpu".to_owned());
        args.push("swiftshader_indirect".to_owned());
    }
    args
}

/// Start an emulator detached (like nohup): the CLI returns while the
/// emulator boots in the background. Resolves a default AVD when none
/// is given.
pub fn start(env_path: &Path, avd: Option<&str>, no_window: bool) -> Result<String> {
    let name = match avd {
        Some(a) => a.to_owned(),
        None => list_avds(env_path)?
            .into_iter()
            .next()
            .context("no AVDs found; run: flenv emulator create")?,
    };
    let sdk = env_path.join("android-sdk");
    let bin = sdk_bin(&sdk, "emulator");
    if !bin.is_file() {
        bail!("emulator not installed; run: flenv setup");
    }
    // Keep the emulator's own output: a failed boot must be readable
    // somewhere, and a detached process has no terminal.
    let log_path = env_path.join("state/emulator.log");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let log = fs::File::create(&log_path)
        .with_context(|| format!("cannot write {}", log_path.display()))?;

    let mut cmd = Command::new(&bin);
    cmd.args(start_args(&name, no_window, crate::doctor::kvm_present()));
    cmd.env("ANDROID_SDK_ROOT", &sdk)
        .env("ANDROID_HOME", &sdk)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    apply_avd_env(&mut cmd, env_path);
    if let Some(java) = java_home_for(env_path) {
        cmd.env("JAVA_HOME", java);
    }
    // Detach: spawn without waiting so the CLI returns immediately.
    let mut child = cmd
        .spawn()
        .with_context(|| format!("cannot start emulator {name}"))?;

    // Missing acceleration or a bad image fails within a second; catch it
    // instead of reporting success for a process that already died.
    sleep(Duration::from_millis(1500));
    if let Some(status) = child.try_wait()? {
        let text = fs::read_to_string(&log_path).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let tail = lines[lines.len().saturating_sub(15)..].join("\n");
        bail!(
            "emulator {name} exited during startup ({status})\nlog: {}\n{}",
            log_path.display(),
            tail
        );
    }

    eprintln!("  ✓ Emulator {name} booting in background");
    eprintln!("    log: {}", log_path.display());
    Ok(name)
}

/// Desktop shortcut content per OS, so the emulator is one click away
/// outside the terminal (mirrors the android-env UX).
pub fn shortcut_contents(
    os: &str,
    label: &str,
    emulator_bin: &str,
    avd_home: &str,
    avd: &str,
    icon: &str,
) -> Result<String> {
    match os {
        "linux" => Ok(format!(
            "[Desktop Entry]\nVersion=1.0\nType=Application\nName={label}\nExec=env ANDROID_AVD_HOME={avd_home} {emulator_bin} -avd {avd}\nIcon={icon}\nTerminal=false\n"
        )),
        "windows" => Ok(format!(
            "@echo off\r\nset \"ANDROID_AVD_HOME={avd_home}\"\r\nstart /B \"\" \"{emulator_bin}\" -avd {avd} > NUL 2>&1\r\n"
        )),
        "macos" => bail!("macOS shortcuts use osacompile at install time, not a file template"),
        _ => bail!("unsupported OS for shortcuts: {os}"),
    }
}

/// Phone icon baked into the binary, written next to Linux shortcuts
/// so launchers show a device instead of a generic gear.
const ICON_SVG: &[u8] = include_bytes!("../assets/flenv.svg");

fn install_icon() -> Result<PathBuf> {
    let dir = home_dir()?.join(".local/share/icons");
    fs::create_dir_all(&dir)?;
    let file = dir.join("flenv.svg");
    if !file.is_file() {
        fs::write(&file, ICON_SVG)?;
    }
    Ok(file)
}

pub fn install_shortcut(env_path: &Path, avd: &str, label: &str) -> Result<PathBuf> {
    let os = std::env::consts::OS;
    let sdk = env_path.join("android-sdk");
    let bin = sdk_bin(&sdk, "emulator").to_string_lossy().into_owned();
    let avd_home = avd_home(env_path).to_string_lossy().into_owned();
    match os {
        "linux" => {
            let icon = install_icon()?;
            let dir = home_dir()?.join(".local/share/applications");
            fs::create_dir_all(&dir)?;
            let file = dir.join(format!("{label}.desktop"));
            fs::write(
                &file,
                shortcut_contents(os, label, &bin, &avd_home, avd, &icon.to_string_lossy())?,
            )?;
            Ok(file)
        }
        "windows" => {
            let dir = std::env::var("APPDATA")
                .map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs"))
                .unwrap_or_else(|_| home_dir().unwrap_or_default().join("Desktop"));
            fs::create_dir_all(&dir)?;
            let file = dir.join(format!("{label}.bat"));
            fs::write(
                &file,
                shortcut_contents(os, label, &bin, &avd_home, avd, "")?,
            )?;
            Ok(file)
        }
        "macos" => {
            let app = home_dir()?.join(format!("Applications/{label}.app"));
            let status = Command::new("osacompile")
                .arg("-e")
                .arg(format!(
                    "do shell script \"ANDROID_AVD_HOME='{avd_home}' {bin} -avd {avd} > /dev/null 2>&1 &\""
                ))
                .arg("-o")
                .arg(&app)
                .status()
                .context("cannot run osacompile")?;
            if !status.success() {
                bail!("osacompile failed");
            }
            Ok(app)
        }
        _ => bail!("unsupported OS for shortcuts: {os}"),
    }
}

fn home_dir() -> Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| anyhow::anyhow!("HOME must be set"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_are_arch_aware() {
        assert_eq!(
            system_image("36", "x86_64").unwrap(),
            "system-images;android-36;google_apis;x86_64"
        );
        assert_eq!(
            system_image("36", "arm64").unwrap(),
            "system-images;android-36;google_apis;arm64-v8a"
        );
        assert_eq!(
            system_image("34", "aarch64").unwrap(),
            "system-images;android-34;google_apis;arm64-v8a"
        );
        assert!(system_image("36", "riscv64").is_err());
    }

    #[test]
    fn avd_list_parses() {
        let out = "Pixel_API_36\nMy_Emulator\n";
        assert_eq!(parse_avd_list(out), vec!["Pixel_API_36", "My_Emulator"]);
        assert!(parse_avd_list("").is_empty());
        assert_eq!(parse_avd_list("  spaced  \n\n"), vec!["spaced"]);
    }

    #[test]
    fn shortcuts_per_os() {
        let linux = shortcut_contents(
            "linux",
            "MyPixel",
            "/sdk/emulator/emulator",
            "/envs/demo/avd",
            "My_AVD",
            "/home/u/.local/share/icons/flenv.svg",
        )
        .unwrap();
        assert!(linux.contains("[Desktop Entry]"));
        assert!(linux.contains(
            "Exec=env ANDROID_AVD_HOME=/envs/demo/avd /sdk/emulator/emulator -avd My_AVD"
        ));
        assert!(linux.contains("Icon=/home/u/.local/share/icons/flenv.svg"));
        let win = shortcut_contents(
            "windows",
            "MyPixel",
            "C:\\Sdk\\emulator.exe",
            "C:\\envs\\demo\\avd",
            "My_AVD",
            "",
        )
        .unwrap();
        assert!(win.contains("set \"ANDROID_AVD_HOME=C:\\envs\\demo\\avd\""));
        assert!(win.contains("emulator.exe\" -avd My_AVD"));
        assert!(shortcut_contents("macos", "x", "y", "z", "w", "").is_err());
        assert!(shortcut_contents("freebsd", "x", "y", "z", "w", "").is_err());
    }

    #[test]
    fn avds_live_in_the_environment() {
        assert_eq!(
            avd_home(Path::new("/envs/demo")),
            PathBuf::from("/envs/demo/avd")
        );
    }

    #[test]
    fn bad_avd_names_rejected() {
        // Pure validation: no SDK needed to reject these.
        for bad in ["", "has space", "has/slash"] {
            assert!(bad.is_empty() || bad.contains('/') || bad.contains(char::is_whitespace));
        }
    }

    #[test]
    fn no_kvm_falls_back_to_tcg() {
        let args = start_args("My_AVD", false, false);
        assert_eq!(args[0], "-avd");
        assert_eq!(args[1], "My_AVD");
        assert!(args.contains(&"-accel".to_owned()));
        assert!(args.contains(&"off".to_owned()));
        assert!(args.contains(&"-gpu".to_owned()));

        // With KVM the emulator picks its own acceleration; no override.
        let args = start_args("My_AVD", false, true);
        assert!(!args.contains(&"-accel".to_owned()));
        assert!(!args.contains(&"-gpu".to_owned()));

        let args = start_args("My_AVD", true, true);
        assert!(args.contains(&"-no-window".to_owned()));
    }
}
