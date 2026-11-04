use crate::view;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub const DEFAULT_IMAGE: &str = "redroid/redroid:14.0.0_64only-latest";
const READY_TIMEOUT: Duration = Duration::from_secs(90);
const POLL: Duration = Duration::from_secs(2);

/// Container name per environment, so two environments do not collide.
pub fn container_name(env_name: &str) -> String {
    format!("flenv-redroid-{env_name}")
}

/// Android userdata lives inside the environment and survives restarts.
pub fn data_dir(env_path: &Path) -> PathBuf {
    env_path.join("redroid-data")
}

fn docker(args: &[&str]) -> Result<String> {
    let output = Command::new("docker")
        .args(args)
        .output()
        .context("cannot run docker")?;
    if !output.status.success() {
        bail!(
            "docker {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `docker run` arguments for a fresh container.
pub fn run_args(name: &str, image: &str, port: u16, data: &Path) -> Vec<String> {
    vec![
        "run".to_owned(),
        "-d".to_owned(),
        "--name".to_owned(),
        name.to_owned(),
        // redroid needs binder access; privileged is the documented mode.
        "--privileged".to_owned(),
        "-v".to_owned(),
        format!("{}:/data", data.display()),
        "-p".to_owned(),
        format!("{port}:5555"),
        image.to_owned(),
    ]
}

/// State of an existing container: `running`, `exited`, ..., or `None`.
pub fn container_state(name: &str) -> Result<Option<String>> {
    let filter = format!("name=^{name}$");
    let out = docker(&["ps", "-a", "--filter", &filter, "--format", "{{.State}}"])?;
    Ok(out
        .lines()
        .next()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty()))
}

fn docker_ready() -> Result<()> {
    docker(&["version", "--format", "{{.Server.Version}}"])
        .map(|_| ())
        .map_err(|_| {
            anyhow::anyhow!(
                "Docker daemon not reachable; start Docker or use: flenv view --backend adb"
            )
        })
}

/// Stop the container for an environment. Userdata stays on disk.
pub fn stop_container(env_name: &str) -> Result<()> {
    docker_ready()?;
    let name = container_name(env_name);
    match container_state(&name)? {
        None => println!("No redroid container for {env_name}."),
        Some(state) if state == "running" => {
            docker(&["stop", &name])?;
            println!("  ✓ Stopped redroid {name} (data kept)");
        }
        Some(state) => println!("  → redroid {name} is {state}; nothing to stop"),
    }
    Ok(())
}

/// redroid needs kernel binder support. Missing support is a warning:
/// the container may still mount binderfs itself when privileged.
pub fn binder_present() -> bool {
    Path::new("/dev/binderfs").exists()
        || Path::new("/dev/binder").exists()
        || Path::new("/sys/module/binder_linux").exists()
        || std::fs::read_to_string("/proc/filesystems")
            .map(|s| {
                s.lines()
                    .any(|l| l.split_whitespace().last() == Some("binder"))
            })
            .unwrap_or(false)
}

pub fn binder_hint() -> &'static str {
    "kernel binder support not detected; redroid may fail to boot (enable CONFIG_ANDROID_BINDER_IPC or modprobe binder_linux)"
}

/// Backend 2: run Android in a redroid container, attach it over adb,
/// then print the same viewer endpoints as backend 1.
pub fn run(
    env_name: &str,
    env_path: &Path,
    image: &str,
    port: u16,
    ssh: Option<&str>,
    stop: bool,
) -> Result<()> {
    if stop {
        return stop_container(env_name);
    }
    docker_ready()?;
    if !binder_present() {
        eprintln!("  → {}", binder_hint());
    }

    let name = container_name(env_name);
    let data = data_dir(env_path);
    std::fs::create_dir_all(&data).with_context(|| format!("cannot create {}", data.display()))?;

    match container_state(&name)? {
        Some(state) if state == "running" => {
            eprintln!("  ✓ redroid {name} already running");
        }
        Some(_) => {
            eprintln!("  → Starting redroid {name}");
            docker(&["start", &name])?;
        }
        None => {
            eprintln!("  → Creating redroid {name} ({image})");
            let args = run_args(&name, image, port, &data);
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            docker(&args)?;
        }
    }

    let serial = format!("localhost:{port}");
    if let Err(e) = view::adb_run(env_path, &["connect", &serial]) {
        eprintln!("  → adb connect failed ({e:#}); waiting anyway");
    }

    eprintln!("  → Waiting for {serial}");
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        let devices = view::adb_devices(env_path)?;
        if let Some(device) = devices
            .iter()
            .find(|d| d.serial == serial && d.state == "device")
        {
            let ready = vec![device];
            println!();
            println!("Backend: redroid ({name})");
            view::print_viewer(env_name, &ready, ssh);
            return Ok(());
        }
        if Instant::now() >= deadline {
            let logs = docker(&["logs", "--tail", "20", &name]).unwrap_or_default();
            bail!(
                "redroid did not become ready on {serial} within {}s\n{}",
                READY_TIMEOUT.as_secs(),
                logs.trim()
            );
        }
        sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_names_are_per_env() {
        assert_eq!(container_name("default"), "flenv-redroid-default");
        assert_eq!(container_name("a.b_c-d"), "flenv-redroid-a.b_c-d");
        assert_ne!(container_name("one"), container_name("two"));
    }

    #[test]
    fn data_lives_in_the_environment() {
        let p = data_dir(Path::new("/envs/demo"));
        assert_eq!(p, PathBuf::from("/envs/demo/redroid-data"));
    }

    #[test]
    fn run_args_are_privileged_and_forward_adb() {
        let args = run_args("c", "img:1", 5555, Path::new("/envs/demo/redroid-data"));
        assert_eq!(args[0], "run");
        assert!(args.contains(&"--privileged".to_owned()));
        assert!(args.contains(&"/envs/demo/redroid-data:/data".to_owned()));
        assert!(args.contains(&"5555:5555".to_owned()));
        assert_eq!(args.last().unwrap(), "img:1");
        // Port override maps to the same container port.
        let args = run_args("c", "img", 6000, Path::new("/d"));
        assert!(args.contains(&"6000:5555".to_owned()));
    }

    #[test]
    fn binder_hint_is_nonempty() {
        assert!(!binder_hint().is_empty());
    }
}
