use std::collections::BTreeMap;
use std::fmt::Display;
use std::sync::Arc;

use crate::date_range::DateRange;
use crate::jobs::AnalyzedJob;
use crate::jobs::Job;
use crate::jobs::JobAnalysisError;
use csv as csv_crate;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KpiSubject {
    Global,
    SalesRep(String),
    UnknownSalesRep,
}
impl Display for KpiSubject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl KpiSubject {
    pub fn as_str(&self) -> &str {
        match self {
            KpiSubject::Global => "[Global]",
            KpiSubject::SalesRep(name) => &name,
            KpiSubject::UnknownSalesRep => "[Unknown]",
        }
    }
}

pub use processing::JobTrackerStats;

pub struct KpiData {
    pub stats_by_rep: BTreeMap<KpiSubject, JobTrackerStats>,
    pub red_flags_by_rep: BTreeMap<KpiSubject, Vec<(Arc<AnalyzedJob>, JobAnalysisError)>>,
}

pub fn calculate_kpi<'a>(
    jobs: impl IntoIterator<Item = Arc<Job>>,
    date_range: DateRange,
) -> KpiData {
    let (trackers_by_rep, red_flags_by_rep) =
        processing::process_jobs(jobs.into_iter(), date_range);
    let stats_by_rep: BTreeMap<_, _> = trackers_by_rep
        .into_iter()
        .map(|(rep, tracker)| (rep, processing::calculate_job_tracker_stats(&tracker)))
        .filter(|(_, stats)| stats.appt_count > 0)
        .collect();
    KpiData { stats_by_rep, red_flags_by_rep }
}

mod processing {
    use std::{collections::BTreeMap, sync::Arc};

    use tracing::info;

    use crate::{
        date_range::DateRange,
        job_tracker::{self, CalcStatsResult, JobTracker3x5},
        jobs::{self, AnalyzedJob, Job, JobAnalysisError, JobKind, Milestone, TimeDelta},
    };

    use super::KpiSubject;

    type TrackersAndFlags = (
        BTreeMap<KpiSubject, JobTracker3x5>,
        BTreeMap<KpiSubject, Vec<(Arc<AnalyzedJob>, JobAnalysisError)>>,
    );

    pub fn process_jobs(
        jobs: impl Iterator<Item = Arc<Job>>,
        date_range: DateRange,
    ) -> TrackersAndFlags {
        let DateRange { from_date, to_date } = date_range;
        info!(
            "Processing jobs settled between {} and {}",
            from_date.map(|dt| dt.to_string()).as_deref().unwrap_or("the beginning of time"),
            to_date.map(|dt| dt.to_string()).as_deref().unwrap_or("the end of time")
        );

        let mut trackers = BTreeMap::new();
        let mut red_flags = BTreeMap::new();
        for job in jobs {
            let (analyzed, errors) = jobs::analyze_job(job);
            let analyzed = Arc::new(analyzed);
            let target = match analyzed.job.sales_rep.clone() {
                Some(name) => KpiSubject::SalesRep(name),
                None => KpiSubject::UnknownSalesRep,
            };
            if let AnalyzedJob { analysis: Some(analysis), .. } = analyzed.as_ref() {
                // only add jobs that were settled
                if let Some(date_settled) = analysis.date_settled() {
                    // only add jobs that were settled within the date range
                    if (from_date.is_none() || date_settled >= from_date.unwrap())
                        && (to_date.is_none() || date_settled <= to_date.unwrap())
                    {
                        let kind = analysis.kind.into_int();
                        trackers
                            .entry(KpiSubject::Global)
                            .or_insert_with(job_tracker::build_job_tracker)
                            .add_job(
                                &analyzed,
                                kind,
                                &analysis.timestamps,
                                analysis.loss_timestamp,
                            );
                        trackers
                            .entry(target.clone())
                            .or_insert_with(job_tracker::build_job_tracker)
                            .add_job(
                                &analyzed,
                                kind,
                                &analysis.timestamps,
                                analysis.loss_timestamp,
                            );
                    }
                }
            }

            if !errors.is_empty() {
                let sales_rep_errors: &mut Vec<_> = red_flags.entry(target).or_default();
                for error in errors {
                    sales_rep_errors.push((analyzed.clone(), error));
                }
            }
        }

        (trackers, red_flags)
    }

    #[derive(Debug)]
    pub struct JobTrackerStats {
        pub appt_count: usize,
        pub install_count: usize,
        pub loss_conv: ConversionStats,
        pub appt_continge_conv: ConversionStats,
        pub appt_contract_insure_conv: ConversionStats,
        pub continge_contract_conv: ConversionStats,
        pub appt_contract_retail_conv: ConversionStats,
        pub install_insure_conv: ConversionStats,
        pub install_retail_conv: ConversionStats,
    }

    #[derive(Debug)]
    pub struct ConversionStats {
        /// All the jobs that made the conversion.
        pub achieved: Vec<Arc<AnalyzedJob>>,
        /// The rate of conversion. `None` if no jobs made the conversion.
        pub conversion_rate: Option<f64>,
        /// The average amount of time for a successful conversion. Zero if no
        /// jobs made the conversion.
        pub average_time_to_achieve: TimeDelta,
    }

    pub fn calculate_job_tracker_stats(tracker: &JobTracker3x5) -> JobTrackerStats {
        let iwc = JobKind::InsuranceWithContingency.into_int(); // "insurance with contingency"
        let iwo = JobKind::InsuranceWithoutContingency.into_int(); // "insurance without contingency"
        let ret = JobKind::Retail.into_int(); // "retail"

        // some basic stats
        let appt_count = tracker
            .calc_stats(Milestone::AppointmentMade.into_int(), &[iwc, iwo, ret])
            .achieved
            .len();
        let install_count =
            tracker.calc_stats(Milestone::Installed.into_int(), &[iwc, iwo, ret]).achieved.len();

        let loss_conv = {
            let (achieved, average_time_to_achieve) = tracker.calc_stats_of_loss();
            let conversion_rate = if appt_count == 0 {
                None
            } else {
                Some(achieved.len() as f64 / appt_count as f64)
            };
            ConversionStats { achieved, conversion_rate, average_time_to_achieve }
        };

        let num_insure_appts =
            tracker.calc_stats(Milestone::AppointmentMade.into_int(), &[iwc, iwo]).achieved.len();

        // calculate stats for each conversion
        let appt_continge_conv = {
            let job_tracker::Bucket { achieved, cum_achieve_time, .. } = tracker
                .get_bucket(iwc, Milestone::ContingencySigned.into_int())
                .expect("hardcoded value should be valid");
            let num_achieved = achieved.len();
            let conversion_rate = if num_insure_appts == 0 {
                None
            } else {
                Some(num_achieved as f64 / num_insure_appts as f64)
            };
            let average_time_to_achieve = if num_achieved == 0 {
                TimeDelta::zero()
            } else {
                *cum_achieve_time
                    / num_achieved
                        .try_into()
                        .expect("number of jobs should not be great enough to overflow")
            };
            ConversionStats { achieved: achieved.clone(), conversion_rate, average_time_to_achieve }
        };
        let appt_contract_insure_conv = {
            let job_tracker::Bucket { achieved, cum_achieve_time, .. } = tracker
                .get_bucket(iwo, Milestone::ContractSigned.into_int())
                .expect("hardcoded value should be valid");
            let num_achieved = achieved.len();
            let conversion_rate = if num_insure_appts == 0 {
                None
            } else {
                Some(num_achieved as f64 / num_insure_appts as f64)
            };
            let average_time_to_achieve = if num_achieved == 0 {
                TimeDelta::zero()
            } else {
                *cum_achieve_time
                    / num_achieved
                        .try_into()
                        .expect("number of jobs should not be great enough to overflow")
            };
            ConversionStats { achieved: achieved.clone(), conversion_rate, average_time_to_achieve }
        };
        let continge_contract_conv = {
            let CalcStatsResult { achieved, conversion_rate, average_time_to_achieve } =
                tracker.calc_stats(Milestone::ContractSigned.into_int(), &[iwc]);
            ConversionStats { achieved, conversion_rate, average_time_to_achieve }
        };
        let appt_contract_retail_conv = {
            let CalcStatsResult { achieved, conversion_rate, average_time_to_achieve } =
                tracker.calc_stats(Milestone::ContractSigned.into_int(), &[ret]);
            ConversionStats { achieved, conversion_rate, average_time_to_achieve }
        };
        let install_insure_conv = {
            let CalcStatsResult { achieved, conversion_rate, average_time_to_achieve } =
                tracker.calc_stats(Milestone::Installed.into_int(), &[iwc, iwo]);
            ConversionStats { achieved, conversion_rate, average_time_to_achieve }
        };
        let install_retail_conv = {
            let CalcStatsResult { achieved, conversion_rate, average_time_to_achieve } =
                tracker.calc_stats(Milestone::Installed.into_int(), &[ret]);
            ConversionStats { achieved, conversion_rate, average_time_to_achieve }
        };

        JobTrackerStats {
            appt_count,
            install_count,
            loss_conv,
            appt_continge_conv,
            appt_contract_insure_conv,
            continge_contract_conv,
            appt_contract_retail_conv,
            install_insure_conv,
            install_retail_conv,
        }
    }
}

pub mod output {
    use std::{
        fs::File,
        io::{BufWriter, Write},
        path::Path,
        sync::Arc,
    };

    use chrono::Utc;

    use crate::{
        apis::google_sheets::{
            self,
            spreadsheet::{
                CellData, ExtendedValue, GridData, RowData, Sheet, SheetProperties, Spreadsheet,
                SpreadsheetProperties,
            },
        },
        jobs::{AnalyzedJob, JobAnalysisError, TimeDelta},
        utils,
    };

    use super::{csv_crate, processing::JobTrackerStats, KpiData, KpiSubject};

    pub mod human {
        use std::collections::btree_map;

        use super::*;

        pub fn print_single_tracker<'a, 'b, 'w, W>(
            subject: &'a KpiSubject,
            stats: &'b JobTrackerStats,
            out: &'w mut W,
        ) -> std::io::Result<()>
        where
            W: Write,
        {
            writeln!(out, "Tracker for {}: ================", subject)?;
            writeln!(out, "Appts {} | Installed {}", stats.appt_count, stats.install_count)?;
            for (name, conv_stats) in [
                ("All Losses", &stats.loss_conv),
                ("(I) Appt to Contingency", &stats.appt_continge_conv),
                ("(I) Appt to Contract", &stats.appt_contract_insure_conv),
                ("(I) Contingency to Contract", &stats.continge_contract_conv),
                ("(R) Appt to Contract", &stats.appt_contract_retail_conv),
                ("(I) Contract to Installation", &stats.install_insure_conv),
                ("(R) Contract to Installation", &stats.install_retail_conv),
            ] {
                writeln!(
                    out,
                    "{:30}    Rate {} | Total {:2} | Avg Time {:.2} days",
                    name,
                    percent_or_na(conv_stats.conversion_rate),
                    conv_stats.achieved.len(),
                    into_days(conv_stats.average_time_to_achieve),
                )?;
                if *subject != KpiSubject::Global {
                    writeln!(out, "    - {}", into_list_of_job_nums(&conv_stats.achieved))?;
                }
            }
            Ok(())
        }

        pub fn print_red_flags<'a, 'w, W>(
            red_flags_by_rep: btree_map::Iter<
                'a,
                KpiSubject,
                Vec<(Arc<AnalyzedJob>, JobAnalysisError)>,
            >,
            out: &'w mut W,
        ) -> std::io::Result<()>
        where
            W: Write,
        {
            for (rep, red_flags) in red_flags_by_rep {
                writeln!(out, "Red flags for {}: ===============", rep)?;
                for (job, err) in red_flags {
                    writeln!(
                        out,
                        "{}: {}",
                        job.job.job_number.as_deref().unwrap_or("unknown job #"),
                        err
                    )?;
                }
            }
            Ok(())
        }

        pub fn print_entire_report_directory(
            kpi_result: &KpiData,
            output_dir: &Path,
        ) -> std::io::Result<()> {
            super::print_entire_report_directory(
                kpi_result,
                output_dir,
                print_single_tracker,
                print_red_flags,
            )
        }

        pub fn print_entire_report_to_writer<W>(kpi_result: &KpiData, out: W) -> std::io::Result<()>
        where
            W: Write,
        {
            super::print_entire_report_to_writer(
                kpi_result,
                out,
                print_single_tracker,
                print_red_flags,
            )
        }
    }

    pub mod csv {
        use std::collections::btree_map;

        use super::*;

        pub fn print_single_tracker<'a, 'b, 'w, W>(
            _subject: &'a KpiSubject,
            stats: &'b JobTrackerStats,
            out: &'w mut W,
        ) -> std::io::Result<()>
        where
            W: Write,
        {
            let mut writer = csv_crate::Writer::from_writer(out);
            writer.write_record(&["Conversion", "Rate", "Total", "Avg Time (days)", "Jobs"])?;
            for (name, conv_stats) in [
                ("All Losses", &stats.loss_conv),
                ("(I) Appt to Contingency", &stats.appt_continge_conv),
                ("(I) Appt to Contract", &stats.appt_contract_insure_conv),
                ("(I) Contingency to Contract", &stats.continge_contract_conv),
                ("(R) Appt to Contract", &stats.appt_contract_retail_conv),
                ("(I) Contract to Installation", &stats.install_insure_conv),
                ("(R) Contract to Installation", &stats.install_retail_conv),
            ] {
                writer.write_record(&[
                    name,
                    &percent_or_na(conv_stats.conversion_rate),
                    &conv_stats.achieved.len().to_string(),
                    &into_days(conv_stats.average_time_to_achieve).to_string(),
                    &into_list_of_job_nums(&conv_stats.achieved),
                ])?;
            }
            writer.write_record(&[
                "Appts",
                &stats.appt_count.to_string(),
                "",
                "Installed",
                &stats.install_count.to_string(),
            ])?;
            Ok(())
        }

        pub fn print_red_flags<'a, 'w, W>(
            red_flags_by_rep: btree_map::Iter<
                'a,
                KpiSubject,
                Vec<(Arc<AnalyzedJob>, JobAnalysisError)>,
            >,
            out: &'w mut W,
        ) -> std::io::Result<()>
        where
            W: Write,
        {
            let mut writer = csv_crate::Writer::from_writer(out);
            writer.write_record(&["Sales Rep", "Job Number", "Error"])?;
            for (rep, red_flags) in red_flags_by_rep {
                for (job, err) in red_flags {
                    writer.write_record(&[
                        &rep.to_string(),
                        job.job.job_number.as_deref().unwrap_or("unknown job #"),
                        &err.to_string(),
                    ])?;
                }
            }
            Ok(())
        }

        pub fn print_entire_report_directory(
            kpi_result: &KpiData,
            output_dir: &Path,
        ) -> std::io::Result<()> {
            super::print_entire_report_directory(
                kpi_result,
                output_dir,
                print_single_tracker,
                print_red_flags,
            )
        }

        pub fn print_entire_report_to_writer<W>(kpi_result: &KpiData, out: W) -> std::io::Result<()>
        where
            W: Write,
        {
            super::print_entire_report_to_writer(
                kpi_result,
                out,
                print_single_tracker,
                print_red_flags,
            )
        }
    }

    fn print_entire_report_directory<F0, F1>(
        kpi_result: &KpiData,
        output_dir: &Path,
        print_single_tracker: F0,
        print_red_flags: F1,
    ) -> std::io::Result<()>
    where
        F0: for<'a, 'b, 'w> Fn(
            &'a KpiSubject,
            &'b JobTrackerStats,
            &'w mut BufWriter<File>,
        ) -> std::io::Result<()>,
        F1: for<'a, 'w> Fn(
            std::collections::btree_map::Iter<
                'a,
                KpiSubject,
                Vec<(Arc<AnalyzedJob>, JobAnalysisError)>,
            >,
            &'w mut BufWriter<File>,
        ) -> std::io::Result<()>,
    {
        let KpiData { stats_by_rep, red_flags_by_rep } = kpi_result;

        // make sure that the output directory exists
        std::fs::create_dir_all(output_dir)?;

        // print the trackers
        for (rep, stats) in stats_by_rep {
            let mut out_file = BufWriter::new(
                std::fs::File::create(output_dir.join(format!("rep-{}-stats.txt", rep)))
                    .expect("the directory should exist"),
            );
            print_single_tracker(rep, stats, &mut out_file)?;
            out_file.flush()?;
        }

        // print the red flags
        let mut out_file = BufWriter::new(
            std::fs::File::create(output_dir.join("red-flags.txt"))
                .expect("the directory should exist"),
        );
        print_red_flags(red_flags_by_rep.iter(), &mut out_file)?;
        out_file.flush()?;

        Ok(())
    }

    fn print_entire_report_to_writer<W, F0, F1>(
        kpi_result: &KpiData,
        mut out: W,
        print_single_tracker: F0,
        print_red_flags: F1,
    ) -> std::io::Result<()>
    where
        W: Write,
        F0: for<'a, 'b, 'w> Fn(
            &'a KpiSubject,
            &'b JobTrackerStats,
            &'w mut W,
        ) -> std::io::Result<()>,
        F1: for<'a, 'w> Fn(
            std::collections::btree_map::Iter<
                'a,
                KpiSubject,
                Vec<(Arc<AnalyzedJob>, JobAnalysisError)>,
            >,
            &'w mut W,
        ) -> std::io::Result<()>,
    {
        let KpiData { stats_by_rep, red_flags_by_rep } = kpi_result;

        // print the trackers
        for (rep, stats) in stats_by_rep {
            print_single_tracker(rep, stats, &mut out)?;
            writeln!(out)?;
        }

        // print the red flags
        print_red_flags(red_flags_by_rep.iter(), &mut out)?;
        Ok(())
    }

    /// Returns the id of the spreadsheet written to.
    pub fn generate_report_google_sheets(
        kpi_data: &KpiData,
        spreadsheet_id: Option<&str>,
    ) -> anyhow::Result<String> {
        fn mk_row(cells: impl IntoIterator<Item = ExtendedValue>) -> RowData {
            RowData {
                values: cells
                    .into_iter()
                    .map(|cell| CellData { user_entered_value: Some(cell) })
                    .collect(),
            }
        }

        let KpiData { stats_by_rep, red_flags_by_rep } = kpi_data;

        // create a stats sheet for each rep
        let mut sheets: Vec<_> = stats_by_rep
            .into_iter()
            .map(|(rep, stats)| {
                let mut rows = Vec::new();
                rows.push(mk_row([
                    ExtendedValue::StringValue("Conversion".to_string()),
                    ExtendedValue::StringValue("Rate".to_string()),
                    ExtendedValue::StringValue("Total".to_string()),
                    ExtendedValue::StringValue("Avg Time (days)".to_string()),
                    ExtendedValue::StringValue("Jobs".to_string()),
                ]));
                for (name, conv_stats) in [
                    ("All Losses", &stats.loss_conv),
                    ("(I) Appt to Contingency", &stats.appt_continge_conv),
                    ("(I) Appt to Contract", &stats.appt_contract_insure_conv),
                    ("(I) Contingency to Contract", &stats.continge_contract_conv),
                    ("(R) Appt to Contract", &stats.appt_contract_retail_conv),
                    ("(I) Contract to Installation", &stats.install_insure_conv),
                    ("(R) Contract to Installation", &stats.install_retail_conv),
                ] {
                    rows.push(mk_row([
                        ExtendedValue::StringValue(name.to_string()),
                        ExtendedValue::StringValue(percent_or_na(conv_stats.conversion_rate)),
                        ExtendedValue::NumberValue(conv_stats.achieved.len() as f64),
                        ExtendedValue::NumberValue(into_days(conv_stats.average_time_to_achieve)),
                        ExtendedValue::StringValue(into_list_of_job_nums(&conv_stats.achieved)),
                    ]));
                }
                rows.push(mk_row([
                    ExtendedValue::StringValue("Appts".to_string()),
                    ExtendedValue::NumberValue(stats.appt_count as f64),
                    ExtendedValue::StringValue("".to_string()),
                    ExtendedValue::StringValue("Installed".to_string()),
                    ExtendedValue::NumberValue(stats.install_count as f64),
                ]));

                Sheet {
                    properties: SheetProperties {
                        title: Some(format!("Stats {}", rep)),
                        ..Default::default()
                    },
                    data: Some(GridData { start_row: 1, start_column: 1, row_data: rows }),
                    ..Default::default()
                }
            })
            .collect();

        // create the red flags sheet
        let mut rows = Vec::new();
        rows.push(mk_row([
            ExtendedValue::StringValue("Sales Rep".to_string()),
            ExtendedValue::StringValue("Job Number".to_string()),
            ExtendedValue::StringValue("Error".to_string()),
        ]));
        for (rep, red_flags) in red_flags_by_rep {
            for (job, err) in red_flags {
                rows.push(mk_row([
                    ExtendedValue::StringValue(rep.to_string()),
                    ExtendedValue::StringValue(
                        job.job.job_number.as_deref().unwrap_or("unknown job #").to_string(),
                    ),
                    ExtendedValue::StringValue(err.to_string()),
                ]));
            }
        }
        sheets.push(Sheet {
            properties: SheetProperties {
                title: Some("Red Flags".to_string()),
                ..Default::default()
            },
            data: Some(GridData { start_row: 0, start_column: 0, row_data: rows }),
            ..Default::default()
        });

        // create the spreadsheet
        let spreadsheet = Spreadsheet {
            properties: SpreadsheetProperties {
                title: Some(format!("KPI Report ({})", Utc::now())),
            },
            sheets: Some(sheets),
            ..Default::default()
        };

        let (id, url) = google_sheets::run_with_credentials(|token| {
            let spreadsheet = spreadsheet.clone();
            if let Some(spreadsheet_id) = spreadsheet_id {
                google_sheets::update_spreadsheet(&token, spreadsheet_id, spreadsheet)
                    .map(|url| (spreadsheet_id.to_owned(), url))
            } else {
                google_sheets::create_spreadsheet(&token, spreadsheet)
            }
        })?;
        utils::open_url(url.as_str());
        Ok(id)
    }

    pub fn into_days(time: TimeDelta) -> f64 {
        const SECONDS_PER_DAY: f64 = 86400.0;
        time.num_seconds() as f64 / SECONDS_PER_DAY
    }
    pub fn percent_or_na(rate: Option<f64>) -> String {
        rate.map(|r| format!("{:6.2}%", r * 100.0)).unwrap_or_else(|| "    N/A".to_owned())
    }
    pub fn into_list_of_job_nums(jobs: &[Arc<AnalyzedJob>]) -> String {
        jobs.iter()
            .map(|job| job.job.job_number.as_deref().unwrap_or_else(|| &job.job.jnid))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
