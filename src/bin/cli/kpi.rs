use std::{path::Path, sync::Arc};

use ahitool::{
    apis::{
        google_sheets::{self, SheetNickname},
        job_nimbus,
    },
    jobs::Job,
    tools,
};
use anyhow::bail;
use chrono::{Datelike as _, NaiveDate, NaiveDateTime, NaiveTime, TimeZone as _, Utc};
use clap::CommandFactory as _;
use tracing::{info, warn};

use crate::CliArgs;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// The JobNimbus API key to use. This key will be cached.
    #[arg(long, default_value = None, global = true, env)]
    jn_api_key: Option<String>,

    /// The filter to use when query JobNimbus for jobs, using ElasticSearch
    /// syntax.
    #[arg(short, long = "filter", default_value = None)]
    filter_filename: Option<String>,

    /// The minimum date to filter jobs by. The final report will only include
    /// jobs where the date that they were settled (date of install or date of
    /// loss) is after the minimum date. Valid options are a date of the form
    /// "%Y-%m-%d", "ytd" (indicating the start of the current year), "today"
    /// (indicating the current date), or "forever" (indicating the beginning of
    /// time).
    #[arg(long = "from", default_value = "forever")]
    from_date: String,
    /// The maximum date to filter jobs by. The final report will only include
    /// jobs where the date that they were settled (date of install or date of
    /// loss) is before the maximum date. Valid options are a date of the form
    /// "%Y-%m-%d", "today" (indicating the current date), or "forever"
    /// (indicating the end of time).
    #[arg(long = "to", default_value = "today")]
    to_date: String,

    /// The format in which to print the output.
    #[arg(long, value_enum, default_value = "google-sheets")]
    format: CliOutputFormat,

    /// The directory to write the output to. "-" or unspecified will write
    /// concatenated file contents to stdout.
    ///
    /// With `--format google-sheets`, this option is the ID of the sheet to
    /// replace.
    #[arg(short, long, default_value = None)]
    output: Option<String>,

    /// Only valid with `--format google-sheets`. Whether to always create a new
    /// Google Sheet. If not specified, then updates the existing Google Sheet
    /// for this command if it exists.
    #[arg(long, conflicts_with = "output")]
    new: bool,
}

#[derive(Debug, clap::ValueEnum, Clone, Copy, Eq, PartialEq)]
enum CliOutputFormat {
    /// Prints a set of human-readable .txt files into the output directory (or
    /// into stdout). Each file corresponds to a sales rep's stats or red flags.
    Human,
    /// Prints a set of CSV files into the output directory. Each file
    /// corresponds to a sales rep's stats, and there is also a CSV file for
    /// red flags.
    Csv,
    /// Outputs a Google Sheet on the user's Google Drive (requires OAuth
    /// authorization).
    GoogleSheets,
}

pub enum OutputSpec<'s> {
    /// Prints a human-readable report into a write stream.
    HumanIntoSingleFile(Box<dyn std::io::Write + Send>),
    /// Prints a human-readable report into a directory, with each file
    /// corresponding to a sales rep.
    HumanIntoDirectory(&'s Path),
    /// Prints a CSV file into a write stream.
    CsvIntoSingleFile(Box<dyn std::io::Write + Send>),
    /// Prints a CSV file into a directory, with each file corresponding to a
    /// sales rep.
    CsvIntoDirectory(&'s Path),
    /// Prints a Google Sheet into a Google Drive account.
    GoogleSheets {
        /// The Google Sheets ID to update. If `None`, then a new Google Sheet
        /// will be created.
        spreadsheet_id: Option<&'s str>,
    },
}

pub async fn main(args: Args) -> anyhow::Result<()> {
    let Args { jn_api_key, filter_filename, from_date, to_date, format, output, new } = args;

    // get the JobNimbus API key
    let jn_api_key = job_nimbus::get_api_key(jn_api_key).await?;

    // parse the output format
    let output: Option<&'static str> = output.map(|s| &*s.leak());
    let output_spec = match format {
        CliOutputFormat::Human => match output {
            Some("-") | None => OutputSpec::HumanIntoSingleFile(Box::new(std::io::stdout())),
            Some(dir) => OutputSpec::HumanIntoDirectory(Path::new(dir)),
        },
        CliOutputFormat::Csv => match output {
            Some("-") | None => OutputSpec::CsvIntoSingleFile(Box::new(std::io::stdout())),
            Some(dir) => OutputSpec::CsvIntoDirectory(Path::new(dir)),
        },
        CliOutputFormat::GoogleSheets => {
            let spreadsheet_id = match output {
                Some(spreadsheet_id) => Some(spreadsheet_id),
                None => {
                    if new {
                        None
                    } else {
                        match google_sheets::read_known_sheets_file(SheetNickname::Kpi).await {
                            Err(e) => {
                                warn!("failed to read known sheets file: {}", e);
                                None
                            }
                            Ok(None) => {
                                info!("no existing spreadsheet found, data will be output to a new one");
                                None
                            }
                            Ok(Some(spreadsheet_id)) => {
                                let spreadsheet_id = &*spreadsheet_id.leak();
                                info!(
                                    "data will be output to a existing sheet with ID {}",
                                    spreadsheet_id
                                );
                                Some(spreadsheet_id)
                            }
                        }
                    }
                }
            };
            OutputSpec::GoogleSheets { spreadsheet_id }
        }
    };
    if format != CliOutputFormat::GoogleSheets && new {
        let err = CliArgs::command().error(
            clap::error::ErrorKind::ArgumentConflict,
            "The `--new` option can only be used with `--format google-sheets`",
        );
        bail!(err);
    }

    // get the filter to use with the query
    let filter = if let Some(filter_filename) = filter_filename {
        Some(tokio::fs::read_to_string(filter_filename).await?)
    } else {
        None
    };

    // parse the date range
    let from_date = match from_date.as_str() {
        "forever" => None,
        "ytd" => Some(
            Utc.from_utc_datetime(&NaiveDateTime::new(
                NaiveDate::from_ymd_opt(Utc::now().year(), 1, 1)
                    .expect("Jan 1 should always be valid in the current year."),
                NaiveTime::MIN,
            )),
        ),
        "today" => Some(Utc::now()),
        date_string => {
            let date = NaiveDate::parse_from_str(date_string, "%Y-%m-%d")
                .map(|date| Utc.from_utc_datetime(&NaiveDateTime::new(date, NaiveTime::MIN)));
            if let Ok(date) = date {
                Some(date)
            } else {
                let err = CliArgs::command().error(
                    clap::error::ErrorKind::ArgumentConflict,
                    "Invalid date format. Use 'forever', 'ytd', 'today', or '%Y-%m-%d'",
                );
                bail!(err);
            }
        }
    };
    let to_date = match to_date.as_str() {
        "forever" => None,
        "today" => Some(Utc::now()),
        date_string => {
            let date = NaiveDate::parse_from_str(date_string, "%Y-%m-%d")
                .map(|date| Utc.from_utc_datetime(&NaiveDateTime::new(date, NaiveTime::MIN)));
            if let Ok(date) = date {
                Some(date)
            } else {
                let err = CliArgs::command().error(
                    clap::error::ErrorKind::ArgumentConflict,
                    "Invalid date format. Use 'forever', 'ytd', 'today', or '%Y-%m-%d'",
                );
                bail!(err);
            }
        }
    };

    // do the processing
    let client = reqwest::Client::new();
    let jobs: Vec<Arc<Job>> =
        job_nimbus::get_all_jobs_from_job_nimbus(client, &jn_api_key, filter.as_deref())
            .await?
            .map(|job| Arc::new(job))
            .collect();
    let kpi_result =
        tokio::task::spawn_blocking(move || tools::kpi::calculate_kpi(jobs, (from_date, to_date)))
            .await?;

    // output the results
    use tools::kpi::output;
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        match output_spec {
            OutputSpec::HumanIntoSingleFile(mut writer) => {
                output::human::print_entire_report_to_writer(&kpi_result, &mut writer)?;
                writer.flush()?;
            }
            OutputSpec::HumanIntoDirectory(dir) => {
                output::human::print_entire_report_directory(&kpi_result, dir)?;
            }
            OutputSpec::CsvIntoSingleFile(mut writer) => {
                output::csv::print_entire_report_to_writer(&kpi_result, &mut writer)?;
                writer.flush()?;
            }
            OutputSpec::CsvIntoDirectory(dir) => {
                output::csv::print_entire_report_directory(&kpi_result, dir)?;
            }
            OutputSpec::GoogleSheets { spreadsheet_id } => {
                output::generate_report_google_sheets(&kpi_result, spreadsheet_id)?;
            }
        }
        Ok(())
    })
    .await??;

    Ok(())
}
