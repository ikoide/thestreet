use serde::{Deserialize, Serialize};

use crate::doors::{
    parse_room_map_id, room_entry_position, room_id_for_door, room_map_id, street_door_side,
    street_entry_position, RoomSide,
};
use crate::map::{
    room_tile, station_entry_position, station_tile, street_tile, train_tile, Position, Tile,
    ROOM_HEIGHT, ROOM_WIDTH, STATION_HEIGHT, STATION_WIDTH, STREET_HEIGHT, TRAIN_HEIGHT,
    TRAIN_WIDTH,
};
use crate::monorail::{
    parse_station_map_id, parse_train_map_id, station_map_id, station_x_for_coord,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveOutcome {
    Moved(Position),
    Transition(Position),
    Blocked,
}

pub fn step(x: i32, y: i32, dir: Direction) -> (i32, i32) {
    match dir {
        Direction::Up => (x, y - 1),
        Direction::Down => (x, y + 1),
        Direction::Left => (x - 1, y),
        Direction::Right => (x + 1, y),
    }
}

pub fn try_move(position: &Position, dir: Direction) -> MoveOutcome {
    if position.map_id == "street" {
        move_on_street(position, dir)
    } else if let Some((side, street_x)) = parse_room_map_id(&position.map_id) {
        move_in_room(position, dir, side, street_x)
    } else if let Some(station_x) = parse_station_map_id(&position.map_id) {
        move_in_station(position, dir, station_x)
    } else if parse_train_map_id(&position.map_id).is_some() {
        move_in_train(position, dir)
    } else {
        MoveOutcome::Blocked
    }
}

fn move_on_street(position: &Position, dir: Direction) -> MoveOutcome {
    let (nx, ny) = step(position.x, position.y, dir);
    if ny < 0 || ny >= STREET_HEIGHT {
        return MoveOutcome::Blocked;
    }
    match street_tile(nx, ny) {
        Tile::Wall | Tile::Customizer => MoveOutcome::Blocked,
        Tile::Door => {
            if let Some(side) = street_door_side(nx, ny) {
                let room_id = room_id_for_door(side, nx);
                let map_id = room_map_id(&room_id);
                let (rx, ry) = room_entry_position(side);
                MoveOutcome::Transition(Position {
                    map_id,
                    x: rx,
                    y: ry,
                })
            } else {
                MoveOutcome::Blocked
            }
        }
        Tile::StationDoor => {
            let station_x = match station_x_for_coord(nx) {
                Some(value) => value,
                None => return MoveOutcome::Blocked,
            };
            let map_id = station_map_id(station_x);
            let (sx, sy) = station_entry_position();
            MoveOutcome::Transition(Position {
                map_id,
                x: sx,
                y: sy,
            })
        }
        Tile::Floor => MoveOutcome::Moved(Position {
            map_id: position.map_id.clone(),
            x: nx,
            y: ny,
        }),
    }
}

fn move_in_room(position: &Position, dir: Direction, side: RoomSide, street_x: i32) -> MoveOutcome {
    let (nx, ny) = step(position.x, position.y, dir);
    if nx < 0 || nx >= ROOM_WIDTH || ny < 0 || ny >= ROOM_HEIGHT {
        return MoveOutcome::Blocked;
    }
    match room_tile(nx, ny, side) {
        Tile::Wall | Tile::Customizer => MoveOutcome::Blocked,
        Tile::Door => {
            let (sx, sy) = street_entry_position(side, street_x);
            MoveOutcome::Transition(Position {
                map_id: "street".to_string(),
                x: sx,
                y: sy,
            })
        }
        Tile::Floor | Tile::StationDoor => MoveOutcome::Moved(Position {
            map_id: position.map_id.clone(),
            x: nx,
            y: ny,
        }),
    }
}

fn move_in_station(position: &Position, dir: Direction, station_x: i64) -> MoveOutcome {
    let (nx, ny) = step(position.x, position.y, dir);
    if nx < 0 || nx >= STATION_WIDTH || ny < 0 || ny >= STATION_HEIGHT {
        return MoveOutcome::Blocked;
    }
    match station_tile(nx, ny) {
        Tile::Wall | Tile::Customizer => MoveOutcome::Blocked,
        Tile::Door => MoveOutcome::Transition(Position {
            map_id: "street".to_string(),
            x: station_x as i32,
            y: crate::monorail::STATION_DOOR_Y,
        }),
        Tile::Floor | Tile::StationDoor => MoveOutcome::Moved(Position {
            map_id: position.map_id.clone(),
            x: nx,
            y: ny,
        }),
    }
}

fn move_in_train(position: &Position, dir: Direction) -> MoveOutcome {
    let (nx, ny) = step(position.x, position.y, dir);
    if nx < 0 || nx >= TRAIN_WIDTH || ny < 0 || ny >= TRAIN_HEIGHT {
        return MoveOutcome::Blocked;
    }
    match train_tile(nx, ny) {
        Tile::Wall => MoveOutcome::Blocked,
        _ => MoveOutcome::Moved(Position {
            map_id: position.map_id.clone(),
            x: nx,
            y: ny,
        }),
    }
}
