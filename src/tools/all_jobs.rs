use std::sync::Arc;

use chrono::Utc;

use crate::{
    apis::google_sheets::{
        self,
        spreadsheet::{
            CellData, ExtendedValue, GridData, GridProperties, RowData, Sheet, SheetProperties,
            Spreadsheet, SpreadsheetProperties,
        },
    },
    jobs::Job,
};

/// Returns the id of the spreadsheet written to.
pub fn generate_all_jobs_google_sheets(
    all_jobs: impl Iterator<Item = Arc<Job>>,
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

    let mut rows = Vec::new();
    rows.push(mk_row([
        ExtendedValue::StringValue("JNID".to_string()),
        ExtendedValue::StringValue("Job Number".to_string()),
        ExtendedValue::StringValue("Sales Rep".to_string()),
        ExtendedValue::StringValue("Status".to_string()),
        ExtendedValue::StringValue("Lead Source".to_string()),
    ]));
    for job in all_jobs {
        rows.push(mk_row([
            ExtendedValue::StringValue(job.jnid.clone()),
            ExtendedValue::StringValue(job.job_number.clone().unwrap_or_default()),
            ExtendedValue::StringValue(job.sales_rep.clone().unwrap_or_default()),
            ExtendedValue::StringValue(job.status.to_string()),
            ExtendedValue::StringValue(job.lead_source.clone().unwrap_or_default()),
        ]));
    }

    let sheet = Sheet {
        properties: SheetProperties {
            title: Some("All Jobs".to_string()),
            grid_properties: Some(GridProperties { row_count: rows.len() as u64 + 2 }),
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
    google_sheets::write_spreadsheet(spreadsheet_id, spreadsheet)
}
