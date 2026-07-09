use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use evdev::KeyEvent;
use evdev::uinput::VirtualDevice;
use log::{debug, info, warn};
use midly::MidiMessage;
use midly::live::LiveEvent;

use crate::config::{Config, KeyPress, MidiKeyMapping};
use crate::midi::MidiMessageHandler;

mod config;
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
    keys: Vec<KeyPress>,
}

impl TryFrom<&MidiKeyMapping> for Action {
    type Error = anyhow::Error;

    fn try_from(value: &MidiKeyMapping) -> Result<Self, Self::Error> {
        Ok(Self {
            desc: String::from(&value.description),
            keys: value.keys.to_vec(),
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

        debug!("{}", act.desc);

        for kp in act.keys.iter() {
            if let Some(wait) = kp.wait {
                thread::sleep(Duration::from_millis(wait));
            }

            let mut keys = vec![*KeyEvent::new(kp.key, 1), *KeyEvent::new(kp.key, 0)];

            if let Some(mod_key) = kp.modifier {
                keys.insert(0, *KeyEvent::new(mod_key, 1));
                keys.push(*KeyEvent::new(mod_key, 0));
            }

            self.kb.emit(&keys)?;
        }

        Ok(())
    }
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
