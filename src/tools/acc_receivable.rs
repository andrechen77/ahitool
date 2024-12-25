use std::{collections::HashMap, io::Write, sync::Arc};

use chrono::Utc;

use crate::{
    apis::google_sheets::{
        self,
        spreadsheet::{
            CellData, ExtendedValue, GridData, RowData, Sheet, SheetProperties, Spreadsheet,
            SpreadsheetProperties,
        },
    },
    jobs::{Job, Status},
    utils,
};

const CATEGORIES_WE_CARE_ABOUT: &[Status] = &[
    Status::PendingPayments,
    Status::PostInstallSupplementPending,
    Status::JobsInProgress,
    Status::FinalWalkAround,
    Status::SubmitCoc,
    Status::PunchList,
    Status::JobCompleted,
    Status::Collections,
];

pub struct AccRecvableData {
    pub total: i32,
    pub categorized_jobs: HashMap<Status, (i32, Vec<Arc<Job>>)>,
}

pub fn calculate_acc_receivable(jobs: impl IntoIterator<Item = Arc<Job>>) -> AccRecvableData {
    let mut results = AccRecvableData { total: 0, categorized_jobs: HashMap::new() };
    for category in CATEGORIES_WE_CARE_ABOUT {
        results.categorized_jobs.insert(category.clone(), (0, Vec::new()));
    }
    for job in jobs {
        let amt = job.amt_receivable;

        if let Some((category_total, category_jobs)) = results.categorized_jobs.get_mut(&job.status)
        {
            results.total += amt;
            *category_total += amt;
            category_jobs.push(job);
        }
    }
    results
}

pub fn print_human(results: &AccRecvableData, mut writer: impl Write) -> std::io::Result<()> {
    let mut zero_amt_jobs = Vec::new();

    writeln!(writer, "Total: ${}", results.total as f64 / 100.0)?;
    for (status, (category_total, jobs)) in &results.categorized_jobs {
        writeln!(writer, "    - {}: total ${}", status, *category_total as f64 / 100.0)?;
        for job in jobs {
            if job.amt_receivable == 0 {
                zero_amt_jobs.push(job);
                continue;
            }

            let name = job.job_name.as_deref().unwrap_or("");
            let number = job.job_number.as_deref().unwrap_or("Unknown Job Number");
            let amount_receivable = job.amt_receivable as f64 / 100.0;
            let days_in_status = Utc::now().signed_duration_since(job.status_mod_date).num_days();
            writeln!(
                writer,
                "        - {} (#{}): ${:.2} ({} days, assigned to {})",
                name,
                number,
                amount_receivable,
                days_in_status,
                job.sales_rep.as_deref().unwrap_or("Unknown Sales Rep")
            )?;
        }
    }

    writeln!(writer, "Jobs with $0 receivable:")?;
    for job in zero_amt_jobs {
        let name = job.job_name.as_deref().unwrap_or("");
        let number = job.job_number.as_deref().unwrap_or("Unknown Job Number");
        let days_in_status = Utc::now().signed_duration_since(job.status_mod_date).num_days();
        writeln!(
            writer,
            "    - {} (#{}): ({} for {} days, assigned to {})",
            name,
            number,
            job.status,
            days_in_status,
            job.sales_rep.as_deref().unwrap_or("Unknown Sales Rep")
        )?;
    }

    Ok(())
}

pub fn print_csv(results: &AccRecvableData, writer: impl Write) -> std::io::Result<()> {
    let mut writer = csv::Writer::from_writer(writer);
    writer.write_record(&[
        "Job Name",
        "Sales Rep",
        "Job Number",
        "Job Status",
        "Amount",
        "Days In Status",
    ])?;
    for (_status, (_category_total, jobs)) in &results.categorized_jobs {
        for job in jobs {
            let name = job.job_name.as_deref().unwrap_or("");
            let sales_rep = job.sales_rep.as_deref().unwrap_or("Unknown Salesman");
            let number = job.job_number.as_deref().unwrap_or("Unknown Job Number");
            let status = format!("{}", job.status);
            let amount_receivable = (job.amt_receivable as f64) / 100.0;
            let days_in_status = Utc::now().signed_duration_since(job.status_mod_date).num_days();
            writer.write_record(&[
                name,
                sales_rep,
                number,
                &status,
                &amount_receivable.to_string(),
                &days_in_status.to_string(),
            ])?;
        }
    }
    writer.flush()?;
    Ok(())
}

pub fn generate_report_google_sheets(
    results: &AccRecvableData,
    spreadsheet_id: Option<&str>,
) -> anyhow::Result<()> {
    fn mk_row(cells: impl IntoIterator<Item = ExtendedValue>) -> RowData {
        RowData {
            values: cells
                .into_iter()
                .map(|cell| CellData { user_entered_value: Some(cell) })
                .collect(),
        }
    }

    let mut rows = Vec::new();
    rows.push(mk_row([
        ExtendedValue::StringValue("Job Name".to_string()),
        ExtendedValue::StringValue("Job Salesman".to_string()),
        ExtendedValue::StringValue("Job Number".to_string()),
        ExtendedValue::StringValue("Job Status".to_string()),
        ExtendedValue::StringValue("Amount".to_string()),
        ExtendedValue::StringValue("Days In Status".to_string()),
    ]));
    for (_status, (_category_total, jobs)) in &results.categorized_jobs {
        for job in jobs {
            let name = job.job_name.as_deref().unwrap_or("");
            let sales_rep = job.sales_rep.as_deref().unwrap_or("Unknown Salesman");
            let number = job.job_number.as_deref().unwrap_or("Unknown Job Number");
            let status = job.status.to_string();
            let amount_receivable = (job.amt_receivable as f64) / 100.0;
            let days_in_status = Utc::now().signed_duration_since(job.status_mod_date).num_days();
            rows.push(mk_row([
                ExtendedValue::StringValue(name.to_owned()),
                ExtendedValue::StringValue(sales_rep.to_owned()),
                ExtendedValue::StringValue(number.to_owned()),
                ExtendedValue::StringValue(status),
                ExtendedValue::NumberValue(amount_receivable),
                ExtendedValue::NumberValue(days_in_status as f64),
            ]));
        }
    }

    let spreadsheet = Spreadsheet {
        properties: SpreadsheetProperties {
            title: Some(format!("Accounts Receivable Report ({})", Utc::now())),
        },
        sheets: Some(vec![Sheet {
            properties: SheetProperties {
                title: Some("Accounts Receivable".to_string()),
                ..Default::default()
            },
            data: Some(GridData { start_row: 1, start_column: 1, row_data: rows }),
        }]),
        ..Default::default()
    };

    let url = google_sheets::run_with_credentials(|token| {
        let spreadsheet = spreadsheet.clone();
        if let Some(spreadsheet_id) = spreadsheet_id {
            google_sheets::update_spreadsheet(&token, spreadsheet_id, spreadsheet)
        } else {
            google_sheets::create_spreadsheet(
                &token,
                google_sheets::SheetNickname::AccReceivable,
                spreadsheet,
            )
        }
    })?;
    utils::open_url(url.as_str());
    Ok(())
}
