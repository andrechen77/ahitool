use std::{fs::File, io::Write};

use ahitool::{
    apis::{
        google_sheets::{self, SheetNickname},
        job_nimbus,
    },
    tools,
};
use clap::CommandFactory as _;
use tracing::{info, warn};

use crate::CliArgs;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// The JobNimbus API key. This key will be cached.
    #[arg(long, default_value = None, global = true, env)]
    jn_api_key: Option<String>,

    /// The format in which to print the output.
    #[arg(long, value_enum, default_value = "google-sheets")]
    format: CliOutputFormat,

    /// The file to write the output to. "-" or unspecified will write to
    /// stdout. This option is ignored with `--format google-sheets`.
    #[arg(short, long, default_value = None)]
    output: Option<String>,

    /// Only valid with `--format google-sheets`. Whether to always create a new
    /// Google Sheet. If not specified, then updates the existing Google Sheet
    /// for this command if it exists.
    #[arg(long)]
    new: bool,
}

#[derive(Debug, clap::ValueEnum, Clone, Copy, Eq, PartialEq)]
enum CliOutputFormat {
    /// Prints a human-readable report into the output file.
    Human,
    /// Prints a CSV file into the output file.
    Csv,
    /// Outputs a Google Sheet on the user's Google Drive (requires OAuth
    /// authorization).
    GoogleSheets,
}

enum OutputSpec<'s> {
    /// Prints a human-readable report into the write stream.
    Human(Box<dyn Write>),
    /// Prints a CSV file into the write stream.
    Csv(Box<dyn Write>),
    GoogleSheets {
        /// The Google Sheets ID to update. If `None`, then a new Google Sheet
        /// will be created.
        spreadsheet_id: Option<&'s str>,
    },
}

pub fn main(args: Args) -> anyhow::Result<()> {
    let Args { jn_api_key, output, format, new } = args;

    // get the JobNimbus API key
    let jn_api_key = job_nimbus::get_api_key(jn_api_key)?;

    // parse the output
    let s: String; // to hold the result of parsing the known sheets file for lifetime reasons only
    let output_spec = match format {
        CliOutputFormat::Human | CliOutputFormat::Csv => {
            let writer: Box<dyn Write> = match output.as_deref() {
                None | Some("-") => Box::new(std::io::stdout()),
                Some(_) => Box::new(File::create(output.expect("checked was some"))?),
            };
            if format == CliOutputFormat::Human {
                OutputSpec::Human(writer)
            } else {
                OutputSpec::Csv(writer)
            }
        }
        CliOutputFormat::GoogleSheets => {
            let spreadsheet_id = match output.as_deref() {
                Some(spreadsheet_id) => Some(spreadsheet_id),
                None => {
                    if new {
                        None
                    } else {
                        match google_sheets::read_known_sheets_file(SheetNickname::AccReceivable) {
                            Err(e) => {
                                warn!("failed to read known sheets file: {}", e);
                                None
                            }
                            Ok(None) => {
                                info!("no existing spreadsheet found, data will be output to a new one");
                                None
                            }
                            Ok(Some(spreadsheet_id)) => {
                                s = spreadsheet_id;
                                info!("data will be output to a existing sheet with ID {}", &s);
                                Some(s.as_str())
                            }
                        }
                    }
                }
            };
            OutputSpec::GoogleSheets { spreadsheet_id }
        }
    };
    if format != CliOutputFormat::GoogleSheets && new {
        CliArgs::command()
            .error(
                clap::error::ErrorKind::ArgumentConflict,
                "The `--new` option can only be used with `--format google-sheets`",
            )
            .exit();
    }

    // do the processing
    let jobs = job_nimbus::get_all_jobs_from_job_nimbus(&jn_api_key, None)?;
    let acc_recv_data = tools::acc_receivable::calculate_acc_receivable(jobs.iter());

    // output the results
    match output_spec {
        OutputSpec::Human(mut writer) => {
            tools::acc_receivable::print_human(&acc_recv_data, &mut writer)?;
            writer.flush()?;
        }
        OutputSpec::Csv(mut writer) => {
            tools::acc_receivable::print_csv(&acc_recv_data, &mut writer)?;
            writer.flush()?;
        }
        OutputSpec::GoogleSheets { spreadsheet_id } => {
            tools::acc_receivable::generate_report_google_sheets(&acc_recv_data, spreadsheet_id)?;
        }
    }

    Ok(())
}
