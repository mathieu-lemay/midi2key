use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::mpsc::channel;

use anyhow::Result;
use evdev::uinput::VirtualDevice;
use evdev::{InputEvent, KeyCode, KeyEvent};
use log::{debug, info, warn};
use midly::MidiMessage;
use midly::live::LiveEvent;
use serde::Deserialize;

use crate::midi::MidiMessageHandler;

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

struct Action {
    desc: String,
    keys: Vec<KeyCode>,
}

impl TryFrom<&MidiKeyMapping> for Action {
    type Error = anyhow::Error;

    fn try_from(value: &MidiKeyMapping) -> Result<Self, Self::Error> {
        let keys = value
            .keys
            .iter()
            .map(|k| match KeyCode::from_str(k) {
                Ok(kc) => Ok(kc),
                Err(_) => anyhow::bail!("Invalid KeyCode: {}", k),
            })
            .collect::<Result<Vec<KeyCode>>>()?;

        Ok(Self {
            desc: String::from(&value.description),
            keys,
        })
    }
}

struct Handler {
    kb: VirtualDevice,
    mappings: HashMap<Event, Action>,
}

impl MidiMessageHandler for Handler {
    fn handle(&mut self, _: u64, raw_message: &[u8]) -> Result<()> {
        let (_ch, msg) = match LiveEvent::parse(raw_message) {
            Ok(LiveEvent::Midi { channel, message }) => (channel, message),
            Ok(evt) => {
                warn!("Ignoring non Midi event: {:?}", evt);
                return Ok(());
            }
            Err(e) => {
                return Err(anyhow::anyhow!(e));
            }
        };

        let evt = match msg {
            MidiMessage::ProgramChange { program: p } => Some(Event::PC(p.as_int())),
            MidiMessage::Controller {
                controller: c,
                value: _,
            } => Some(Event::CC(c.as_int())),
            _ => None,
        };

        if evt.is_none() {
            warn!("Unsupported message: {:?}", msg);
            return Ok(());
        }
        let evt = evt.unwrap();

        let act = self.mappings.get(&evt);
        if act.is_none() {
            warn!("Unsupported message: {:?}", msg);
            return Ok(());
        }
        let act = act.unwrap();

        let mut keys: Vec<InputEvent> = act.keys.iter().map(|k| *KeyEvent::new(*k, 1)).collect();

        act.keys.iter().rev().for_each(|k| {
            let e = *KeyEvent::new(*k, 0);
            keys.push(e);
        });

        debug!("{}", act.desc);
        self.kb.emit(&keys)?;

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct Config {
    midi_device: String,
    mappings: Vec<MidiKeyMapping>,
}

#[derive(Debug, Deserialize)]
struct MidiKeyMapping {
    event: String,
    description: String,
    keys: Vec<String>,
}

fn get_config() -> Result<Config> {
    let mut cfg_file = dirs::config_dir().unwrap_or(PathBuf::from("."));
    cfg_file.push(APP_NAME);
    cfg_file.push("config.toml");

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

    let mappings = get_mappings(&config)?;

    let handler = Handler { kb, mappings };
    let _conn = midi::get_midi_conn(&config.midi_device, handler)?;

    let (tx, rx) = channel();
    ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler");
    info!("Running. Press Ctrl-C to quit.");

    rx.recv().expect("Could not receive from channel.");
    info!("Closing connection");

    Ok(())
}
