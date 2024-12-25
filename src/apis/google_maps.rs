use anyhow::anyhow;
use http::{header::CONTENT_TYPE, StatusCode};
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;
use tracing::trace;

const ENDPOINT_GOOGLE_MAPS_PLACES: &str = "https://places.googleapis.com/v1/places:searchText";

#[derive(Error, Debug)]
pub enum LookupError {
    #[error("This request came too soon after a previous request, and we have been rate-limited")]
    TooFast,
    #[error("The address was not found")]
    NotFound,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub fn lookup(api_key: &str, address: &str) -> Result<LatLng, LookupError> {
    let url = ENDPOINT_GOOGLE_MAPS_PLACES;
    trace!("Sending request to look up address: {}", address);

    let response = ureq::post(&url)
        .query("key", api_key)
        .query("fields", "places.id,places.location,places.displayName")
        .set(CONTENT_TYPE.as_str(), "application/json")
        .send_json(&json!({
            "textQuery": address
        }));

    let successful_response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(status_code, _))
            if status_code == StatusCode::TOO_MANY_REQUESTS =>
        {
            return Err(LookupError::TooFast);
        }
        Err(ureq::Error::Status(status_code, _)) => {
            return Err(LookupError::Other(anyhow!(
                "Request failed with status code: {}",
                status_code
            )));
        }
        Err(err) => {
            return Err(LookupError::Other(err.into()));
        }
    };

    #[derive(Deserialize)]
    struct ApiResponse {
        places: Vec<Place>,
    }

    let response: serde_json::Value =
        successful_response.into_json().map_err(anyhow::Error::from)?;
    trace!("received response: {}", response);
    let response: ApiResponse = serde_json::from_value(response).map_err(anyhow::Error::from)?;

    if let Some(place) = response.places.into_iter().next() {
        let Place { location, .. } = place;
        Ok(location)
    } else {
        Err(LookupError::NotFound)
    }
}

#[derive(Deserialize)]
struct Place {
    #[allow(dead_code)]
    pub id: String,
    pub location: LatLng,
}

#[derive(Deserialize)]
pub struct LatLng {
    pub latitude: f64,
    pub longitude: f64,
}
