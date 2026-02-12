use std::collections::HashSet;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use street_world::monorail::{is_track_row, TRACK_ROWS};
use street_world::{
    parse_room_map_id, parse_station_map_id, parse_train_map_id, room_id_for_door, room_tile,
    station_tile, street_door_side, street_tile, train_tile, RoomSide, Tile, ROOM_HEIGHT,
    ROOM_WIDTH, STATION_HEIGHT, STATION_WIDTH, STREET_CIRCUMFERENCE_TILES, STREET_HEIGHT,
    TRAIN_HEIGHT, TRAIN_WIDTH,
};

use crate::ui::AppState;

pub fn draw_ui(f: &mut Frame, app: &AppState) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((STREET_HEIGHT + 2) as u16),
            Constraint::Min(0),
        ])
        .split(size);

    let title_style = Style::default()
        .fg(Color::LightCyan)
        .add_modifier(Modifier::BOLD);
    let map_block = Block::default()
        .borders(Borders::ALL)
        .title(
            Line::from(Span::styled(app.location_label(), title_style)).alignment(Alignment::Left),
        )
        .title(
            Line::from(Span::styled(app.position_label(), title_style)).alignment(Alignment::Right),
        );

    let map_area = chunks[0];
    let inner_map = map_block.inner(map_area);
    let map_content = render_map(app, inner_map);
    let map_paragraph = Paragraph::new(map_content).block(map_block);
    f.render_widget(map_paragraph, map_area);

    let lower = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(0)])
        .split(chunks[1]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(11)])
        .split(lower[0]);

    let info_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(app.info_title(), title_style));
    let info_text = app.info_text.as_str();
    let info_paragraph = Paragraph::new(info_text)
        .block(info_block)
        .wrap(Wrap { trim: true });
    f.render_widget(info_paragraph, left[0]);

    let minimap_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled("Ring", title_style));
    let minimap_area = minimap_block.inner(left[1]);
    let minimap_text = render_minimap(app, minimap_area);
    let minimap = Paragraph::new(minimap_text).block(minimap_block);
    f.render_widget(minimap, left[1]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(lower[1]);

    let chat_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled("Chat", title_style));
    let chat_text = app.chat_text.as_str();
    let chat_paragraph = Paragraph::new(chat_text)
        .block(chat_block)
        .wrap(Wrap { trim: false });
    f.render_widget(chat_paragraph, right[0]);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(app.input_title(), title_style));
    let input_hint = app.input_hint();
    let (input_text, input_style) = if app.input_mode {
        (app.input.as_str(), Style::default().fg(Color::White))
    } else {
        (input_hint.as_str(), Style::default().fg(Color::DarkGray))
    };
    let input_paragraph = Paragraph::new(input_text)
        .block(input_block)
        .style(input_style);
    f.render_widget(input_paragraph, right[1]);
}

fn render_map(app: &AppState, area: Rect) -> Text<'static> {
    if app.map_id == "street" {
        render_street(app, area)
    } else if let Some((side, _)) = parse_room_map_id(&app.map_id) {
        render_room(app, area, side)
    } else if parse_station_map_id(&app.map_id).is_some() {
        render_station(app, area)
    } else if parse_train_map_id(&app.map_id).is_some() {
        render_train(app, area)
    } else {
        Text::from("unknown map")
    }
}

fn render_street(app: &AppState, area: Rect) -> Text<'static> {
    let width = area.width as i32;
    let player_x = app.position.0;
    let start_x = player_x - width / 2;

    let wall_style = Style::default().fg(Color::DarkGray);
    let door_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let station_style = Style::default()
        .fg(Color::LightBlue)
        .add_modifier(Modifier::BOLD);
    let floor_style = Style::default().fg(Color::Gray);
    let track_style = Style::default()
        .fg(Color::LightGreen)
        .add_modifier(Modifier::BOLD);
    let train_clockwise_style = Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    let train_counter_style = Style::default()
        .fg(Color::LightCyan)
        .add_modifier(Modifier::BOLD);
    let player_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let other_style = Style::default()
        .fg(Color::LightMagenta)
        .add_modifier(Modifier::BOLD);

    let circumference = STREET_CIRCUMFERENCE_TILES as i64;
    let mut train_positions_top: HashSet<i64> = HashSet::new();
    let mut train_positions_bottom: HashSet<i64> = HashSet::new();
    for train in &app.trains {
        let head = adjust_train_x(train.x, player_x, circumference);
        let positions = if train.clockwise {
            &mut train_positions_bottom
        } else {
            &mut train_positions_top
        };
        for offset in 0..TRAIN_WIDTH {
            positions.insert(head - offset as i64);
        }
    }
    let top_track_row = TRACK_ROWS[0];
    let bottom_track_row = TRACK_ROWS[1];

    let mut lines = Vec::new();
    for y in 0..STREET_HEIGHT {
        let mut spans = Vec::with_capacity(width as usize);
        for dx in 0..width {
            let x = start_x + dx;
            let (ch, style) = if x == player_x && y == app.position.1 {
                ("@", player_style)
            } else if app.nearby_positions.contains(&(x, y)) {
                ("o", other_style)
            } else if y == top_track_row && train_positions_top.contains(&(x as i64)) {
                ("T", train_counter_style)
            } else if y == bottom_track_row && train_positions_bottom.contains(&(x as i64)) {
                ("T", train_clockwise_style)
            } else {
                match street_tile(x, y) {
                    Tile::Wall => ("#", wall_style),
                    Tile::Door => {
                        let style = door_style_for(app, x, y, door_style);
                        ("D", style)
                    }
                    Tile::StationDoor => ("M", station_style),
                    Tile::Customizer => (
                        "C",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Tile::Floor => {
                        if is_track_row(y) {
                            ("=", track_style)
                        } else {
                            (".", floor_style)
                        }
                    }
                }
            };
            spans.push(Span::styled(ch, style));
        }
        lines.push(Line::from(spans));
    }
    Text::from(lines)
}

fn render_station(app: &AppState, area: Rect) -> Text<'static> {
    let width = area.width as i32;
    let view_width = width.min(STATION_WIDTH);
    let player_x = app.position.0;
    let start_x = if STATION_WIDTH > view_width {
        let half = view_width / 2;
        let mut start = player_x - half;
        if start < 0 {
            start = 0;
        }
        let max_start = STATION_WIDTH - view_width;
        if start > max_start {
            start = max_start;
        }
        start
    } else {
        0
    };

    let wall_style = Style::default().fg(Color::DarkGray);
    let door_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let floor_style = Style::default().fg(Color::Gray);
    let player_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let other_style = Style::default()
        .fg(Color::LightMagenta)
        .add_modifier(Modifier::BOLD);

    let mut lines = Vec::new();
    for y in 0..STATION_HEIGHT {
        let mut spans = Vec::with_capacity(view_width as usize);
        for dx in 0..view_width {
            let x = start_x + dx;
            let (ch, style) = if x == app.position.0 && y == app.position.1 {
                ("@", player_style)
            } else if app.nearby_positions.contains(&(x, y)) {
                ("o", other_style)
            } else {
                match station_tile(x, y) {
                    Tile::Wall => ("#", wall_style),
                    Tile::Door => ("D", door_style),
                    Tile::Customizer => (
                        "C",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    _ => (".", floor_style),
                }
            };
            spans.push(Span::styled(ch, style));
        }
        lines.push(Line::from(spans));
    }
    Text::from(lines)
}

fn render_train(app: &AppState, area: Rect) -> Text<'static> {
    let width = area.width as i32;
    let view_width = width.min(TRAIN_WIDTH);
    let player_x = app.position.0;
    let start_x = if TRAIN_WIDTH > view_width {
        let half = view_width / 2;
        let mut start = player_x - half;
        if start < 0 {
            start = 0;
        }
        let max_start = TRAIN_WIDTH - view_width;
        if start > max_start {
            start = max_start;
        }
        start
    } else {
        0
    };

    let wall_style = Style::default().fg(Color::DarkGray);
    let floor_style = Style::default().fg(Color::Gray);
    let player_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let other_style = Style::default()
        .fg(Color::LightMagenta)
        .add_modifier(Modifier::BOLD);

    let mut lines = Vec::new();
    for y in 0..TRAIN_HEIGHT {
        let mut spans = Vec::with_capacity(view_width as usize);
        for dx in 0..view_width {
            let x = start_x + dx;
            let (ch, style) = if x == app.position.0 && y == app.position.1 {
                ("@", player_style)
            } else if app.nearby_positions.contains(&(x, y)) {
                ("o", other_style)
            } else {
                match train_tile(x, y) {
                    Tile::Wall => ("#", wall_style),
                    _ => (".", floor_style),
                }
            };
            spans.push(Span::styled(ch, style));
        }
        lines.push(Line::from(spans));
    }
    Text::from(lines)
}

fn render_room(app: &AppState, area: Rect, side: RoomSide) -> Text<'static> {
    let width = area.width as i32;
    let view_width = width.min(ROOM_WIDTH);
    let player_x = app.position.0;
    let start_x = if ROOM_WIDTH > view_width {
        let half = view_width / 2;
        let mut start = player_x - half;
        if start < 0 {
            start = 0;
        }
        let max_start = ROOM_WIDTH - view_width;
        if start > max_start {
            start = max_start;
        }
        start
    } else {
        0
    };

    let wall_style = Style::default().fg(Color::DarkGray);
    let door_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let floor_style = Style::default().fg(Color::Gray);
    let customizer_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    let player_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let other_style = Style::default()
        .fg(Color::LightMagenta)
        .add_modifier(Modifier::BOLD);

    let mut lines = Vec::new();
    for y in 0..ROOM_HEIGHT {
        let mut spans = Vec::with_capacity(view_width as usize);
        for dx in 0..view_width {
            let x = start_x + dx;
            let (ch, style) = if x == app.position.0 && y == app.position.1 {
                ("@", player_style)
            } else if app.nearby_positions.contains(&(x, y)) {
                ("o", other_style)
            } else {
                match room_tile(x, y, side) {
                    Tile::Wall => ("#", wall_style),
                    Tile::Door => ("D", door_style),
                    Tile::Customizer => ("C", customizer_style),
                    Tile::Floor | Tile::StationDoor => (".", floor_style),
                }
            };
            spans.push(Span::styled(ch, style));
        }
        lines.push(Line::from(spans));
    }
    Text::from(lines)
}

fn render_minimap(app: &AppState, area: Rect) -> Text<'static> {
    use std::f64::consts::TAU;

    let width = area.width.max(3) as i32;
    let height = area.height.max(3) as i32;
    let radius = (width.min(height) / 2 - 1).max(1) as f64;
    let cx = (width - 1) as f64 / 2.0;
    let cy = (height - 1) as f64 / 2.0;

    let x = app.ring_x();
    let circumference = street_world::STREET_CIRCUMFERENCE_TILES as f64;
    let wrapped = (x.rem_euclid(street_world::STREET_CIRCUMFERENCE_TILES) as f64) / circumference;
    let theta = wrapped * TAU;
    let px = (cx + radius * theta.cos()).round() as i32;
    let py = (cy + radius * theta.sin()).round() as i32;

    let ring_style = Style::default().fg(Color::DarkGray);
    let player_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let mut lines = Vec::new();
    for y in 0..height {
        let mut spans = Vec::with_capacity(width as usize);
        for x in 0..width {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let is_ring = (dist - radius).abs() <= 0.6;
            let (ch, style) = if x == px && y == py {
                ("@", player_style)
            } else if is_ring {
                ("o", ring_style)
            } else {
                (" ", Style::default())
            };
            spans.push(Span::styled(ch, style));
        }
        lines.push(Line::from(spans));
    }
    Text::from(lines)
}

fn adjust_train_x(train_x: f64, reference_x: i32, circumference: i64) -> i64 {
    let base = train_x.round() as i64;
    if circumference == 0 {
        return base;
    }
    let reference = reference_x as i64;
    let k = ((reference - base) as f64 / circumference as f64).round() as i64;
    base + k * circumference
}

fn door_style_for(app: &AppState, x: i32, y: i32, fallback: Style) -> Style {
    let Some(side) = street_door_side(x, y) else {
        return fallback;
    };
    let room_id = room_id_for_door(side, x);
    let Some(room) = app.room_cache.get(&room_id) else {
        return fallback;
    };
    room.door_color
        .as_deref()
        .and_then(color_from_name)
        .map(|color| Style::default().fg(color).add_modifier(Modifier::BOLD))
        .unwrap_or(fallback)
}

fn color_from_name(name: &str) -> Option<Color> {
    match name {
        "red" => Some(Color::LightRed),
        "green" => Some(Color::LightGreen),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::LightBlue),
        "magenta" => Some(Color::LightMagenta),
        "cyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        _ => None,
    }
}
