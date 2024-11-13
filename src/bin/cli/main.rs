use clap::Parser;
use tracing::error;

mod acc_receivable;
mod kpi;
mod update;

#[derive(Parser, Debug)]
struct CliArgs {
    /// The command to perform.
    #[command(subcommand)]
    command: Subcommand,
}

fn main() {
    // set up tracing
    tracing_subscriber::fmt::init();

    let CliArgs { command } = CliArgs::parse();

    let result: anyhow::Result<()> =
        tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(async {
            match command {
                Subcommand::Kpi(job_kpi_args) => {
                    kpi::main(job_kpi_args).await?;
                }
                Subcommand::Ar(acc_recv_args) => {
                    acc_receivable::main(acc_recv_args).await?;
                }
                Subcommand::Update(update_args) => {
                    update::main(update_args).await?;
                }
            }
            Ok(())
        });
    if let Err(err) = result {
        if let Some(cli_err) = err.downcast_ref::<clap::Error>() {
            cli_err.exit();
        } else {
            error!("error during execution: {}", err);
            std::process::exit(1);
        }
    }
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
