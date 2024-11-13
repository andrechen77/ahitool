use std::path::Path;

use anyhow::bail;
use reqwest::{self, header::CONTENT_TYPE, Response};
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

pub async fn get_api_key(new_api_key: Option<String>) -> Result<String, GetApiKeyError> {
    let cache_file = Path::new(DEFAULT_CACHE_FILE);

    if let Some(new_api_key) = new_api_key {
        if let Err(err) = tokio::fs::write(cache_file, &new_api_key).await {
            warn!("failed to cache new API key in file: {}", err);
        }
        Ok(new_api_key)
    } else if cache_file.exists() {
        Ok(tokio::fs::read_to_string(cache_file).await?)
    } else {
        Err(GetApiKeyError::MissingApiKey)
    }
}

const ENDPOINT_JOBS: &str = "https://app.jobnimbus.com/api1/jobs";

async fn request_from_job_nimbus(
    client: reqwest::Client,
    api_key: &str,
    num_jobs: usize,
    filter: Option<&str>,
) -> anyhow::Result<Response> {
    let url = reqwest::Url::parse(ENDPOINT_JOBS)?;
    let mut request = client
        .get(url.clone())
        .bearer_auth(&api_key)
        .header(CONTENT_TYPE, "application/json")
        .query(&[("size", num_jobs.to_string().as_str())]);
    if let Some(filter) = filter {
        request = request.query(&[("filter", filter)]);
    }
    let response = request.send().await?;
    if !response.status().is_success() {
        bail!("Request failed with status code: {}", response.status());
    }
    Ok(response)
}

// blocking
pub async fn get_all_jobs_from_job_nimbus(
    client: reqwest::Client,
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
    let response = request_from_job_nimbus(client.clone(), api_key, 1, filter).await?;
    let response: ApiResponse = response.json().await?;
    let count = response.count as usize;

    info!("detected {} jobs in JobNimbus", count);

    // make a request to actually get those jobs
    let response = request_from_job_nimbus(client.clone(), api_key, count, filter).await?;
    let response: ApiResponse = response.json().await?;
    info!("recieved {} jobs from JobNimbus", response.count);
    assert_eq!(response.count as usize, count);

    Ok(response.results.into_iter().filter_map(|v| {
        Job::try_from(v).inspect_err(|err| warn!("error deserializing job: {}", err)).ok()
    }))
}
