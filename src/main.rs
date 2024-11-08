use clap::Parser;
use subcommands::Subcommand;

mod apis;
mod job_tracker;
mod jobs;
mod subcommands;
mod utils;

#[derive(Parser, Debug)]
struct CliArgs {
    /// The command to perform.
    #[command(subcommand)]
    command: Subcommand,
}

fn main() -> anyhow::Result<()> {
    // set up tracing
    tracing_subscriber::fmt::init();

    let CliArgs { command } = CliArgs::parse();

    match command {
        Subcommand::Kpi(job_kpi_args) => {
            subcommands::kpi::main(job_kpi_args)?;
        }
        Subcommand::Ar(acc_recv_args) => {
            subcommands::acc_receivable::main(acc_recv_args)?;
        }
        Subcommand::Update(update_args) => {
            subcommands::update::main(update_args)?;
        }
    }

    Ok(())
}
