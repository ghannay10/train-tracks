use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct JourneyRequest {
    pub origin: Station,
    pub destination: Station,
    #[serde(rename = "outwardTime")]
    pub outward_time: OutwardTime,
}

#[derive(Debug, Serialize)]
pub struct Station {
    pub crs: String,
}

#[derive(Debug, Serialize)]
pub struct OutwardTime {
    #[serde(rename = "travelTime")]
    pub travel_time: String,
    #[serde(rename = "type")]
    pub time_type: String,
}

// Response - only fields we display

#[derive(Debug, Deserialize)]
pub struct JourneyResponse {
    #[serde(rename = "outwardJourneys")]
    pub outward_journeys: Vec<Journey>,
    #[serde(rename = "stationLookup")]
    pub station_lookup: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct Journey {
    pub duration: String,
    pub legs: Vec<Leg>,
}

#[derive(Debug, Deserialize)]
pub struct Leg {
    pub board: LegStation,
    pub alight: LegStation,
    #[serde(rename = "originPlatform")]
    pub origin_platform: Option<String>,
    pub status: String,
    pub operator: Option<Operator>,
    #[serde(rename = "isReplacementBus")]
    pub is_replacement_bus: bool,
    pub timetable: LegTimetable,
    #[serde(rename = "delayInMinutes")]
    pub delay_in_minutes: i32,
    #[serde(default)]
    pub destinations: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct LegStation {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Operator {
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct LegTimetable {
    pub scheduled: Times,
    pub realtime: OptionalTimes,
}

#[derive(Debug, Deserialize)]
pub struct Times {
    pub departure: String,
    pub arrival: String,
}

#[derive(Debug, Deserialize)]
pub struct OptionalTimes {
    pub departure: Option<String>,
    pub arrival: Option<String>,
}

// Station picker

#[derive(Debug, Deserialize)]
pub struct StationPickerResponse {
    pub payload: StationPickerPayload,
}

#[derive(Debug, Deserialize)]
pub struct StationPickerPayload {
    pub stations: Vec<StationInfo>,
}

#[derive(Debug, Deserialize)]
pub struct StationInfo {
    #[serde(rename = "crsCode")]
    pub crs_code: String,
    pub name: String,
}
