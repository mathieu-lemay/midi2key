use anyhow::Result;
use log::{error, info};
use midir::{
    Ignore,
    MidiIO,
    MidiInput,
    MidiInputConnection,
    MidiOutput,
    MidiOutputConnection,
    PortInfoError,
};

pub trait MidiMessageHandler {
    fn handle(&mut self, stamp: u64, data: &[u8]) -> Result<()>;
}

pub fn get_midi_input_conn(
    device_name: &str,
    mut handler: impl MidiMessageHandler + Send + 'static,
) -> Result<MidiInputConnection<()>> {
    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    let port = get_midi_port(&midi_in, device_name)?;

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

pub fn get_midi_output_conn(device_name: &str) -> Result<MidiOutputConnection> {
    let midi_out = MidiOutput::new("midir output")?;

    let port = get_midi_port(&midi_out, device_name)?;

    let in_port_name = midi_out.port_name(&port)?;
    info!("Opening connection to {}", in_port_name);

    let conn_res = midi_out.connect(&port, "midi2key-read-input");

    match conn_res {
        Ok(c) => Ok(c),
        Err(e) => Err(anyhow::format_err!("{:?}", e)),
    }
}

fn get_midi_port<T>(midi_io: &dyn MidiIO<Port = T>, device_name: &str) -> Result<T>
where
    T: Clone,
{
    let in_ports = midi_io.ports();

    for p in in_ports {
        if let Ok(name) = midi_io.port_name(&p) {
            if name.starts_with(device_name) {
                return Ok(p);
            }
        }
    }

    let devices = midi_io
        .ports()
        .iter()
        .map(|p| midi_io.port_name(p))
        .collect::<Result<Vec<String>, PortInfoError>>();

    let available_devices = match devices {
        Ok(d) => d.join("\n  "),
        Err(_) => "Unable to list available devices.".to_string(),
    };

    anyhow::bail!(
        "No MIDI port found for device: {}. Available devices:\n  {}",
        device_name,
        available_devices
    )
}
