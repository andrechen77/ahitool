use http::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use tracing::{info, warn};

use crate::jobs::Job;

pub const DEFAULT_CACHE_FILE: &str = "job_nimbus_api_key.txt";

const ENDPOINT_JOBS: &str = "https://app.jobnimbus.com/api1/jobs";

fn request_from_job_nimbus(
    api_key: &str,
    num_jobs: usize,
    filter: Option<&str>,
) -> anyhow::Result<ureq::Response> {
    let mut request = ureq::get(ENDPOINT_JOBS)
        .set(AUTHORIZATION.as_str(), format!("Bearer {}", api_key).as_str())
        .set(CONTENT_TYPE.as_str(), "application/json")
        .query("size", num_jobs.to_string().as_str());
    if let Some(filter) = filter {
        request = request.query("filter", filter);
    }

    let response = request.call()?;

    Ok(response)
}

pub fn get_all_jobs_from_job_nimbus(
    api_key: &str,
    filter: Option<&str>,
) -> anyhow::Result<impl Iterator<Item = Job>> {
    use serde_json::Value;
    #[derive(Deserialize)]
    struct ApiResponse {
        count: u64,
        results: Vec<Value>,
    }

    info!("Getting all jobs from JobNimbus");

    // make a request to find out the number of jobs
    let response = request_from_job_nimbus(api_key, 1, filter)?;
    let response: ApiResponse = response.into_json()?;
    let count = response.count as usize;

    info!("Detected {} jobs in JobNimbus", count);

    // make a request to actually get those jobs
    let response = request_from_job_nimbus(api_key, count, filter)?;
    let response: ApiResponse = response.into_json()?;
    info!("Recieved {} jobs from JobNimbus", response.count);
    assert_eq!(response.count as usize, count);

    Ok(response.results.into_iter().filter_map(|v| {
        Job::try_from(v).inspect_err(|err| warn!("Error deserializing job: {}", err)).ok()
    }))
}
