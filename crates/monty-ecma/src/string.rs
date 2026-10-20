//! UTF-8 strings indexed by Unicode scalar value.

use std::{fmt, string::FromUtf16Error};

/// MontyScript strings cannot contain lone UTF-16 surrogates.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(crate) struct JsString(Box<str>);

impl JsString {
    pub fn empty() -> Self {
        Self("".into())
    }

    pub fn from_str(text: &str) -> Self {
        Self(text.into())
    }

    /// Panics if an internal buffer contains an invalid Unicode scalar value.
    pub fn from_units(units: &[u32]) -> Self {
        Self(
            units
                .iter()
                .map(|&unit| char::from_u32(unit).expect("valid scalar buffer"))
                .collect(),
        )
    }

    pub fn from_utf16(units: &[u16]) -> Result<Self, FromUtf16Error> {
        String::from_utf16(units).map(|text| Self(text.into()))
    }

    pub fn from_unit_iter(units: impl Iterator<Item = u16>) -> Result<Self, FromUtf16Error> {
        Self::from_utf16(&units.collect::<Vec<_>>())
    }

    pub fn len(&self) -> usize {
        self.0.chars().count()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn unit_at(&self, index: usize) -> Option<u32> {
        self.0.chars().nth(index).map(u32::from)
    }
    pub fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.chars().map(u32::from)
    }
    pub fn to_units(&self) -> Vec<u32> {
        self.units().collect()
    }
    pub fn to_utf16(&self) -> Vec<u16> {
        self.0.encode_utf16().collect()
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn slice(&self, start: usize, end: usize) -> Self {
        let text: String = self
            .0
            .chars()
            .skip(start.min(end))
            .take(end.saturating_sub(start))
            .collect();
        Self(text.into())
    }

    pub fn as_array_index(&self) -> Option<u32> {
        let bytes = self.0.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 10
            || (bytes.len() > 1 && bytes[0] == b'0')
            || !bytes.iter().all(u8::is_ascii_digit)
        {
            return None;
        }
        self.0.parse::<u32>().ok().filter(|index| *index != u32::MAX)
    }

    pub fn to_string_lossy(&self) -> String {
        self.0.to_string()
    }
}

impl fmt::Debug for JsString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
impl fmt::Display for JsString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl From<&str> for JsString {
    fn from(text: &str) -> Self {
        Self::from_str(text)
    }
}
