use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

mod flutter;

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
    /// Start an emulator for an environment
    Start(StartArgs),
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
struct StartArgs {
    /// Environment to start the emulator from (default: selected)
    #[arg(long)]
    name: Option<String>,
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
        Commands::Start(args) => cmd_start(args),
        Commands::View(args) => cmd_view(args),
    }
}

/// Names become record filenames and path components, so separators
/// and traversal are rejected. (Full record handling lands in #39.)
fn validate_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() || name == "." || name == ".." {
        anyhow::bail!("invalid environment name: {name}");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        anyhow::bail!("invalid environment name: {name}");
    }
    Ok(())
}

/// Resolve the environment directory. External roots are storage bases;
/// flenv keeps its own namespace beneath them. (Records land in #39.)
fn environment_path(name: &str, root: Option<&Path>) -> anyhow::Result<PathBuf> {
    validate_name(name)?;
    if let Some(root) = root {
        if !root.is_dir() {
            anyhow::bail!("root is not a directory: {}", root.display());
        }
        Ok(root.join("flenv/environments").join(name))
    } else {
        let home = std::env::var("FLENV_HOME")
            .map(PathBuf::from)
            .or_else(|_| {
                std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".flenv"))
                    .map_err(|_| anyhow::anyhow!("HOME must be set"))
            })?;
        Ok(home.join("environments").join(name))
    }
}

fn cmd_setup(args: SetupArgs) -> anyhow::Result<()> {
    let environment = environment_path(&args.name, args.root.as_deref())?;
    std::fs::create_dir_all(environment.join("flutter"))?;
    std::fs::create_dir_all(environment.join("state"))?;
    if args.isolated {
        std::fs::create_dir_all(environment.join("cache/pub"))?;
        std::fs::create_dir_all(environment.join("cache/gradle"))?;
        std::fs::create_dir_all(environment.join("workspace"))?;
    }

    println!("[1/2] Flutter");
    flutter::provision(&environment, &args.flutter_version)?;

    // Android provisioning lands in #38.
    println!("\nEnvironment {} provisioned.", args.name);
    println!("Path: {}", environment.display());
    Ok(())
}

fn cmd_doctor(args: DoctorArgs) -> anyhow::Result<()> {
    // Full checks land in #41 (toolchain + KVM warning).
    println!(
        "doctor: name={} verbose={}",
        args.name.as_deref().unwrap_or("(selected)"),
        args.verbose,
    );
    Ok(())
}

fn cmd_list() -> anyhow::Result<()> {
    // Full listing lands in #39 (env roots + records).
    println!("NAME STATUS PATH");
    Ok(())
}

fn cmd_start(args: StartArgs) -> anyhow::Result<()> {
    // Full AVD handling lands in #40.
    println!(
        "start: name={}",
        args.name.as_deref().unwrap_or("(selected)"),
    );
    Ok(())
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
        for sub in ["setup", "doctor", "list", "start", "view"] {
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
