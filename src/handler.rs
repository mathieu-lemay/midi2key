use std::collections::HashMap;

use anyhow::Result;
use evdev::uinput::VirtualDevice;
use evdev::{InputEvent, KeyCode, KeyEvent};
use log::{debug, warn};
use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};
use midir::MidiOutputConnection;
use midly::MidiMessage;
use midly::live::LiveEvent;

use crate::action::midi::MidiAction;
use crate::midi::MidiMessageHandler;
use crate::{Action, Event};

pub struct Handler {
    pub kb: VirtualDevice,
    pub midi_out: Option<MidiOutputConnection>,
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
        emit_midi_events(&mut self.midi_out, &act.midi)?;

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

fn emit_midi_events(
    midi_out: &mut Option<MidiOutputConnection>,
    acts: &[MidiAction],
) -> Result<()> {
    let midi_out = match midi_out.as_mut() {
        Some(m) => m,
        None => {
            if !acts.is_empty() {
                warn!("Katana actions defined but no midi output selected");
            }

            return Ok(());
        }
    };

    for act in acts {
        let midi_msg = match *act {
            MidiAction::PC(p) => MidiMsg::ChannelVoice {
                channel: Channel::Ch1,
                msg: ChannelVoiceMsg::ProgramChange { program: p },
            },
            MidiAction::CC(c, v) => {
                let cc = ControlChange::CC {
                    control: c,
                    value: v,
                };
                MidiMsg::ChannelVoice {
                    channel: Channel::Ch1,
                    msg: ChannelVoiceMsg::ControlChange { control: cc },
                }
            }
        };

        midi_out.send(&midi_msg.to_midi())?;
    }

    Ok(())
}
