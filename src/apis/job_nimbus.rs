use std::path::Path;

use http::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use tracing::{info, warn};

use crate::jobs::Job;

const DEFAULT_CACHE_FILE: &str = "job_nimbus_api_key.txt";

#[derive(Debug, thiserror::Error)]
pub enum GetApiKeyError {
    #[error("JobNimbus API key was not specified and the cache file does not exist.")]
    MissingApiKey,
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

pub fn get_api_key(new_api_key: Option<String>) -> Result<String, GetApiKeyError> {
    let cache_file = Path::new(DEFAULT_CACHE_FILE);

    if let Some(new_api_key) = new_api_key {
        if let Err(err) = std::fs::write(cache_file, &new_api_key) {
            warn!("failed to cache new API key in file: {}", err);
        } else {
            info!("cached new API key in file");
        }
        Ok(new_api_key)
    } else if cache_file.exists() {
        let api_key = std::fs::read_to_string(cache_file)?;
        info!("loaded API key from cache file");
        Ok(api_key)
    } else {
        Err(GetApiKeyError::MissingApiKey)
    }
}

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

    info!("getting all jobs from JobNimbus");

    // make a request to find out the number of jobs
    let response = request_from_job_nimbus(api_key, 1, filter)?;
    let response: ApiResponse = response.into_json()?;
    let count = response.count as usize;

    info!("detected {} jobs in JobNimbus", count);

    // make a request to actually get those jobs
    let response = request_from_job_nimbus(api_key, count, filter)?;
    let response: ApiResponse = response.into_json()?;
    info!("recieved {} jobs from JobNimbus", response.count);
    assert_eq!(response.count as usize, count);

    Ok(response.results.into_iter().filter_map(|v| {
        Job::try_from(v).inspect_err(|err| warn!("error deserializing job: {}", err)).ok()
    }))
}
