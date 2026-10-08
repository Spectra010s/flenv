use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod android;
mod dl;
mod doctor;
mod emulator;
mod env;
mod flutter;
mod java;

/// flenv: start Flutter development without Android Studio.
#[derive(Parser, Debug)]
#[command(name = "flenv", version, about = "Flutter environment manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Install Flutter + Android SDKs into --root
    Setup(SetupArgs),
    /// Verify the toolchain and report problems
    Doctor(DoctorArgs),
    /// List known environments
    List,
    /// Select the default environment
    Use(UseArgs),
    /// Print shell activation exports (eval "$(flenv env <name>)")
    Env(EnvArgs),
    /// Manage emulator images and virtual devices
    Emulator(EmulatorArgs),
    /// Show how to view a running emulator/device
    View(ViewArgs),
}

#[derive(clap::Args, Debug)]
struct SetupArgs {
    /// Storage base; flenv creates <root>/flenv/environments/<name> beneath it
    #[arg(long, value_name = "DIR")]
    root: Option<PathBuf>,

    /// Environment name (default: default)
    #[arg(long, default_value = "default")]
    name: String,

    /// Keep caches and workspace inside the environment
    #[arg(long, default_value_t = false)]
    isolated: bool,

    /// Flutter version: stable or a pinned release like 3.38.0
    #[arg(long, default_value = "stable")]
    flutter_version: String,

    /// Android API level to install
    #[arg(long, default_value = android::DEFAULT_API)]
    android_api: String,

    /// Android build-tools version to install
    #[arg(long, default_value = android::DEFAULT_BUILD_TOOLS)]
    android_build_tools: String,
}

#[derive(clap::Args, Debug)]
struct DoctorArgs {
    /// Diagnose a named environment (default: selected)
    #[arg(long)]
    name: Option<String>,

    /// Show resolved paths and readiness state
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

#[derive(clap::Args, Debug)]
struct UseArgs {
    /// Environment to select
    name: String,
}

#[derive(clap::Args, Debug)]
struct EnvArgs {
    /// Environment to activate
    name: String,
}

#[derive(clap::Args, Debug)]
struct EmulatorArgs {
    #[command(subcommand)]
    action: EmulatorAction,
}

#[derive(clap::Subcommand, Debug)]
enum EmulatorAction {
    /// Install a system image and create an AVD
    Create(EmulatorCreateArgs),
    /// List available AVDs
    List(EmulatorEnvArgs),
    /// Start an emulator (detached, boots in background)
    Start(EmulatorStartArgs),
}

#[derive(clap::Args, Debug)]
struct EmulatorEnvArgs {
    /// Environment holding the SDK (default: selected)
    #[arg(long)]
    name: Option<String>,
}

#[derive(clap::Args, Debug)]
struct EmulatorCreateArgs {
    /// Environment holding the SDK (default: selected)
    #[arg(long)]
    name: Option<String>,

    /// AVD name (default: <env>_API_<api>)
    #[arg(long)]
    avd: Option<String>,

    /// Android API level for the system image
    #[arg(long, default_value = android::DEFAULT_API)]
    api: String,

    /// Device profile for the AVD
    #[arg(long, default_value = "pixel")]
    device: String,

    /// Skip the desktop shortcut
    #[arg(long, default_value_t = false)]
    no_shortcut: bool,
}

#[derive(clap::Args, Debug)]
struct EmulatorStartArgs {
    /// Environment holding the SDK (default: selected)
    #[arg(long)]
    name: Option<String>,

    /// AVD to start (default: first available)
    #[arg(long)]
    avd: Option<String>,

    /// Run without a window (headless / slow-preview hosts)
    #[arg(long, default_value_t = false)]
    no_window: bool,
}

#[derive(clap::Args, Debug)]
struct ViewArgs {
    /// Environment to inspect (default: selected)
    #[arg(long)]
    name: Option<String>,

    /// Viewer backend: auto (default) or redroid
    #[arg(long, default_value = "auto")]
    backend: String,
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        eprintln!("flenv: error: {err:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Setup(args) => cmd_setup(args),
        Commands::Doctor(args) => cmd_doctor(args),
        Commands::List => cmd_list(),
        Commands::Use(args) => cmd_use(args),
        Commands::Env(args) => cmd_env(args),
        Commands::Emulator(args) => cmd_emulator(args),
        Commands::View(args) => cmd_view(args),
    }
}

fn cmd_setup(args: SetupArgs) -> anyhow::Result<()> {
    env::validate_name(&args.name)?;
    let home = env::flenv_home()?;
    let environment = env::environment_path(&args.name, args.root.as_deref(), &home)?;
    std::fs::create_dir_all(environment.join("flutter"))?;
    std::fs::create_dir_all(environment.join("state"))?;
    if args.isolated {
        std::fs::create_dir_all(environment.join("cache/pub"))?;
        std::fs::create_dir_all(environment.join("cache/gradle"))?;
        std::fs::create_dir_all(environment.join("workspace"))?;
    }
    // Record first so retries and later commands resolve the name.
    env::write_record(&home, &args.name, &environment, args.isolated)?;

    println!("[1/2] Flutter");
    flutter::provision(&environment, &args.flutter_version)?;

    println!("\n[2/2] Android");
    android::provision(&environment, &args.android_api, &args.android_build_tools)?;

    println!("\nEnvironment {} provisioned.", args.name);
    println!("Path: {}", environment.display());
    Ok(())
}

fn cmd_doctor(args: DoctorArgs) -> anyhow::Result<()> {
    let home = env::flenv_home()?;
    doctor::run(&home, args.name.as_deref(), args.verbose)
}

fn cmd_list() -> anyhow::Result<()> {
    let home = env::flenv_home()?;
    std::fs::create_dir_all(home.join("records"))?;
    println!("{:<20} {:<10} PATH", "NAME", "STATUS");
    let entries = env::list(&home)?;
    if entries.is_empty() {
        println!("No environments recorded.");
        return Ok(());
    }
    for entry in entries {
        let mut display = entry.name.clone();
        if entry.record.as_ref().is_some_and(|r| r.isolated) {
            display.push_str(" (isolated)");
        }
        if entry.selected {
            display = format!("* {display}");
        }
        println!(
            "{:<20} {:<10} {}",
            display,
            entry.status.to_string(),
            entry
                .record
                .map(|r| r.path.display().to_string())
                .unwrap_or_else(|| "-".to_owned())
        );
    }
    Ok(())
}

/// `use` persists the selection only — it cannot mutate the parent
/// shell. The `shell/flenv.sh` wrapper layers `env` output on top.
fn cmd_use(args: UseArgs) -> anyhow::Result<()> {
    let home = env::flenv_home()?;
    env::set_selected(&home, &args.name)?;
    println!("Selected environment: {}", args.name);
    Ok(())
}

/// Print activation exports on stdout only; failures go to stderr via
/// main's error path so `eval` never consumes a broken environment.
fn cmd_env(args: EnvArgs) -> anyhow::Result<()> {
    let home = env::flenv_home()?;
    let path = env::resolve(&home, &args.name)?;
    let record = env::read_record(&home, &args.name)?
        .ok_or_else(|| anyhow::anyhow!("environment not found: {}", args.name))?;
    let java_file = path.join("state/java-home");
    let java_home = std::fs::read_to_string(&java_file)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .filter(|p| p.join("bin/java").is_file());
    print!(
        "{}",
        env::render_activation(
            &path,
            &args.name,
            record.isolated,
            java_home.as_deref(),
            &env::HostSnapshot::capture()
        )
    );
    Ok(())
}

fn cmd_emulator(args: EmulatorArgs) -> anyhow::Result<()> {
    let home = env::flenv_home()?;
    match args.action {
        EmulatorAction::Create(c) => {
            let (env_name, path) = env::resolve_or_selected(&home, c.name.as_deref())?;
            let avd = c.avd.unwrap_or_else(|| format!("{env_name}_API_{}", c.api));
            emulator::create(&path, &avd, &c.api, &c.device)?;
            if !c.no_shortcut {
                match emulator::install_shortcut(&path, &avd, &avd) {
                    Ok(file) => println!("  ✓ Shortcut: {}", file.display()),
                    Err(e) => eprintln!("  → Shortcut skipped: {e:#}"),
                }
            }
            Ok(())
        }
        EmulatorAction::List(l) => {
            let (_, path) = env::resolve_or_selected(&home, l.name.as_deref())?;
            let avds = emulator::list_avds(&path)?;
            if avds.is_empty() {
                println!("No AVDs. Create one with: flenv emulator create");
            }
            for avd in avds {
                println!("{avd}");
            }
            Ok(())
        }
        EmulatorAction::Start(s) => {
            let (_, path) = env::resolve_or_selected(&home, s.name.as_deref())?;
            emulator::start(&path, s.avd.as_deref(), s.no_window)?;
            Ok(())
        }
    }
}

fn cmd_view(args: ViewArgs) -> anyhow::Result<()> {
    // Full viewer output lands in #42 (backend 1) and #43 (redroid).
    println!(
        "view: name={} backend={}",
        args.name.as_deref().unwrap_or("(selected)"),
        args.backend,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_help_lists_all_commands() {
        let mut cmd = Cli::command();
        let help = cmd.render_help().to_string();
        for sub in ["setup", "doctor", "list", "use", "env", "emulator", "view"] {
            assert!(help.contains(sub), "help missing: {sub}\n{help}");
        }
    }

    #[test]
    fn setup_parses_root_name_isolated() {
        let cli = Cli::try_parse_from([
            "flenv",
            "setup",
            "--root",
            "/mnt",
            "--name",
            "x",
            "--isolated",
        ])
        .expect("parse setup");
        let Commands::Setup(args) = cli.command else {
            panic!("wrong subcommand");
        };
        assert_eq!(args.root, Some(PathBuf::from("/mnt")));
        assert_eq!(args.name, "x");
        assert!(args.isolated);
    }

    #[test]
    fn setup_defaults() {
        let cli = Cli::try_parse_from(["flenv", "setup"]).expect("parse setup defaults");
        let Commands::Setup(args) = cli.command else {
            panic!("wrong subcommand");
        };
        assert_eq!(args.root, None);
        assert_eq!(args.name, "default");
        assert!(!args.isolated);
    }
}
