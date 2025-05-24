use anyhow::Result;
use itertools::Itertools;
use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

#[derive(Debug)]
pub struct MidiAction {
    msg: MidiMsg,
}

impl MidiAction {
    pub fn to_midi(&self) -> Vec<u8> {
        self.msg.to_midi()
    }
}

impl TryFrom<&str> for MidiAction {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (type_, val) = match value.splitn(2, " ").collect_tuple() {
            Some((t, v)) => (t, v),
            None => {
                anyhow::bail!(format!("Unable to parse {}", value))
            }
        };

        let msg = match type_ {
            "CC" => parse_cc_action(val),
            "PC" => parse_pc_action(val),
            _ => anyhow::bail!("Invalid action: {}", value),
        };

        match msg {
            Ok(m) => Ok(MidiAction { msg: m }),
            Err(e) => Err(anyhow::anyhow!(format!("Unable to parse {}: {}", value, e))),
        }
    }
}

fn parse_cc_action(value: &str) -> Result<MidiMsg> {
    let parts: Vec<u8> = value
        .splitn(2, ":")
        .map(|v| match v.parse::<u8>() {
            Ok(i) => Ok(i),
            Err(e) => Err(anyhow::anyhow!(e)),
        })
        .collect::<Result<Vec<u8>>>()?;

    if parts.len() != 2 {
        anyhow::bail!("value should contain exactly 2 parts")
    }

    let cc = ControlChange::CC {
        control: parts[0],
        value: parts[1],
    };

    Ok(MidiMsg::ChannelVoice {
        channel: Channel::Ch1,
        msg: ChannelVoiceMsg::ControlChange { control: cc },
    })
}

fn parse_pc_action(value: &str) -> Result<MidiMsg> {
    let p = value.parse::<u8>()?;

    Ok(MidiMsg::ChannelVoice {
        channel: Channel::Ch1,
        msg: ChannelVoiceMsg::ProgramChange { program: p },
    })
}
