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

fn deserialize_services<'de, D>(deserializer: D) -> Result<Vec<Service>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<Vec<Service>> = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    #[serde(default, deserialize_with = "deserialize_services")]
    services: Vec<Service>,
}

#[derive(Debug, Deserialize, Clone)]
struct ServiceDetailLocation {
    crs: Option<String>,
    description: Option<String>,
    #[serde(rename = "gbttBookedArrival")]
    booked_arrival: Option<String>,
    #[serde(rename = "gbttBookedDeparture")]
    booked_departure: Option<String>,
    #[serde(rename = "realtimeArrival")]
    realtime_arrival: Option<String>,
    #[serde(rename = "realtimeDeparture")]
    realtime_departure: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ServiceDetailResponse {
    locations: Vec<ServiceDetailLocation>,
}

#[derive(Debug, Clone)]
struct Connection {
    leg1_departure: String,
    leg1_platform: Option<String>,
    leg1_arrival: String,
    interchange_station: String,
    interchange_crs: String,
    change_time: i32,
    leg2_departure: String,
    leg2_platform: Option<String>,
    leg2_arrival: String,
    total_duration: i32,
}

const MIN_CHANGE_TIME: i32 = 3;

#[derive(Debug, Clone)]
enum JourneyOption {
    Direct {
        departure: String,
        expected_departure: String,
        platform: Option<String>,
        arrival: String,
        origin: String,
        destination: String,
        status: String,
    },
    WithChange(Connection),
}

// Major interchange stations
const MAJOR_INTERCHANGES: &[&str] = &[
    "RDG", // Reading
    "BHM", // Birmingham New Street
    "BRI", // Bristol Temple Meads
    "MAN", // Manchester Piccadilly
    "LDS", // Leeds
    "EDB", // Edinburgh
    "GLC", // Glasgow Central
    "CRE", // Crewe
    "YRK", // York
    "NCL", // Newcastle
    "SHF", // Sheffield
    "NOT", // Nottingham
    "PBO", // Peterborough
    "CBG", // Cambridge
    "OXF", // Oxford
    "SWI", // Swindon
    "BTH", // Bath Spa
    "EXD", // Exeter St Davids
    "CLJ", // Clapham Junction
    "WIM", // Wimbledon
    "SAL", // Salisbury
    "SOT", // Southampton Central
    "BMS", // Bournemouth
    "PLY", // Plymouth
    "NWP", // Newport
    "CDF", // Cardiff Central
    "SWA", // Swansea
    "COV", // Coventry
    "WVH", // Wolverhampton
    "STP", // London St Pancras
    "EUS", // London Euston
    "KGX", // London King's Cross
    "LST", // London Liverpool Street
    "VIC", // London Victoria
    "WAT", // London Waterloo
    "PAD", // London Paddington
    "WOP",
];

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

async fn fetch_service_stops(
    service_uid: &str,
    run_date: &str,
    username: &str,
    password: &str,
) -> Result<Vec<ServiceDetailLocation>, Box<dyn Error>> {
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

    Ok(response.locations)
}

fn time_to_minutes(time: &str) -> Option<i32> {
    if time.len() != 4 {
        return None;
    }
    let hours: i32 = time[0..2].parse().ok()?;
    let mins: i32 = time[2..4].parse().ok()?;
    Some(hours * 60 + mins)
}

async fn find_connections(
    origin: &str,
    destination: &str,
    username: &str,
    password: &str,
) -> Result<Vec<Connection>, Box<dyn Error>> {
    let mut connections = Vec::new();

    let origin_services = fetch_services(origin, None, username, password).await?;
    let origin_services: Vec<_> = origin_services.into_iter().take(10).collect();

    for service in &origin_services {
        let stops =
            match fetch_service_stops(&service.service_uid, &service.run_date, username, password)
                .await
            {
                Ok(s) => s,
                Err(_) => continue,
            };

        let leg1_departure = match &service.location.departure {
            Some(d) => d.clone(),
            None => continue,
        };

        let origin_upper = origin.to_uppercase();
        let origin_idx = stops
            .iter()
            .position(|s| s.crs.as_ref().map(|c| c.to_uppercase()) == Some(origin_upper.clone()));

        if origin_idx.is_none() {
            continue;
        }
        let origin_idx = origin_idx.unwrap();

        let dest_upper = destination.to_uppercase();
        let destination_on_route = stops
            .iter()
            .skip(origin_idx + 1)
            .any(|s| s.crs.as_ref().map(|c| c.to_uppercase()) == Some(dest_upper.clone()));

        if destination_on_route {
            continue;
        }

        for stop in stops.iter().skip(origin_idx + 1) {
            let interchange_crs = match &stop.crs {
                Some(c) => c.clone(),
                None => continue,
            };

            let interchange_upper = interchange_crs.to_uppercase();

            if interchange_upper == destination.to_uppercase() {
                break;
            }

            if !MAJOR_INTERCHANGES.contains(&interchange_upper.as_str()) {
                continue;
            }

            let interchange_name = stop
                .description
                .clone()
                .unwrap_or_else(|| interchange_crs.clone());

            let leg1_arrival = stop
                .realtime_arrival
                .clone()
                .or_else(|| stop.booked_arrival.clone());

            let leg1_arrival = match leg1_arrival {
                Some(a) => a,
                None => continue,
            };

            let connecting_services =
                match fetch_services(&interchange_crs, Some(destination), username, password).await
                {
                    Ok(s) => s,
                    Err(_) => continue,
                };

            let leg1_arrival_mins = match time_to_minutes(&leg1_arrival) {
                Some(m) => m,
                None => continue,
            };

            for conn_service in connecting_services {
                let leg2_departure = match &conn_service.location.departure {
                    Some(d) => d.clone(),
                    None => continue,
                };

                let leg2_departure_mins = match time_to_minutes(&leg2_departure) {
                    Some(m) => m,
                    None => continue,
                };

                let leg2_departure_mins = if leg2_departure_mins < leg1_arrival_mins {
                    leg2_departure_mins + 24 * 60
                } else {
                    leg2_departure_mins
                };

                let change_time = leg2_departure_mins - leg1_arrival_mins;

                if change_time < MIN_CHANGE_TIME {
                    continue;
                }

                if change_time > 60 {
                    continue;
                }

                let leg2_arrival = match fetch_arrival_at_destination(
                    &conn_service.service_uid,
                    &conn_service.run_date,
                    destination,
                    username,
                    password,
                )
                .await
                {
                    Ok(Some(a)) => a,
                    _ => continue,
                };

                let leg1_dep_mins = match time_to_minutes(&leg1_departure) {
                    Some(m) => m,
                    None => continue,
                };
                let leg2_arr_mins = match time_to_minutes(&leg2_arrival) {
                    Some(m) => m,
                    None => continue,
                };

                let leg2_arr_mins = if leg2_arr_mins < leg1_dep_mins {
                    leg2_arr_mins + 24 * 60
                } else {
                    leg2_arr_mins
                };

                let total_duration = leg2_arr_mins - leg1_dep_mins;

                connections.push(Connection {
                    leg1_departure: leg1_departure.clone(),
                    leg1_platform: service.location.platform.clone(),
                    leg1_arrival: leg1_arrival.clone(),
                    interchange_station: interchange_name.clone(),
                    interchange_crs: interchange_crs.clone(),
                    change_time,
                    leg2_departure,
                    leg2_platform: conn_service.location.platform.clone(),
                    leg2_arrival,
                    total_duration,
                });

                break;
            }
        }
    }

    connections.sort_by_key(|c| c.total_duration);
    connections.dedup_by(|a, b| {
        a.leg1_departure == b.leg1_departure && a.interchange_crs == b.interchange_crs
    });

    Ok(connections)
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

    const TOTAL_WIDTH: usize = 90;

    let mut journey_options: Vec<JourneyOption> = Vec::new();
    let mut direct_trains: Vec<(i32, i32)> = Vec::new();

    for service in services.iter() {
        let departure_str = format_time(&service.location.departure);
        let expected_departure = format_time(&service.location.realtime_departure);

        let destination_name = service
            .location
            .destination
            .first()
            .map(|d| d.description.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let origin_name = service
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

        let arrival_with_duration = match &arrival_time {
            Some(arr) => {
                if let Some(duration) = calculate_duration_minutes(&departure_str, arr) {
                    format!("{} ({}m)", arr, duration)
                } else {
                    arr.clone()
                }
            }
            None => "N/A".to_string(),
        };

        if let (Some(dep_mins), Some(arr)) = (time_to_minutes(&departure_str), &arrival_time) {
            if let Some(arr_mins) = time_to_minutes(arr) {
                let arr_mins = if arr_mins < dep_mins {
                    arr_mins + 24 * 60
                } else {
                    arr_mins
                };
                direct_trains.push((dep_mins, arr_mins));
            }
        }

        let status_text = if service.location.display_as == "CANCELLED_CALL" {
            "Cancelled"
        } else if departure_str == expected_departure {
            "On time"
        } else if departure_str == "N/A" || expected_departure == "N/A" {
            "Unknown"
        } else {
            "Delayed"
        };

        journey_options.push(JourneyOption::Direct {
            departure: departure_str,
            expected_departure,
            platform: service.location.platform.clone(),
            arrival: arrival_with_duration,
            origin: origin_name,
            destination: destination_name,
            status: status_text.to_string(),
        });
    }

    if let Some(dest) = &args.destination {
        println!("Searching for connections...");
        let connections = find_connections(&args.origin, dest, &username, &password).await?;

        for conn in connections.iter() {
            let conn_dep = match time_to_minutes(&conn.leg1_departure) {
                Some(m) => m,
                None => continue,
            };
            let conn_arr = match time_to_minutes(&conn.leg2_arrival) {
                Some(m) => m,
                None => continue,
            };
            let conn_arr = if conn_arr < conn_dep {
                conn_arr + 24 * 60
            } else {
                conn_arr
            };

            let dominated_by_direct = direct_trains
                .iter()
                .any(|(d_dep, d_arr)| *d_dep >= conn_dep && *d_arr <= conn_arr);

            let dominated_by_connection = connections.iter().any(|other| {
                if std::ptr::eq(conn, other) {
                    return false;
                }
                let other_dep = match time_to_minutes(&other.leg1_departure) {
                    Some(m) => m,
                    None => return false,
                };
                let other_arr = match time_to_minutes(&other.leg2_arrival) {
                    Some(m) => m,
                    None => return false,
                };
                let other_arr = if other_arr < other_dep {
                    other_arr + 24 * 60
                } else {
                    other_arr
                };
                other_dep >= conn_dep && other_arr <= conn_arr
            });

            if !dominated_by_direct && !dominated_by_connection {
                journey_options.push(JourneyOption::WithChange(conn.clone()));
            }
        }
    }

    journey_options.sort_by_key(|opt| {
        let dep = match opt {
            JourneyOption::Direct { departure, .. } => departure.clone(),
            JourneyOption::WithChange(conn) => conn.leg1_departure.clone(),
        };
        time_to_minutes(&dep).unwrap_or(9999)
    });

    println!("{} journey options found", journey_options.len());
    println!("{}", "=".repeat(TOTAL_WIDTH));
    println!(
        "{:6} {:6} {:10} {:10} {:12} {:20} {:20}",
        "Time", "Exp.", "Status", "Platform", "Arrival", "Origin", "Terminus"
    );
    println!("{}", "-".repeat(TOTAL_WIDTH));

    for opt in &journey_options {
        match opt {
            JourneyOption::Direct {
                departure,
                expected_departure,
                platform,
                arrival,
                origin,
                destination,
                status,
            } => {
                let platform_fmt = format!("{:10}", platform.as_deref().unwrap_or("TBA")).blue();
                let status_fmt = format!("{:10}", status);
                let status_colored = match status.as_str() {
                    "On time" => status_fmt.green(),
                    "Cancelled" => status_fmt.red(),
                    "Delayed" => status_fmt.red(),
                    _ => status_fmt.yellow(),
                };

                println!(
                    "{:6} {:6} {} {} {:12} {:20} {:20}",
                    departure,
                    expected_departure,
                    status_colored,
                    platform_fmt,
                    arrival,
                    origin,
                    destination
                );
            }
            JourneyOption::WithChange(conn) => {
                let platform1 =
                    format!("{:10}", conn.leg1_platform.as_deref().unwrap_or("TBA")).blue();
                let status_fmt = format!("{:10}", "Change").yellow();

                println!(
                    "{:6} {:6} {} {} {:12} {:20} {:20}",
                    conn.leg1_departure,
                    conn.leg1_departure,
                    status_fmt,
                    platform1,
                    conn.leg1_arrival,
                    args.origin.to_uppercase(),
                    conn.interchange_station
                );

                let platform2 =
                    format!("{:10}", conn.leg2_platform.as_deref().unwrap_or("TBA")).blue();
                let change_info = format!("({}m chg)", conn.change_time);
                let arrival_with_duration = if let Some(duration) =
                    calculate_duration_minutes(&conn.leg1_departure, &conn.leg2_arrival)
                {
                    format!("{} ({}m)", conn.leg2_arrival, duration)
                } else {
                    conn.leg2_arrival.clone()
                };

                println!(
                    "{:6} {:6} {:10} {} {:12} {:20} {:20}",
                    "",
                    "",
                    "",
                    platform2,
                    arrival_with_duration,
                    format!("{} {}", conn.interchange_station, change_info),
                    args.destination
                        .as_ref()
                        .map(|d| d.to_uppercase())
                        .unwrap_or_default()
                );
            }
        }
    }

    Ok(())
}
