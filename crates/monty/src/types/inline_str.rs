//! Fixed-capacity UTF-8 strings shared by values, views and iterators.

use std::str;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

/// The length enum supplies niches so a containing `Value` remains 16 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InlineStr {
    len: InlineLen,
    bytes: [u8; 15],
}

impl InlineStr {
    pub(crate) fn new(text: &str) -> Option<Self> {
        if text.len() > 15 {
            return None;
        }
        let mut bytes = [0; 15];
        bytes[..text.len()].copy_from_slice(text.as_bytes());
        Some(Self {
            len: InlineLen::from_len(text.len()),
            bytes,
        })
    }

    pub(crate) fn as_str(&self) -> &str {
        str::from_utf8(&self.bytes[..self.len.get()]).expect("inline string contains valid UTF-8")
    }
}

impl Serialize for InlineStr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for InlineStr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Older snapshots stored the length and the entire padded buffer.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Stored {
            Text(String),
            Legacy { len: InlineLen, bytes: [u8; 15] },
        }

        match Stored::deserialize(deserializer)? {
            Stored::Text(text) => Self::new(&text).ok_or_else(|| D::Error::custom("inline string exceeds capacity")),
            Stored::Legacy { len, bytes } => {
                let text = str::from_utf8(&bytes[..len.get()]).map_err(D::Error::custom)?;
                Ok(Self::new(text).expect("legacy length fits inline storage"))
            }
        }
    }
}

/// Byte length for an inline UTF-8 string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
#[repr(u8)]
enum InlineLen {
    #[serde(rename = "0")]
    L0,
    #[serde(rename = "1")]
    L1,
    #[serde(rename = "2")]
    L2,
    #[serde(rename = "3")]
    L3,
    #[serde(rename = "4")]
    L4,
    #[serde(rename = "5")]
    L5,
    #[serde(rename = "6")]
    L6,
    #[serde(rename = "7")]
    L7,
    #[serde(rename = "8")]
    L8,
    #[serde(rename = "9")]
    L9,
    #[serde(rename = "10")]
    L10,
    #[serde(rename = "11")]
    L11,
    #[serde(rename = "12")]
    L12,
    #[serde(rename = "13")]
    L13,
    #[serde(rename = "14")]
    L14,
    #[serde(rename = "15")]
    L15,
}

impl InlineLen {
    fn from_len(len: usize) -> Self {
        match len {
            0 => Self::L0,
            1 => Self::L1,
            2 => Self::L2,
            3 => Self::L3,
            4 => Self::L4,
            5 => Self::L5,
            6 => Self::L6,
            7 => Self::L7,
            8 => Self::L8,
            9 => Self::L9,
            10 => Self::L10,
            11 => Self::L11,
            12 => Self::L12,
            13 => Self::L13,
            14 => Self::L14,
            15 => Self::L15,
            _ => unreachable!("inline string length exceeds capacity"),
        }
    }

    fn get(self) -> usize {
        match self {
            Self::L0 => 0,
            Self::L1 => 1,
            Self::L2 => 2,
            Self::L3 => 3,
            Self::L4 => 4,
            Self::L5 => 5,
            Self::L6 => 6,
            Self::L7 => 7,
            Self::L8 => 8,
            Self::L9 => 9,
            Self::L10 => 10,
            Self::L11 => 11,
            Self::L12 => 12,
            Self::L13 => 13,
            Self::L14 => 14,
            Self::L15 => 15,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{types::StringIterator, value::Value};

    #[test]
    fn capacity_counts_utf8_bytes() {
        assert_eq!(size_of::<InlineStr>(), 16);
        assert_eq!(size_of::<Value>(), 16);
        for text in ["", "a", "é", "abcdefghijklmno", "éééééééa"] {
            assert_eq!(InlineStr::new(text).unwrap().as_str(), text);
        }
        assert!(InlineStr::new("abcdefghijklmnop").is_none());
        assert!(InlineStr::new("éééééééé").is_none());
    }

    #[test]
    fn values_serialize_as_text_and_read_legacy_buffers() {
        #[derive(Serialize)]
        enum LegacyValue {
            #[serde(rename = "S")]
            Inline { len: String, bytes: [u8; 15] },
        }

        for length in 0..=15 {
            let text = "x".repeat(length);
            let mut bytes = [0; 15];
            bytes[..length].fill(b'x');
            let legacy = LegacyValue::Inline {
                len: length.to_string(),
                bytes,
            };
            let encoded = minicbor_serde::to_vec(&legacy).unwrap();
            let restored: Value = minicbor_serde::from_slice(&encoded).unwrap();
            assert_eq!(restored.inline_str(), Some(text.as_str()));
            let expected = format!(r#"{{"S":"{text}"}}"#);
            assert_eq!(serde_json::to_string(&restored).unwrap(), expected);
            let compact = minicbor_serde::to_vec(&restored).unwrap();
            let reloaded: Value = minicbor_serde::from_slice(&compact).unwrap();
            assert_eq!(reloaded.inline_str(), Some(text.as_str()));
        }
    }
    #[test]
    fn legacy_iterator_preserves_its_position() {
        let stored = serde_json::json!({
            "source": {"Inline": {"len": "3", "bytes": [97, 195, 169, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]}},
            "byte_offset": 1,
            "ascii": false,
        });
        let iterator: StringIterator = serde_json::from_value(stored).unwrap();
        assert_eq!(
            serde_json::to_value(iterator).unwrap(),
            serde_json::json!({
                "source": {"Inline": "aé"}, "byte_offset": 1, "ascii": false,
            })
        );
    }
}
