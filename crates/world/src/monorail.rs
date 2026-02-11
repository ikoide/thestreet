use crate::map::{STREET_CIRCUMFERENCE_TILES, STREET_HEIGHT};

pub const TRACK_ROWS: [i32; 2] = [STREET_HEIGHT / 2 - 1, STREET_HEIGHT / 2];
pub const STATION_DOOR_Y_TOP: i32 = TRACK_ROWS[0] - 1;
pub const STATION_DOOR_Y_BOTTOM: i32 = TRACK_ROWS[1] + 1;

pub const STATION_LABELS: [&str; 4] = ["north", "east", "south", "west"];

pub fn station_positions() -> [i64; 4] {
    let quarter = STREET_CIRCUMFERENCE_TILES / 4;
    [0, quarter, quarter * 2, quarter * 3]
}

pub fn station_label_for_x(station_x: i64) -> Option<&'static str> {
    let positions = station_positions();
    for (idx, pos) in positions.iter().enumerate() {
        if *pos == station_x {
            return Some(STATION_LABELS[idx]);
        }
    }
    None
}

pub fn station_x_for_label(label: &str) -> Option<i64> {
    let positions = station_positions();
    for (idx, pos) in positions.iter().enumerate() {
        if label.eq_ignore_ascii_case(STATION_LABELS[idx]) {
            return Some(*pos);
        }
    }
    None
}

pub fn is_track_row(y: i32) -> bool {
    TRACK_ROWS.contains(&y)
}

pub fn is_station_x(x: i32) -> bool {
    let positions = station_positions();
    let circumference = STREET_CIRCUMFERENCE_TILES;
    let wrapped = (x as i64).rem_euclid(circumference);
    positions.iter().any(|pos| *pos == wrapped)
}

pub fn station_x_for_coord(x: i32) -> Option<i64> {
    if !is_station_x(x) {
        return None;
    }
    let circumference = STREET_CIRCUMFERENCE_TILES;
    let wrapped = (x as i64).rem_euclid(circumference);
    Some(wrapped)
}

pub fn is_station_door(x: i32, y: i32) -> bool {
    (y == STATION_DOOR_Y_TOP || y == STATION_DOOR_Y_BOTTOM) && is_station_x(x)
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
