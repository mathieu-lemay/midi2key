use std::str::FromStr;

use anyhow::Result;
use evdev::KeyCode;
use midi::MidiAction;

use crate::MidiKeyMapping;

pub mod midi;

pub struct Action {
    pub desc: String,
    pub keys: Vec<KeyCode>,
    pub midi: Vec<MidiAction>,
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

        let midi = match &value.midi {
            Some(kat) => kat
                .iter()
                .map(|k| match MidiAction::try_from(k.as_str()) {
                    Ok(kc) => Ok(kc),
                    Err(_) => anyhow::bail!("Invalid Katana Action: {:?}", k),
                })
                .collect::<Result<Vec<MidiAction>>>()?,
            None => vec![],
        };

        Ok(Self {
            desc: String::from(&value.description),
            keys,
            midi,
        })
    }
}
