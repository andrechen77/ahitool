use clap::Parser;

mod acc_receivable;
mod kpi;
mod update;

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
            kpi::main(job_kpi_args)?;
        }
        Subcommand::Ar(acc_recv_args) => {
            acc_receivable::main(acc_recv_args)?;
        }
        Subcommand::Update(update_args) => {
            update::main(update_args)?;
        }
    }

    Ok(())
}

#[derive(clap::Subcommand, Debug)]
pub enum Subcommand {
    /// Update the executable to the latest version.
    Update(update::Args),
    /// Generate a KPI report for salesmen based on job milestones.
    Kpi(kpi::Args),
    /// Generate a report for all accounts receivable.
    Ar(acc_receivable::Args),
}
