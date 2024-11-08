use chrono::{DateTime, Duration, Local};
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
    publicTime: String
}

#[derive(Debug, Deserialize)]
struct ServiceLocation {
    #[serde(rename = "gbttBookedDeparture")]
    departure: Option<String>,
    platform: Option<String>,
    #[serde(rename = "realtimeDeparture")]
    realtime_departure: Option<String>,
    destination: Vec<Station>,
    origin: Vec<Station>
}

#[derive(Debug, Deserialize)]
struct Service {
    #[serde(rename = "locationDetail")]
    location: ServiceLocation,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    services: Vec<Service>,
}

async fn fetch_services(
    origin: &str,
    destination: Option<&str>,
    username: &str,
    password: &str,
) -> Result<Vec<Service>, Box<dyn Error>> {
    let client = reqwest::Client::new();
    
    let now: DateTime<Local> = Local::now();
    let url = format!(
        "https://api.rtt.io/api/v1/json/search/{}",
        origin
    );

    // if destination is provided, append it to the URL
    let url = if let Some(destination) = destination {
        format!("{}/to/{}", url, destination)
    } else {
        url
    };

    let mut headers = HeaderMap::new();
    let auth = format!(
        "Basic {}",
        base64::encode(format!("{}:{}", username, password))
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth)?);

    let response = client
        .get(&url)
        .headers(headers)
        .send()
        .await?
        .json::<ApiResponse>()
        .await?;

    Ok(response.services)
}

fn format_time(time: &Option<String>) -> String {
    time.as_ref()
        .map(|t| t.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    
    let username = env::var("RTT_USERNAME")
        .expect("RTT_USERNAME environment variable not set");
    let password = env::var("RTT_PASSWORD")
        .expect("RTT_PASSWORD environment variable not set");

    let services = fetch_services(&args.origin, args.destination.as_deref(), &username, &password).await?;

    // print services
    println!("{} services found", services.len());

    const TOTAL_WIDTH: usize = 90;  // Adjust this based on your terminal width
    println!("{}", "=".repeat(TOTAL_WIDTH));

    println!(
        "{:6} {:6} {:10} {:10} {:8} {:20} {:20}",
        "Time", "Exp.", "Status", "Platform", "Arrival", "Origin", "Destination"
    );
    println!("{}", "-".repeat(TOTAL_WIDTH));


    for service in services {
        let departure_str = format_time(&service.location.departure);
        let expected_departure = if let Some(realtime_departure) = &service.location.realtime_departure {
            format_time(&service.location.realtime_departure)
        } else {
            "N/A".to_string()
        };
    
        let platform = service
            .location
            .platform
            .unwrap_or_else(|| "TBA".to_string())
            .blue()
            .to_string();
    
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
    
        let exp_arrival = service
            .location
            .destination
            .first()
            .map(|d| d.publicTime.clone())
            .unwrap_or_else(|| "Unknown".to_string());
    
        let status = if departure_str == expected_departure {
            "On time".green()
        } else if departure_str == "N/A" || expected_departure == "N/A" {
            "Unknown".yellow()
        } else {
            "Delayed".red()
        };
    
        println!(
            "{:6} {:6} {:10} {:20} {:8} {:20} {:20}",
            departure_str, expected_departure, status, platform, exp_arrival, origin, destination
        );
    }
    

    Ok(())
}