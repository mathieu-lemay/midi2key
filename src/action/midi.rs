use anyhow::Result;
use itertools::Itertools;

#[derive(Debug)]
pub enum MidiAction {
    PC(u8),
    CC(u8, u8),
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

        let act = match type_ {
            "CC" => parse_cc_action(val),
            "PC" => parse_pc_action(val),
            _ => anyhow::bail!("Invalid action: {}", value),
        };

        act.map_err(|e| anyhow::anyhow!(format!("Unable to parse {}: {}", value, e)))
    }
}

fn parse_cc_action(value: &str) -> Result<MidiAction> {
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

    Ok(MidiAction::CC(parts[0], parts[1]))
}

fn parse_pc_action(value: &str) -> Result<MidiAction> {
    let v = value.parse::<u8>()?;

    Ok(MidiAction::PC(v))
}
