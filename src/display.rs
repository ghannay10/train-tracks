use chrono::{DateTime, Utc};
use colored::*;
use std::collections::HashMap;

use crate::api::Journey;
use crate::util::{calculate_change_time, format_iso_time, truncate};

pub const TOTAL_WIDTH: usize = 100;

fn print_row(
    dep: &str,
    arr: &str,
    duration: &str,
    status: ColoredString,
    platform: &str,
    operator: &str,
    from: &str,
    terminus: &str,
    dimmed: bool,
) {
    let row = format!(
        "{:6} {:6} {:10} {} {:8} {:12} {:20} {:20}",
        dep,
        arr,
        duration,
        status,
        platform.blue(),
        operator,
        truncate(from, 20),
        truncate(terminus, 20)
    );
    if dimmed {
        println!("{}", row.dimmed());
    } else {
        println!("{}", row);
    }
}

fn format_status(status: &str, is_bus: bool) -> ColoredString {
    let padded = format!("{:10}", status);
    if is_bus {
        padded.magenta()
    } else {
        match status {
            "Departed" => padded.blue(),
            "On time" => padded.green(),
            "Cancelled" | "Delayed" => padded.red(),
            s if s.starts_with("Exp") => padded.red(),
            _ => padded.yellow(),
        }
    }
}

fn print_divider(dimmed: bool) {
    let line = "-".repeat(TOTAL_WIDTH);
    if dimmed {
        println!("{}", line.dimmed());
    } else {
        println!("{}", line);
    }
}

pub fn print_header(origin: &str, destination: &str, station_lookup: &HashMap<String, String>) {
    println!(
        "{} -> {}",
        station_lookup
            .get(&origin.to_uppercase())
            .unwrap_or(&origin.to_string()),
        station_lookup
            .get(&destination.to_uppercase())
            .unwrap_or(&destination.to_string()),
    );
    println!("{}", "=".repeat(TOTAL_WIDTH));
    println!(
        "{:6} {:6} {:10} {:10} {:8} {:12} {:20} {:20}",
        "Dep", "Arr", "Duration", "Status", "Platform", "Operator", "From", "Terminus"
    );
    println!("{}", "-".repeat(TOTAL_WIDTH));
}

pub async fn print_journey(journey: &Journey, station_lookup: &HashMap<String, String>) {
    for (i, leg) in journey.legs.iter().enumerate() {
        let scheduled_dep = &leg.timetable.scheduled.departure;
        let scheduled_arr = &leg.timetable.scheduled.arrival;

        let realtime_dep = leg
            .timetable
            .realtime
            .departure
            .as_ref()
            .unwrap_or(scheduled_dep);

        let dep_formatted = format_iso_time(scheduled_dep);
        let arr_formatted = format_iso_time(scheduled_arr);

        let duration_str = if i == 0 {
            journey.duration.clone()
        } else {
            let prev_leg = &journey.legs[i - 1];
            let prev_arr = prev_leg
                .timetable
                .realtime
                .arrival
                .as_ref()
                .unwrap_or(&prev_leg.timetable.scheduled.arrival);
            if let Some(change_mins) = calculate_change_time(prev_arr, realtime_dep) {
                format!("({}m chg)", change_mins)
            } else {
                String::new()
            }
        };

        let has_departed = if let Ok(dep) = DateTime::parse_from_rfc3339(realtime_dep) {
            dep.with_timezone(&Utc) < Utc::now()
        } else {
            false
        };

        let is_late = realtime_dep != scheduled_dep || leg.delay_in_minutes > 0;

        let status = if leg.status == "Cancelled" {
            "Cancelled".to_string()
        } else if has_departed {
            "Departed".to_string()
        } else if leg.is_replacement_bus {
            "Bus".to_string()
        } else if is_late {
            // show exected if time given, otherwise just show delay
            if realtime_dep != scheduled_dep {
                format!("Exp {}", format_iso_time(realtime_dep))
            } else if leg.delay_in_minutes > 0 {
                format!("Exp +{}m", leg.delay_in_minutes)
            } else {
                "Delayed".to_string()
            }
        } else {
            match leg.status.as_str() {
                "OnTime" => "On time".to_string(),
                "Unknown" => "-".to_string(),
                _ => leg.status.clone(),
            }
        };

        let platform = leg.origin_platform.as_deref().unwrap_or("-");
        let operator = if leg.is_replacement_bus {
            "Rail Repl.".to_string()
        } else {
            leg.operator
                .as_ref()
                .map(|o| o.code.clone())
                .unwrap_or_else(|| "---".to_string())
        };
        let from_station = &leg.board.name;
        let terminus = leg
            .destinations
            .first()
            .map(|crs| {
                station_lookup
                    .get(crs)
                    .cloned()
                    .unwrap_or_else(|| crs.clone())
            })
            .unwrap_or_else(|| leg.alight.name.clone());

        let status_colored = format_status(&status, leg.is_replacement_bus);
        print_row(
            &dep_formatted,
            &arr_formatted,
            &duration_str,
            status_colored,
            platform,
            &operator,
            from_station,
            &terminus,
            has_departed,
        );
    }

    let last_departed = journey.legs.last().map_or(false, |leg| {
        let dep_time = leg
            .timetable
            .realtime
            .departure
            .as_ref()
            .unwrap_or(&leg.timetable.scheduled.departure);
        DateTime::parse_from_rfc3339(dep_time)
            .map(|dt| dt.with_timezone(&Utc) < Utc::now())
            .unwrap_or(false)
    });
    print_divider(last_departed);
}
