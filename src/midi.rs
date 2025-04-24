use anyhow::Result;
use log::{error, info};
use midir::{Ignore, MidiInput, MidiInputConnection, MidiInputPort};

pub trait MidiMessageHandler {
    fn handle(&mut self, stamp: u64, data: &[u8]) -> Result<()>;
}

pub fn get_midi_conn(
    device_name: &str,
    mut handler: impl MidiMessageHandler + Send + 'static,
) -> Result<MidiInputConnection<()>> {
    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    let port = get_midi_port(&midi_in, device_name).ok_or(anyhow::format_err!(
        "No MIDI port found for device: {}",
        device_name
    ))?;

    let in_port_name = midi_in.port_name(&port)?;
    info!("Opening connection to {}", in_port_name);

    let conn_res = midi_in.connect(
        &port,
        "midi2key-read-input",
        move |s, m, _| {
            if let Err(e) = handler.handle(s, m) {
                error!("Error handling midi message: {:?}", e);
            };
        },
        (),
    );

    match conn_res {
        Ok(c) => Ok(c),
        Err(e) => Err(anyhow::format_err!("{:?}", e)),
    }
}

fn get_midi_port(midi_in: &MidiInput, device_name: &str) -> Option<MidiInputPort> {
    let in_ports = midi_in.ports();

    for p in in_ports {
        if let Ok(name) = midi_in.port_name(&p) {
            if name.starts_with(device_name) {
                return Some(p);
            }
        }
    }

    None
}
