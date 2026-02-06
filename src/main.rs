use chrono::{DateTime, Duration, Utc};
use clap::Parser;
use std::error::Error;

use train_tracks::display::{print_header, print_journey};
use train_tracks::scheduler::get_journeys;
use train_tracks::util::parse_time_input;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    origin: String,
    destination: String,
    #[arg(short, long)]
    time: Option<String>,
    #[arg(short = 'H', long, default_value = "2")]
    hours: i64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let now = Utc::now();
    let start_time = args
        .time
        .map(|t| parse_time_input(&t))
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or(now);

    let end_time = start_time + Duration::hours(args.hours);

    let result = get_journeys(&args.origin, &args.destination, start_time, end_time).await?;

    print_header(&args.origin, &args.destination, &result.stations);
    for journey in &result.journeys {
        print_journey(journey, &result.stations).await;
    }
    println!("{} journeys found", result.journeys.len());

    Ok(())
}
