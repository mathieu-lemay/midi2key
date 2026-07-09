use std::fmt;
use std::str::FromStr;

use anyhow::Result;
use evdev::KeyCode;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, de};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub midi_device: String,
    pub mappings: Vec<MidiKeyMapping>,
}

#[derive(Debug, Deserialize)]
pub struct MidiKeyMapping {
    pub event: String,
    pub description: String,
    pub keys: Vec<KeyPress>,
}

#[derive(Debug, Clone)]
pub struct KeyPress {
    pub modifier: Option<KeyCode>,
    pub key: KeyCode,
    pub wait: Option<u64>,
}

impl<'de> Deserialize<'de> for KeyPress {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Modifier,
            Key,
            Wait,
        }

        struct KeyPressVisitor;

        impl<'de> Visitor<'de> for KeyPressVisitor {
            type Value = KeyPress;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct KeyPress")
            }

            fn visit_map<V>(self, mut map: V) -> Result<KeyPress, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut modifier = None;
                let mut key = None;
                let mut wait = None;

                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Modifier => {
                            if modifier.is_some() {
                                return Err(de::Error::duplicate_field("modifier"));
                            }
                            let val: String = map.next_value()?;
                            modifier = match KeyCode::from_str(&val) {
                                Ok(k) => Some(k),
                                Err(e) => return Err(de::Error::custom(e)),
                            };
                        }
                        Field::Key => {
                            if key.is_some() {
                                return Err(de::Error::duplicate_field("key"));
                            }
                            let val: String = map.next_value()?;
                            key = match KeyCode::from_str(&val) {
                                Ok(k) => Some(k),
                                Err(e) => return Err(de::Error::custom(e)),
                            };
                        }
                        Field::Wait => {
                            if wait.is_some() {
                                return Err(de::Error::duplicate_field("wait"));
                            }
                            wait = Some(map.next_value()?);
                        }
                    }
                }
                let key = key.ok_or_else(|| de::Error::missing_field("key"))?;
                Ok(KeyPress {
                    modifier,
                    key,
                    wait,
                })
            }
        }

        const FIELDS: &[&str] = &["modifier", "key"];
        deserializer.deserialize_struct("KeyPress", FIELDS, KeyPressVisitor)
    }
}
