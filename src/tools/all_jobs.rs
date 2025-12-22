use std::{path::Path, sync::Arc};

use chrono::Utc;

use crate::{
    apis::google_sheets::{
        self,
        spreadsheet::{
            CellData, ExtendedValue, GridData, GridProperties, RowData, Sheet, SheetProperties,
            Spreadsheet, SpreadsheetProperties,
        },
    },
    jobs::{Job, MilestoneDates},
};

/// Returns the id of the spreadsheet written to.
pub fn generate_all_jobs_google_sheets(
    all_jobs: impl Iterator<Item = Arc<Job>>,
    spreadsheet_id: Option<&str>,
    oauth_cache_file: &Path,
) -> anyhow::Result<String> {
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
        ExtendedValue::StringValue("JNID".to_string()),
        ExtendedValue::StringValue("Job Number".to_string()),
        ExtendedValue::StringValue("Job Name".to_string()),
        ExtendedValue::StringValue("Sales Rep".to_string()),
        ExtendedValue::StringValue("State".to_string()),
        ExtendedValue::StringValue("Branch ID".to_string()),
        ExtendedValue::StringValue("Created Date".to_string()),
        ExtendedValue::StringValue("Status".to_string()),
        ExtendedValue::StringValue("Status Mod. Date".to_string()),
        ExtendedValue::StringValue("Lead Source".to_string()),
        ExtendedValue::StringValue("Insurance Job?".to_string()),
        ExtendedValue::StringValue("Insurance Claim Number".to_string()),
        ExtendedValue::StringValue("Insurance Company Name".to_string()),
        ExtendedValue::StringValue("Appt. Date".to_string()),
        ExtendedValue::StringValue("Contingency Date".to_string()),
        ExtendedValue::StringValue("Contract Date".to_string()),
        ExtendedValue::StringValue("Install Date".to_string()),
        ExtendedValue::StringValue("Job Loss Date".to_string()),
        ExtendedValue::StringValue("Amt Receivable".to_string()),
    ]));
    for job in all_jobs {
        let Job {
            jnid,
            job_number,
            sales_rep,
            state,
            branch,
            status,
            lead_source,
            created_date,
            milestone_dates,
            status_mod_date,
            insurance_checkbox,
            insurance_claim_number,
            insurance_company_name,
            job_name,
            amt_receivable,
        } = &*job;
        let MilestoneDates {
            appointment_date,
            contingency_date,
            contract_date,
            install_date,
            loss_date,
        } = milestone_dates;
        rows.push(mk_row([
            ExtendedValue::StringValue(jnid.clone()),
            ExtendedValue::StringValue(job_number.clone().unwrap_or_default()),
            ExtendedValue::StringValue(job_name.clone().unwrap_or_default()),
            ExtendedValue::StringValue(sales_rep.clone().unwrap_or_default()),
            ExtendedValue::StringValue(state.clone().unwrap_or_default()),
            ExtendedValue::StringValue(branch.map(|b| b.to_string()).unwrap_or_default()),
            ExtendedValue::StringValue(created_date.date_naive().to_string()),
            ExtendedValue::StringValue(status.to_string()),
            ExtendedValue::StringValue(status_mod_date.date_naive().to_string()),
            ExtendedValue::StringValue(lead_source.clone().unwrap_or_default()),
            ExtendedValue::StringValue(insurance_checkbox.to_string()),
            ExtendedValue::StringValue(insurance_claim_number.clone().unwrap_or_default()),
            ExtendedValue::StringValue(insurance_company_name.clone().unwrap_or_default()),
            ExtendedValue::StringValue(
                appointment_date.map(|d| d.date_naive().to_string()).unwrap_or_default(),
            ),
            ExtendedValue::StringValue(
                contingency_date.map(|d| d.date_naive().to_string()).unwrap_or_default(),
            ),
            ExtendedValue::StringValue(
                contract_date.map(|d| d.date_naive().to_string()).unwrap_or_default(),
            ),
            ExtendedValue::StringValue(
                install_date.map(|d| d.date_naive().to_string()).unwrap_or_default(),
            ),
            ExtendedValue::StringValue(
                loss_date.map(|d| d.date_naive().to_string()).unwrap_or_default(),
            ),
            ExtendedValue::StringValue((*amt_receivable as f64 / 100.0).to_string()),
        ]));
    }
    rows.push(mk_row(vec![ExtendedValue::StringValue("".to_string()); rows[0].values.len()]));

    let sheet = Sheet {
        properties: SheetProperties {
            title: Some("All Jobs".to_string()),
            grid_properties: Some(GridProperties { row_count: rows.len() as u64 + 1 }),
            ..Default::default()
        },
        data: Some(GridData { start_row: 1, start_column: 1, row_data: rows }),
    };

    let spreadsheet = Spreadsheet {
        properties: SpreadsheetProperties { title: Some(format!("All Jobs ({})", Utc::now())) },
        sheets: Some(vec![sheet]),
        ..Default::default()
    };

    // generate the spreadsheet
    google_sheets::write_spreadsheet(spreadsheet_id, spreadsheet, oauth_cache_file)
}
