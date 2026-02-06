use chrono::{DateTime, Duration, Utc};
use std::collections::{HashMap, HashSet};
use std::error::Error;

use crate::api::{fetch_journeys, fetch_station_name, Journey};
use crate::display::{print_header, print_journey};

pub async fn fetch_and_display_journeys(
    origin: &str,
    destination: &str,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> Result<usize, Box<dyn Error>> {
    let now = Utc::now();
    let mut is_past = start_time < now;

    let mut seen: HashSet<String> = HashSet::new();
    let mut stations: HashMap<String, String> = HashMap::new();
    let mut batch: Vec<(String, Journey)> = Vec::new();
    let mut count = 0;
    let mut header_printed = false;

    let mut query_time = if is_past {
        now.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    } else {
        start_time.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    };

    loop {
        let mode = if is_past { "ARRIVE" } else { "DEPART" };
        let response = fetch_journeys(origin, destination, &query_time, mode).await?;

        if !header_printed {
            stations = response.station_lookup;
            print_header(origin, destination, &stations);
            header_printed = true;
        }

        if response.outward_journeys.is_empty() {
            break;
        }

        let mut last_dep: Option<DateTime<Utc>> = None;

        for journey in response.outward_journeys {
            if let Some((sort_time, dep_utc)) =
                process_journey(&journey, &mut seen, &mut stations, start_time, end_time).await
            {
                last_dep = Some(dep_utc);
                batch.push((sort_time, journey));
            }
        }

        // On first loop if start time is in the past, query from now to get any recently departed journeys.
        // This is because the API doesn't allow querying in the past, but we still want to show recently departed journeys if user queries from a past time.
        if is_past {
            is_past = false;
            query_time = (now - Duration::minutes(20))
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string();
            continue;
        }

        // Check if we need to paginate
        let Some(dep) = last_dep else {
            print_batch(&mut batch, None, &stations, &mut count).await;
            break;
        };
        if dep >= end_time {
            print_batch(&mut batch, None, &stations, &mut count).await;
            break;
        }

        query_time = (dep + Duration::minutes(1))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();

        // API doesnt always return in perfect order. Peek at next batch to determine if we can print current batch or need to wait for next page
        // makes sure printed in order.
        let next_batch = fetch_journeys(origin, destination, &query_time, "DEPART").await?;
        let min_next_departure_time = next_batch
            .outward_journeys
            .iter()
            .filter_map(|j| j.legs.first())
            .filter_map(|l| DateTime::parse_from_rfc3339(&l.timetable.scheduled.departure).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .min();

        print_batch(&mut batch, min_next_departure_time, &stations, &mut count).await;

        let mut last_departure_time = None;
        for journey in next_batch.outward_journeys {
            if let Some((sort_time, dep_utc)) =
                process_journey(&journey, &mut seen, &mut stations, start_time, end_time).await
            {
                last_departure_time = Some(dep_utc);
                batch.push((sort_time, journey));
            }
        }

        if let Some(dep) = last_departure_time {
            query_time = (dep + Duration::minutes(1))
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string();
        }
    }

    Ok(count)
}

async fn process_journey(
    journey: &Journey,
    seen: &mut HashSet<String>,
    stations: &mut HashMap<String, String>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Option<(String, DateTime<Utc>)> {
    let leg = journey.legs.first()?;
    let dep_str = &leg.timetable.scheduled.departure;
    let sig = format!("{}_{}", dep_str, journey.duration);

    if seen.contains(&sig) {
        return None;
    }
    seen.insert(sig);

    let dep_utc = DateTime::parse_from_rfc3339(dep_str)
        .ok()?
        .with_timezone(&Utc);

    if dep_utc < start || dep_utc > end {
        return None;
    }

    // Lookup unknown terminus stations. e.g PAD to Paddington
    for l in &journey.legs {
        for crs in &l.destinations {
            if !stations.contains_key(crs) {
                if let Ok(Some(name)) = fetch_station_name(crs).await {
                    stations.insert(crs.clone(), name);
                }
            }
        }
    }

    let sort_time = leg
        .timetable
        .realtime
        .departure
        .as_ref()
        .unwrap_or(dep_str)
        .clone();
    Some((sort_time, dep_utc))
}

async fn print_batch(
    batch: &mut Vec<(String, Journey)>,
    min_next: Option<DateTime<Utc>>,
    stations: &HashMap<String, String>,
    count: &mut usize,
) {
    batch.sort_by(|a, b| a.0.cmp(&b.0));
    let mut saved_for_next_batch = Vec::new();

    for (sort_time, journey) in batch.drain(..) {
        let departs_before_next_batch =
            match (DateTime::parse_from_rfc3339(&sort_time).ok(), min_next) {
                (Some(dep), Some(min)) => dep.with_timezone(&Utc) < min,
                _ => true,
            };

        if departs_before_next_batch {
            print_journey(&journey, stations).await;
            *count += 1;
        } else {
            saved_for_next_batch.push((sort_time, journey));
        }
    }

    *batch = saved_for_next_batch;
}
