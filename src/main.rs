use base64::{engine::general_purpose, Engine as _};
use clap::Parser;
use colored::*;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::Deserialize;
use std::env;
use std::error::Error;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Origin station code (e.g., PAD for Paddington)
    origin: String,

    /// Optional destination station code (e.g., BRI for Bristol)
    destination: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Station {
    description: String,
    #[serde(rename = "publicTime")]
    public_time: String,
}

#[derive(Debug, Deserialize)]
struct ServiceLocation {
    #[serde(rename = "gbttBookedDeparture")]
    departure: Option<String>,
    platform: Option<String>,
    #[serde(rename = "realtimeDeparture")]
    realtime_departure: Option<String>,
    destination: Vec<Station>,
    origin: Vec<Station>,
    #[serde(rename = "displayAs")]
    display_as: String,
}

#[derive(Debug, Deserialize)]
struct Service {
    #[serde(rename = "locationDetail")]
    location: ServiceLocation,
    #[serde(rename = "serviceUid")]
    service_uid: String,
    #[serde(rename = "runDate")]
    run_date: String,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    services: Vec<Service>,
}

// Structs for service detail endpoint
#[derive(Debug, Deserialize)]
struct ServiceDetailLocation {
    crs: Option<String>,
    #[serde(rename = "gbttBookedArrival")]
    booked_arrival: Option<String>,
    #[serde(rename = "realtimeArrival")]
    realtime_arrival: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ServiceDetailResponse {
    locations: Vec<ServiceDetailLocation>,
}

async fn fetch_services(
    origin: &str,
    destination: Option<&str>,
    username: &str,
    password: &str,
) -> Result<Vec<Service>, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let url = format!("https://api.rtt.io/api/v1/json/search/{}", origin);

    let url = if let Some(destination) = destination {
        format!("{}/to/{}", url, destination)
    } else {
        url
    };

    let mut headers = HeaderMap::new();
    let auth = format!(
        "Basic {}",
        general_purpose::STANDARD.encode(format!("{}:{}", username, password))
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth)?);

    println!("curl -H 'Authorization: {}' '{}'", auth, url);

    let response = client
        .get(&url)
        .headers(headers)
        .send()
        .await?
        .json::<ApiResponse>()
        .await?;

    Ok(response.services)
}

async fn fetch_arrival_at_destination(
    service_uid: &str,
    run_date: &str,
    destination: &str,
    username: &str,
    password: &str,
) -> Result<Option<String>, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let date_parts: Vec<&str> = run_date.split('-').collect();
    let url = format!(
        "https://api.rtt.io/api/v1/json/service/{}/{}/{}/{}",
        service_uid, date_parts[0], date_parts[1], date_parts[2]
    );

    let mut headers = HeaderMap::new();
    let auth = format!(
        "Basic {}",
        general_purpose::STANDARD.encode(format!("{}:{}", username, password))
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth)?);

    let response = client
        .get(&url)
        .headers(headers)
        .send()
        .await?
        .json::<ServiceDetailResponse>()
        .await?;

    let dest_upper = destination.to_uppercase();
    for loc in response.locations {
        if let Some(crs) = &loc.crs {
            if crs.to_uppercase() == dest_upper {
                return Ok(loc.realtime_arrival.or(loc.booked_arrival));
            }
        }
    }

    Ok(None)
}

fn format_time(time: &Option<String>) -> String {
    time.as_ref()
        .map(|t| t.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

fn calculate_duration_minutes(departure: &str, arrival: &str) -> Option<i32> {
    if departure.len() != 4 || arrival.len() != 4 {
        return None;
    }

    let dep_hours: i32 = departure[0..2].parse().ok()?;
    let dep_mins: i32 = departure[2..4].parse().ok()?;
    let arr_hours: i32 = arrival[0..2].parse().ok()?;
    let arr_mins: i32 = arrival[2..4].parse().ok()?;

    let dep_total = dep_hours * 60 + dep_mins;
    let mut arr_total = arr_hours * 60 + arr_mins;

    // Handle overnight journeys (arrival is next day)
    if arr_total < dep_total {
        arr_total += 24 * 60;
    }

    Some(arr_total - dep_total)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let username = env::var("RTT_USERNAME").expect("RTT_USERNAME environment variable not set");
    let password = env::var("RTT_PASSWORD").expect("RTT_PASSWORD environment variable not set");

    let services = fetch_services(
        &args.origin,
        args.destination.as_deref(),
        &username,
        &password,
    )
    .await?;

    // print services
    println!("{} services found", services.len());

    const TOTAL_WIDTH: usize = 90;
    println!("{}", "=".repeat(TOTAL_WIDTH));

    println!(
        "{:6} {:6} {:10} {:10} {:12} {:20} {:20}",
        "Time", "Exp.", "Status", "Platform", "Arrival", "Origin", "Destination"
    );
    println!("{}", "-".repeat(TOTAL_WIDTH));

    for (_index, service) in services.iter().enumerate() {
        // println!("{:#?}", service);

        let departure_str = format_time(&service.location.departure);
        let expected_departure = format_time(&service.location.realtime_departure);

        let platform = format!(
            "{:10}",
            service.location.platform.as_deref().unwrap_or("TBA")
        )
        .blue();

        let destination = service
            .location
            .destination
            .first()
            .map(|d| d.description.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let origin = service
            .location
            .origin
            .first()
            .map(|d| d.description.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let arrival_time = if let Some(dest) = &args.destination {
            match fetch_arrival_at_destination(
                &service.service_uid,
                &service.run_date,
                dest,
                &username,
                &password,
            )
            .await
            {
                Ok(Some(arrival)) => Some(arrival),
                Ok(None) => None,
                Err(_) => None,
            }
        } else {
            service
                .location
                .destination
                .first()
                .map(|d| d.public_time.clone())
        };

        let exp_arrival = match &arrival_time {
            Some(arr) => {
                if let Some(duration) = calculate_duration_minutes(&departure_str, arr) {
                    format!("{} ({}m)", arr, duration)
                } else {
                    arr.clone()
                }
            }
            None => "N/A".to_string(),
        };

        let status_text = if service.location.display_as == "CANCELLED_CALL" {
            "Cancelled"
        } else if departure_str == expected_departure {
            "On time"
        } else if departure_str == "N/A" || expected_departure == "N/A" {
            "Unknown"
        } else {
            "Delayed"
        };

        let status = format!("{:10}", status_text);
        let status = match status_text {
            "On time" => status.green(),
            "Cancelled" => status.red(),
            "Delayed" => status.red(),
            _ => status.yellow(),
        };

        println!(
            "{:6} {:6} {} {} {:12} {:20} {:20}",
            departure_str, expected_departure, status, platform, exp_arrival, origin, destination
        );
    }

    Ok(())
}
