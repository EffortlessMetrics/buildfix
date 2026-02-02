use anyhow::Context;
use clap::{Parser, Subcommand};
use fs_err as fs;

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Workspace helper tasks")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print schema identifiers used by buildfix.
    PrintSchemas,
    /// Create an empty artifacts layout (artifacts/<sensor>/report.json placeholders).
    InitArtifacts {
        #[arg(long, default_value = "artifacts")]
        dir: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Command::PrintSchemas => {
            println!("{}", buildfix_types::schema::BUILDFIX_PLAN_V1);
            println!("{}", buildfix_types::schema::BUILDFIX_APPLY_V1);
            println!("{}", buildfix_types::schema::BUILDFIX_REPORT_V1);
        }
        Command::InitArtifacts { dir } => {
            fs::create_dir_all(&dir).with_context(|| format!("create {dir}"))?;
            for s in ["buildscan", "builddiag", "depguard", "buildfix"] {
                fs::create_dir_all(format!("{dir}/{s}"))?;
            }
            println!("initialized {dir}/{{buildscan,builddiag,depguard,buildfix}}");
        }
    }
    Ok(())
}
