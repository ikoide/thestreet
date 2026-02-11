use crate::map::{STREET_CIRCUMFERENCE_TILES, STREET_HEIGHT};

pub const TRACK_ROWS: [i32; 2] = [STREET_HEIGHT / 2 - 1, STREET_HEIGHT / 2];
pub const STATION_DOOR_Y: i32 = TRACK_ROWS[0] - 1;

pub fn station_positions() -> [i64; 2] {
    [0, STREET_CIRCUMFERENCE_TILES / 2]
}

pub fn is_track_row(y: i32) -> bool {
    TRACK_ROWS.contains(&y)
}

pub fn is_station_x(x: i32) -> bool {
    let positions = station_positions();
    let circumference = STREET_CIRCUMFERENCE_TILES;
    let wrapped = (x as i64).rem_euclid(circumference);
    wrapped == positions[0] || wrapped == positions[1]
}

pub fn station_x_for_coord(x: i32) -> Option<i64> {
    if !is_station_x(x) {
        return None;
    }
    let positions = station_positions();
    let circumference = STREET_CIRCUMFERENCE_TILES;
    let wrapped = (x as i64).rem_euclid(circumference);
    if wrapped == positions[0] {
        Some(positions[0])
    } else if wrapped == positions[1] {
        Some(positions[1])
    } else {
        None
    }
}

pub fn is_station_door(x: i32, y: i32) -> bool {
    y == STATION_DOOR_Y && is_station_x(x)
}

pub fn station_map_id(station_x: i64) -> String {
    format!("station/{station_x}")
}

pub fn parse_station_map_id(map_id: &str) -> Option<i64> {
    let station_id = map_id.strip_prefix("station/")?;
    station_id.parse::<i64>().ok()
}

pub fn train_map_id(train_id: u32) -> String {
    format!("train/{train_id}")
}

pub fn parse_train_map_id(map_id: &str) -> Option<u32> {
    let train_id = map_id.strip_prefix("train/")?;
    train_id.parse::<u32>().ok()
}
