# Train Tracks

Rust CLI that hits the Realtime Trains API (https://www.realtimetrains.co.uk/) and displays the departure board for stations in the UK and to specific destinations if desired

## Prerequisites

- Rust installed on your system.

## Installation

```bash
git clone https://github.com/ghannay10/train-tracks
cd ./train-tracks
cargo install --path .
```

## API sign-up

Sign up to the Realtime Trains API (https://api.rtt.io/) and store your API username and password

```
export RTT_USERNAME=
export RTT_PASSWORD=
```

## Usage

- See departures from a station: `train-tracks <STATION CODE>`
- See departures to a destination: `train-tracks <ORIGIN> <DESTINATION>`
