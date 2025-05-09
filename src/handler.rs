use std::collections::HashMap;

use anyhow::Result;
use evdev::uinput::VirtualDevice;
use evdev::{InputEvent, KeyCode, KeyEvent};
use log::{debug, warn};
use midly::MidiMessage;
use midly::live::LiveEvent;

use crate::katana::{KatanaControl, Preset};
use crate::midi::MidiMessageHandler;
use crate::{Action, Event, KatanaAction};

pub struct Handler {
    pub kb: VirtualDevice,
    pub kat: Option<KatanaControl>,
    pub mappings: HashMap<Event, Action>,
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

        emit_keyboard_events(&mut self.kb, &act.keys)?;
        emit_katana_events(&mut self.kat, &act.kat)?;

        Ok(())
    }
}

fn emit_keyboard_events(kb: &mut VirtualDevice, keys: &[KeyCode]) -> Result<()> {
    let mut evts: Vec<InputEvent> = keys.iter().map(|k| *KeyEvent::new(*k, 1)).collect();

    keys.iter().rev().for_each(|k| {
        let e = *KeyEvent::new(*k, 0);
        evts.push(e);
    });

    kb.emit(&evts)?;

    Ok(())
}

fn emit_katana_events(kat: &mut Option<KatanaControl>, acts: &[KatanaAction]) -> Result<()> {
    let kat = match kat.as_mut() {
        Some(kat) => kat,
        None => {
            if !acts.is_empty() {
                warn!("Katana actions defined but no midi output selected");
            }

            return Ok(());
        }
    };

    for act in acts {
        apply_katana_action(kat, act)?;
    }

    Ok(())
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
