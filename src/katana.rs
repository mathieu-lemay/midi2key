use anyhow::Result;
use log::{debug, error};
use midi_msg::{ManufacturerID, MidiMsg, SystemExclusiveMsg};
use midir::MidiOutputConnection;

const ROLAND_MANUFACTURER_ID: ManufacturerID = ManufacturerID(0x41, None);
const SYSEX_WRITE_PREFIX: [u8; 6] = [0x00, 0x00, 0x00, 0x00, 0x33, 0x12];
const EDIT_MODE_ON: [u8; 5] = [0x7F, 0x00, 0x00, 0x01, 0x01];
const EDIT_MODE_OFF: [u8; 5] = [0x7F, 0x00, 0x00, 0x01, 0x00];

const PRESET_ADDR: [u8; 4] = [0x00, 0x01, 0x00, 0x00];

#[derive(Debug)]
pub enum Preset {
    Panel,
    A1,
    A2,
    A3,
    A4,
    B1,
    B2,
    B3,
    B4,
}

impl From<Preset> for u8 {
    fn from(value: Preset) -> Self {
        match value {
            Preset::Panel => 0,
            Preset::A1 => 1,
            Preset::A2 => 2,
            Preset::A3 => 3,
            Preset::A4 => 4,
            Preset::B1 => 5,
            Preset::B2 => 6,
            Preset::B3 => 7,
            Preset::B4 => 8,
        }
    }
}

pub struct KatanaControl {
    conn: MidiOutputConnection,
}

impl KatanaControl {
    pub fn new(conn: MidiOutputConnection) -> Result<Self> {
        let mut ctrl = Self { conn };

        ctrl.enter_edit_mode()?;

        Ok(ctrl)
    }

    pub fn change_preset(&mut self, preset: Preset) -> Result<()> {
        debug!("Changing preset to {:?}", preset);

        let mut payload = Vec::with_capacity(6);
        payload.extend(PRESET_ADDR);
        payload.extend([0x00, preset.into()]);

        let msg = create_sysex_message(&payload);

        self.send(&msg)
    }

    fn enter_edit_mode(&mut self) -> Result<()> {
        debug!("Entering edit mode");
        let msg = create_sysex_message(&EDIT_MODE_ON);

        self.send(&msg)
    }

    fn exit_edit_mode(&mut self) -> Result<()> {
        debug!("Exiting edit mode");
        let msg = create_sysex_message(&EDIT_MODE_OFF);

        self.send(&msg)
    }

    fn send(&mut self, msg: &MidiMsg) -> Result<()> {
        self.conn.send(&msg.to_midi())?;

        Ok(())
    }
}

impl Drop for KatanaControl {
    fn drop(&mut self) {
        if let Err(e) = self.exit_edit_mode() {
            error!("Error exiting edit mode: {:?}", e)
        }
    }
}

fn create_sysex_message(data: &[u8]) -> MidiMsg {
    let msg = SystemExclusiveMsg::Commercial {
        id: ROLAND_MANUFACTURER_ID,
        data: create_sysex_payload(data),
    };

    MidiMsg::SystemExclusive { msg }
}

fn create_sysex_payload(data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(32);

    payload.extend_from_slice(&SYSEX_WRITE_PREFIX);
    payload.extend_from_slice(data);
    payload.push(calculate_checksum(data));

    payload
}

fn calculate_checksum(data: &[u8]) -> u8 {
    let acc = data.iter().fold(0u8, |sum, &byte| (sum + byte) & 0x7f);

    (0x80 - acc) & 0x7f
}
