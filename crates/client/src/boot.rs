use std::io::{self, Write};

use crossterm::{
    cursor,
    execute,
    terminal::{self, Clear, ClearType},
};
use tokio::time::{interval, Duration, Instant};

use street_common::config::ClientConfig;

use crate::net::{connect, Connection};

const SPINNER: [char; 4] = ['|', '/', '-', '\\'];
const MIN_BOOT_MS: u64 = 1200;

const BOOT_LOGO: [&str; 6] = [
    "  _______ _           _____ _                 _ ",
    " |__   __| |         / ____| |               | |",
    "    | |  | |__   ___| (___ | |_ _ __ ___  ___| |_",
    "    | |  | '_ \\ / _ \\ \\___ \\| __| '__/ _ \\/ _ \\ __|",
    "    | |  | | | |  __/ ____) | |_| | |  __/  __/ |_",
    "    |_|  |_| |_|\\___|_____/ \\__|_|  \\___|\\___|\\__|",
];

const BOOT_STEPS: [&str; 5] = [
    "booting kernel",
    "mounting sector map",
    "calibrating doors",
    "syncing monorail",
    "establishing link",
];

pub async fn boot_and_connect(
    config: &ClientConfig,
    signing_key: &ed25519_dalek::SigningKey,
    x25519_pubkey: &str,
) -> anyhow::Result<Connection> {
    let mut stdout = io::stdout();
    execute!(stdout, cursor::Hide)?;

    let mut connect_fut = Box::pin(connect(config, signing_key, x25519_pubkey));
    let mut ticker = interval(Duration::from_millis(120));
    let mut ticks: u64 = 0;
    let mut step_index: usize = 0;
    let start = Instant::now();
    let min_duration = Duration::from_millis(MIN_BOOT_MS);
    let mut result: Option<anyhow::Result<Connection>> = None;

    draw_boot(&mut stdout, config, step_index, SPINNER[0])?;

    let result = loop {
        if result.is_some() && start.elapsed() >= min_duration {
            break result.expect("boot result missing");
        }
        tokio::select! {
            res = &mut connect_fut, if result.is_none() => {
                result = Some(res);
            }
            _ = ticker.tick() => {
                ticks += 1;
                let max_step = BOOT_STEPS.len().saturating_sub(1);
                if step_index < max_step && ticks % 4 == 0 {
                    step_index += 1;
                }
                let spinner = SPINNER[(ticks as usize) % SPINNER.len()];
                draw_boot(&mut stdout, config, step_index, spinner)?;
            }
        }
    };

    execute!(stdout, cursor::Show, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
    stdout.flush()?;

    result
}

fn draw_boot(
    stdout: &mut io::Stdout,
    config: &ClientConfig,
    step_index: usize,
    spinner: char,
) -> anyhow::Result<()> {
    let (width, height) = terminal::size().unwrap_or((80, 24));

    let mut lines = Vec::new();
    lines.extend(BOOT_LOGO.iter().map(|line| line.to_string()));
    lines.push("".to_string());
    lines.push("Snow Crash transit node // client boot".to_string());
    lines.push(format!("link: {}", config.relay_url));
    lines.push("".to_string());

    for (index, step) in BOOT_STEPS.iter().enumerate() {
        let line = if index < step_index {
            format!("[OK] {step}")
        } else if index == step_index {
            format!("[{spinner}] {step}")
        } else {
            format!("[..] {step}")
        };
        lines.push(line);
    }

    lines.push("".to_string());
    lines.push("press Ctrl+C to abort".to_string());

    let total_lines = lines.len() as u16;
    let top_pad = height.saturating_sub(total_lines) / 2;

    execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
    for _ in 0..top_pad {
        writeln!(stdout)?;
    }
    for line in lines {
        let centered = center_line(&line, width as usize);
        writeln!(stdout, "{centered}")?;
    }
    stdout.flush()?;
    Ok(())
}

fn center_line(line: &str, width: usize) -> String {
    if line.len() >= width {
        return line.to_string();
    }
    let pad = (width - line.len()) / 2;
    format!("{:pad$}{}", "", line, pad = pad)
}
