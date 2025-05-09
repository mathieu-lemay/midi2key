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

use crate::katana::{KatanaControl, Preset};
use crate::midi::{MidiMessageHandler, get_midi_output_conn};

mod katana;
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

#[derive(Debug)]
pub enum KatanaAction {
    PresetPanel,
    PresetA1,
    PresetA2,
    PresetA3,
    PresetA4,
    PresetB1,
    PresetB2,
    PresetB3,
    PresetB4,
}

impl TryFrom<&str> for KatanaAction {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "PRESET_PANEL" => Ok(KatanaAction::PresetPanel),
            "PRESET_A1" => Ok(KatanaAction::PresetA1),
            "PRESET_A2" => Ok(KatanaAction::PresetA2),
            "PRESET_A3" => Ok(KatanaAction::PresetA3),
            "PRESET_A4" => Ok(KatanaAction::PresetA4),
            "PRESET_B1" => Ok(KatanaAction::PresetB1),
            "PRESET_B2" => Ok(KatanaAction::PresetB2),
            "PRESET_B3" => Ok(KatanaAction::PresetB3),
            "PRESET_B4" => Ok(KatanaAction::PresetB4),
            _ => anyhow::bail!("Invalid action: {}", value),
        }
    }
}

struct Action {
    desc: String,
    keys: Vec<KeyCode>,
    kat: Vec<KatanaAction>,
}

impl TryFrom<&MidiKeyMapping> for Action {
    type Error = anyhow::Error;

    fn try_from(value: &MidiKeyMapping) -> Result<Self, Self::Error> {
        let keys = match &value.keys {
            Some(keys) => keys
                .iter()
                .map(|k| match KeyCode::from_str(k) {
                    Ok(kc) => Ok(kc),
                    Err(_) => anyhow::bail!("Invalid KeyCode: {}", k),
                })
                .collect::<Result<Vec<KeyCode>>>()?,
            None => vec![],
        };

        let kat = match &value.kat {
            Some(kat) => kat
                .iter()
                .map(|k| match KatanaAction::try_from(k.as_str()) {
                    Ok(kc) => Ok(kc),
                    Err(_) => anyhow::bail!("Invalid Katana Action: {:?}", k),
                })
                .collect::<Result<Vec<KatanaAction>>>()?,
            None => vec![],
        };
        Ok(Self {
            desc: String::from(&value.description),
            keys,
            kat,
        })
    }
}

struct Handler {
    kb: VirtualDevice,
    kat: Option<KatanaControl>,
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

        let mut keys: Vec<InputEvent> = act.keys.iter().map(|k| *KeyEvent::new(*k, 1)).collect();

        act.keys.iter().rev().for_each(|k| {
            let e = *KeyEvent::new(*k, 0);
            keys.push(e);
        });

        self.kb.emit(&keys)?;

        if let Some(kat) = &mut self.kat {
            for act in &act.kat {
                apply_katana_action(kat, act)?;
            }
        } else {
            if !act.kat.is_empty() {
                warn!(
                    "Katana actions defined but no midi output selected: {}",
                    act.desc
                );
            }
        }

        Ok(())
    }
}

fn apply_katana_action(kat: &mut KatanaControl, act: &KatanaAction) -> Result<()> {
    match act {
        KatanaAction::PresetPanel => kat.change_preset(Preset::Panel),
        KatanaAction::PresetA1 => kat.change_preset(Preset::A1),
        KatanaAction::PresetA2 => kat.change_preset(Preset::A2),
        KatanaAction::PresetA3 => kat.change_preset(Preset::A3),
        KatanaAction::PresetA4 => kat.change_preset(Preset::A4),
        KatanaAction::PresetB1 => kat.change_preset(Preset::B1),
        KatanaAction::PresetB2 => kat.change_preset(Preset::B2),
        KatanaAction::PresetB3 => kat.change_preset(Preset::B3),
        KatanaAction::PresetB4 => kat.change_preset(Preset::B4),
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
    kat: Option<Vec<String>>,
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

    let kat = match midi_out {
        Some(conn) => Some(KatanaControl::new(conn)?),
        None => None,
    };

    let mappings = get_mappings(&config)?;

    let handler = Handler { kb, kat, mappings };
    let _conn = midi::get_midi_input_conn(&config.midi_input, handler)?;

    let (tx, rx) = channel();
    ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler");
    info!("Running. Press Ctrl-C to quit.");

    rx.recv().expect("Could not receive from channel.");
    info!("Closing connection");

    Ok(())
}
