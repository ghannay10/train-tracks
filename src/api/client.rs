use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::error::Error;

use super::models::{
    JourneyRequest, JourneyResponse, OutwardTime, Station, StationPickerResponse,
};

pub async fn fetch_station_name(crs: &str) -> Result<Option<String>, Box<dyn Error>> {
    let client = reqwest::Client::new();

    let url = format!(
        "https://stationpicker.nationalrail.co.uk/stationPicker/{}",
        crs.to_lowercase()
    );

    let mut headers = HeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("*/*"));
    headers.insert(
        "User-Agent",
        HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:147.0) Gecko/20100101 Firefox/147.0",
        ),
    );
    headers.insert(
        "Referer",
        HeaderValue::from_static("https://www.nationalrail.co.uk/"),
    );
    headers.insert(
        "Origin",
        HeaderValue::from_static("https://www.nationalrail.co.uk"),
    );

    let response = client.get(&url).headers(headers).send().await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let result: StationPickerResponse = response.json().await?;

    // Find the station with matching CRS code
    let crs_upper = crs.to_uppercase();
    Ok(result
        .payload
        .stations
        .iter()
        .find(|s| s.crs_code.to_uppercase() == crs_upper)
        .map(|s| s.name.clone()))
}

pub async fn fetch_journeys(
    origin: &str,
    destination: &str,
    departure_time: &str,
    time_type: &str,
) -> Result<JourneyResponse, Box<dyn Error>> {
    let client = reqwest::Client::new();

    let request = JourneyRequest {
        origin: Station { crs: origin.to_uppercase() },
        destination: Station { crs: destination.to_uppercase() },
        outward_time: OutwardTime {
            travel_time: departure_time.to_string(),
            time_type: time_type.to_string(),
        },
    };

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("Accept", HeaderValue::from_static("application/json"));
    headers.insert(
        "User-Agent",
        HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:147.0) Gecko/20100101 Firefox/147.0",
        ),
    );
    headers.insert(
        "Referer",
        HeaderValue::from_static("https://www.nationalrail.co.uk/"),
    );
    headers.insert("X-JP-Platform", HeaderValue::from_static("web"));
    headers.insert(
        "Origin",
        HeaderValue::from_static("https://www.nationalrail.co.uk"),
    );

    let response = client
        .post("https://jpservices.nationalrail.co.uk/journey-planner")
        .headers(headers)
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, body).into());
    }

    let body = response.text().await?;
    if body.is_empty() {
        return Err("API returned empty response".into());
    }

    let parsed: JourneyResponse = serde_json::from_str(&body).map_err(|e| {
        format!(
            "JSON parse error: {} - Body: {}",
            e,
            &body[..body.len().min(200)]
        )
    })?;

    Ok(parsed)
}
