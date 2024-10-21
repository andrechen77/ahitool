use tracing::info;

use crate::{apis::{self, google_maps::LatLng, google_sheets::{self, spreadsheet::{CellData, ExtendedValue, GridData, GridProperties, RowData, Sheet, SheetProperties, Spreadsheet, SpreadsheetProperties}, SheetNickname}, job_nimbus}, jobs::Job, utils};

#[derive(clap::Args, Debug)]
pub struct Args {
	// #[arg(long)]
	// key: String,
}

pub fn main(jn_api_key: &str, args: Args) -> anyhow::Result<()> {
	let Args { /* key: maps_api_key */ } = args;

	let jobs = job_nimbus::get_all_jobs_from_job_nimbus(jn_api_key, None)?;

	let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
	// let client = reqwest::Client::new();

	let mut address_data = Vec::new();
	for job in jobs {
		match job.geo {
			Some((lat, lng)) => {
				address_data.push((
					job,
					(lat, lng)
				));
			}
			None => {
				info!("Job {} has no coordinates", job.jnid);
			}
		}
	}

	fn mk_row(job: &Job, lat: f64, lng: f64) -> RowData {
		RowData {
			values: vec![
				CellData { user_entered_value: Some(ExtendedValue::StringValue(job.job_number.as_ref().cloned().unwrap_or("".to_owned()))) },
				CellData { user_entered_value: Some(ExtendedValue::StringValue(format!("https://app.jobnimbus.com/job/{}", job.jnid))) },
				CellData { user_entered_value: Some(ExtendedValue::NumberValue(lat)) },
				CellData { user_entered_value: Some(ExtendedValue::NumberValue(lng)) },
			],
		}
	}

	let mut rows = Vec::new();
	for (job, (lat, lng)) in address_data {
		rows.push(mk_row(&job, lat, lng));
	}
	let sheet = Sheet {
		properties: SheetProperties {
			title: Some("Job Locations".to_owned()),
			grid_properties: Some(GridProperties {
				row_count: rows.len() as i32 + 10,
				column_count: 100
			}),
			..Default::default()
		},
		data: Some(GridData { start_row: 1, start_column: 1, row_data: rows[0..950].to_owned() }),
		..Default::default()
	};
	let spreadsheet = Spreadsheet {
		properties: SpreadsheetProperties {
			title: Some("Job Locations".to_owned()),
		},
		sheets: Some(vec![sheet]),
		..Default::default()
	};

	// let url = runtime.block_on(google_sheets::run_with_credentials(|token| {
	// 	let token = token.clone();
	// 	let spreadsheet = &spreadsheet;
	// 	async move {
	// 		let spreadsheet = spreadsheet.clone();
	// 		google_sheets::create_spreadsheet(&token, SheetNickname::CoordsTemp, spreadsheet).await
	// 	}
	// }))?;
	// utils::open_url(url.as_str());

	Ok(())
}

