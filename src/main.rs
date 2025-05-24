use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::sync::mpsc::channel;

use action::Action;
use anyhow::Result;
use log::info;
use serde::Deserialize;

use crate::handler::Handler;
use crate::midi::get_midi_output_conn;

mod action;
mod handler;
mod midi;
mod virtual_keyboard;

const APP_NAME: &str = "midi2key";

#[derive(Debug, Eq, PartialEq, Hash)]
enum Event {
    PC(u8),
    CC(u8),
}

impl TryFrom<&str> for Event {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (t, v) = value
            .split_once(' ')
            .ok_or_else(|| anyhow::anyhow!("Invalid event: {}", value))?;

        let val = v.parse::<u8>()?;

        let evt = match t {
            "PC" => Event::PC(val),
            "CC" => Event::CC(val),
            _ => anyhow::bail!("Invalid event type: {}: {}", t, value),
        };

        Ok(evt)
    }
}

#[derive(Debug, Deserialize)]
struct Config {
    midi_input: String,
    midi_output: Option<String>,
    mappings: Vec<MidiKeyMapping>,
}

#[derive(Debug, Deserialize)]
struct MidiKeyMapping {
    event: String,
    description: String,
    keys: Option<Vec<String>>,
    midi: Option<Vec<String>>,
}

fn get_config() -> Result<Config> {
    let mut cfg_file = dirs::config_dir().unwrap_or(PathBuf::from("."));
    cfg_file.push(APP_NAME);
    cfg_file.push("config.v2.toml");

    let s = match read_to_string(&cfg_file) {
        Ok(s) => s,
        Err(e) => anyhow::bail!("Failed to read config file: {:?}: {}", cfg_file, e),
    };

    let config = toml::from_str(&s)?;

    Ok(config)
}

fn get_mappings(config: &Config) -> Result<HashMap<Event, Action>> {
    let mut mappings = HashMap::new();

    for m in &config.mappings {
        info!("Adding mapping: {:?} => {}", m.event, m.description);

        let event = m.event.as_str().try_into()?;
        let action = (m).try_into()?;

        mappings.insert(event, action);
    }

    Ok(mappings)
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let config = get_config()?;

    let kb = virtual_keyboard::create_virtual_keyboard()?;
    let midi_out = match &config.midi_output {
        Some(out) => Some(get_midi_output_conn(out)?),
        None => None,
    };

    let mappings = get_mappings(&config)?;

    let handler = Handler {
        kb,
        midi_out,
        mappings,
    };
    let _conn = midi::get_midi_input_conn(&config.midi_input, handler)?;

    let (tx, rx) = channel();
    ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler");
    info!("Running. Press Ctrl-C to quit.");

    rx.recv().expect("Could not receive from channel.");
    info!("Closing connection");

    Ok(())
}
