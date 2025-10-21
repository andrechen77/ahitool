mod oauth;
pub mod spreadsheet;

use std::collections::HashMap;
use std::collections::HashSet;

use anyhow::anyhow;
use http::{StatusCode, header::AUTHORIZATION};
pub use oauth::Token;
use oauth::TryWithCredentialsError;
pub use oauth::run_with_credentials;
use oauth2::TokenResponse as _;
use serde::Deserialize;
use serde_json::json;
use spreadsheet::GridCoordinate;
use spreadsheet::SheetProperties;
use spreadsheet::Spreadsheet;
use spreadsheet::update::Request;
use tracing::info;
use tracing::trace;
use tracing::warn;

use crate::utils;

const ENDPOINT_SPREADSHEETS: &str = "https://sheets.googleapis.com/v4/spreadsheets";

/// Creates the specified spreadsheet in the user's Google Drive. Return the id
/// and URL of the created sheet.
pub fn create_spreadsheet(
    creds: &Token,
    spreadsheet: Spreadsheet,
) -> Result<(String, String), TryWithCredentialsError> {
    trace!("Sending request to create sheet");
    let response = ureq::post(ENDPOINT_SPREADSHEETS)
        .set(AUTHORIZATION.as_str(), format!("Bearer {}", creds.access_token().secret()).as_str())
        .send_json(&spreadsheet);
    let successful_response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(status_code, _)) if status_code == StatusCode::UNAUTHORIZED => {
            return Err(TryWithCredentialsError::Unauthorized(anyhow!(
                "Request to create sheet was unauthorized with status code: {}",
                status_code
            )));
        }
        Err(err) => {
            return Err(TryWithCredentialsError::Other(anyhow!(
                "Request to create sheet failed: {}",
                err
            )));
        }
    };

    #[derive(Deserialize)]
    struct ApiResponse {
        #[serde(rename = "spreadsheetId")]
        spreadsheet_id: String,
        #[serde(rename = "spreadsheetUrl")]
        spreadsheet_url: String,
    }
    let ApiResponse { spreadsheet_id, spreadsheet_url } =
        successful_response.into_json().map_err(anyhow::Error::from)?;

    info!("Created Google Sheet at {}", spreadsheet_url);
    Ok((spreadsheet_id, spreadsheet_url))
}

/// Updates the specified spreadsheet in the user's Google Drive. Return the URL of the created sheet.
pub fn update_spreadsheet(
    creds: &Token,
    spreadsheet_id: &str,
    spreadsheet: Spreadsheet,
) -> Result<String, TryWithCredentialsError> {
    // get the current spreadsheet data so we can merge the new data with it
    let existing_spreadsheet: Spreadsheet = {
        let url = format!("{ENDPOINT_SPREADSHEETS}/{spreadsheet_id}");
        let response = ureq::get(&url)
            .set(
                AUTHORIZATION.as_str(),
                format!("Bearer {}", creds.access_token().secret()).as_str(),
            )
            .call();

        let success_response = match response {
            Ok(response) => response,
            Err(ureq::Error::Status(status_code, _)) if status_code == StatusCode::UNAUTHORIZED => {
                return Err(TryWithCredentialsError::Unauthorized(anyhow!(
                    "request to get current sheet was unauthorized with status code: {}",
                    status_code
                )));
            }
            Err(err) => {
                return Err(TryWithCredentialsError::Other(anyhow!(
                    "request to get current sheet failed: {}",
                    err
                )));
            }
        };

        success_response.into_json().map_err(anyhow::Error::from)?
    };

    // keep track of existing sheet IDs
    let mut title_to_sheet_id = HashMap::new();
    let mut existing_sheet_ids = HashSet::new();
    if let Some(sheets) = existing_spreadsheet.sheets {
        for sheet in sheets {
            let SheetProperties { sheet_id, title, .. } = sheet.properties;
            if let (Some(sheet_id), Some(title)) = (sheet_id, title) {
                title_to_sheet_id.insert(title, sheet_id);
            }
            if let Some(sheet_id) = sheet_id {
                existing_sheet_ids.insert(sheet_id);
            }
        }
    }
    let mut sheets_to_delete = existing_sheet_ids.clone();

    // prepare the correct JSON to send with the `batchUpdate` request. see
    // https://developers.google.com/sheets/api/reference/rest/v4/spreadsheets/batchUpdate
    let mut requests = Vec::new();

    // change the spreadsheet properties
    if spreadsheet.properties.title.is_some() {
        requests.push(Request::UpdateSpreadsheetProperties {
            properties: spreadsheet.properties,
            fields: "title",
        });
    }

    // add new sheets
    if let Some(sheets) = spreadsheet.sheets {
        for sheet in sheets {
            if sheet.properties.sheet_id.is_some() {
                warn!("Sheet ID is ignored when updating a spreadsheet");
            }

            let sheet_id = if let Some(sheet_id) =
                title_to_sheet_id.get(sheet.properties.title.as_ref().unwrap())
            {
                // add a request to update the sheet properties; namely the
                // grid data
                requests.push(Request::UpdateSheetProperties {
                    properties: SheetProperties { sheet_id: Some(*sheet_id), ..sheet.properties },
                    fields: "gridProperties.rowCount",
                });
                sheets_to_delete.remove(sheet_id);

                *sheet_id
            } else {
                // find a sheet ID that is not already in use
                let mut sheet_id = 0;
                while existing_sheet_ids.contains(&sheet_id) {
                    sheet_id += 1;
                }
                existing_sheet_ids.insert(sheet_id);

                // push a request to add a new sheet with the id
                requests.push(Request::AddSheet {
                    properties: SheetProperties { sheet_id: Some(sheet_id), ..sheet.properties },
                });

                sheet_id
            };

            if let Some(grid_data) = sheet.data {
                // push a request to update the content of the sheet
                requests.push(Request::UpdateCells {
                    rows: grid_data.row_data,
                    fields: "userEnteredValue",
                    start: GridCoordinate {
                        sheet_id,
                        row_index: grid_data.start_row,
                        column_index: grid_data.start_column,
                    },
                })
            }
        }
    }

    // remove the sheets that don't exist anymore
    for sheet_id in sheets_to_delete {
        requests.push(Request::DeleteSheet { sheet_id });
    }

    // construct the final request body
    let request_body = json!({
        "requests": requests,
        "includeSpreadsheetInResponse": true,
        "responseIncludeGridData": false,
    });

    // Write request body to file for debugging
    if let Ok(mut file) = std::fs::File::create("google_sheets_request.json") {
        if let Err(e) = serde_json::to_writer_pretty(&mut file, &request_body) {
            warn!("Failed to write Google Sheets request to file: {}", e);
        }
    }

    let url = format!("{ENDPOINT_SPREADSHEETS}/{spreadsheet_id}:batchUpdate");

    let response = ureq::post(&url)
        .set(AUTHORIZATION.as_str(), format!("Bearer {}", creds.access_token().secret()).as_str())
        .send_json(&request_body);
    let successful_response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(status_code, _)) if status_code == StatusCode::UNAUTHORIZED => {
            return Err(TryWithCredentialsError::Unauthorized(anyhow!(
                "request to update spreadsheet was unauthorized with status code: {}",
                status_code
            )));
        }
        Err(err) => {
            return Err(TryWithCredentialsError::Other(anyhow!(
                "request to update spreadsheet failed: {}",
                // err,
                err.into_response().unwrap().into_string().unwrap(),
            )));
        }
    };

    #[derive(Deserialize)]
    struct ApiResponse {
        replies: serde_json::Value,
        #[serde(rename = "updatedSpreadsheet")]
        updated_spreadsheet: Option<Spreadsheet>,
    }
    let response_content: ApiResponse =
        successful_response.into_json().map_err(anyhow::Error::from)?;
    trace!("Received replies to updating sheet: {}", response_content.replies);

    let url = 'url: {
        if let Some(updated_spreadsheet) = response_content.updated_spreadsheet {
            if let Some(spreadsheet_url) = updated_spreadsheet.spreadsheet_url {
                break 'url spreadsheet_url;
            }
        }
        warn!(
            "No URL returned in response to updating sheet. Inferring URL from spreadsheet ID and a hardcoded pattern"
        );
        format!(
            "https://docs.google.com/spreadsheets/d/{spreadsheet_id}/edit",
            spreadsheet_id = spreadsheet_id
        )
    };
    info!("Updated Google Sheet at {}", url);
    Ok(url)
}

// Either create or update a spreadsheet based on whether a spreadsheet ID is
// provided. Then open the spreadsheet in the browser.
pub fn write_spreadsheet(
    spreadsheet_id: Option<&str>,
    spreadsheet: Spreadsheet,
) -> anyhow::Result<String> {
    let (id, url) = run_with_credentials(|token| {
        let spreadsheet = spreadsheet.clone();
        if let Some(spreadsheet_id) = spreadsheet_id {
            update_spreadsheet(&token, spreadsheet_id, spreadsheet)
                .map(|url| (spreadsheet_id.to_owned(), url))
        } else {
            create_spreadsheet(&token, spreadsheet)
        }
    })?;
    utils::open_url(url.as_str());
    Ok(id)
}
