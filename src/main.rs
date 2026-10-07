use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    match cli.command {
        Commands::Setup(args) => cmd_setup(args),
        Commands::Doctor(args) => cmd_doctor(args),
        Commands::List => cmd_list(),
        Commands::Start(args) => cmd_start(args),
        Commands::View(args) => cmd_view(args),
    }
}

fn cmd_setup(args: SetupArgs) {
    // Full provisioning lands in #37 (Flutter) and #38 (Android).
    println!(
        "setup: name={} root={} isolated={}",
        args.name,
        args.root
            .as_ref()
            .map(|p| p.display().to_string())
            .as_deref()
            .unwrap_or("(default)"),
        args.isolated,
    );
}

fn cmd_doctor(args: DoctorArgs) {
    // Full checks land in #41 (toolchain + KVM warning).
    println!(
        "doctor: name={} verbose={}",
        args.name.as_deref().unwrap_or("(selected)"),
        args.verbose,
    );
}

fn cmd_list() {
    // Full listing lands in #39 (env roots + records).
    println!("NAME STATUS PATH");
}

fn cmd_start(args: StartArgs) {
    // Full AVD handling lands in #40.
    println!(
        "start: name={}",
        args.name.as_deref().unwrap_or("(selected)"),
    );
}

fn cmd_view(args: ViewArgs) {
    // Full viewer output lands in #42 (backend 1) and #43 (redroid).
    println!(
        "view: name={} backend={}",
        args.name.as_deref().unwrap_or("(selected)"),
        args.backend,
    );
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
