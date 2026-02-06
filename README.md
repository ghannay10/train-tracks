# train-tracks

CLI tool for checking train times between UK stations. Uses the National Rail journey planner API.

Shows scheduled times, realtime delays, platform info, and connecting trains.

## Install

```bash
git clone https://github.com/ghannay10/train-tracks
cd train-tracks
cargo install --path .
```

## Usage

```bash
# Journeys from Paddington to Newbury, next 2 hours
train-tracks PAD NBY

# Specify a start time
train-tracks PAD NBY -t 14:00

# Look further ahead
train-tracks PAD NBY -H 4
```

Station codes are the 3-letter CRS codes (PAD, NBY, BRI, etc).

## What it shows

- Departure/arrival times (scheduled)
- Journey duration
- Status (on time, delayed, cancelled, departed)
- Platform
- Operator
- Connecting trains with change times

Departed trains show dimmed. Delays show in red with expected time.
