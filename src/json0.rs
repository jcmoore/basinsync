//! JSON0 scalar adapters at the HTTP boundary.
use crate::lexidecimal::Viewpoint;
use base64::{
    Engine,
    alphabet::{STANDARD, URL_SAFE},
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use num_bigint::BigUint;
use serde::{Deserialize, Deserializer, Serializer, de::Error};

pub fn serialize_text<S: Serializer>(value: &str, serializer: S) -> Result<S::Ok, S::Error> {
    if value.starts_with('0') {
        serializer.serialize_str(&format!("0{value}"))
    } else {
        serializer.serialize_str(value)
    }
}

pub fn serialize_viewpoint<S: Serializer>(
    value: &Viewpoint,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&format!("0x{}", value.hexadecimal()))
}

pub fn serialize_biguint<S: Serializer>(value: &BigUint, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&format!("0x{value:x}"))
}

pub fn deserialize_text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let wire = String::deserialize(deserializer)?;
    if let Some(value) = wire.strip_prefix("00") {
        return Ok(format!("0{value}"));
    }
    // Any other leading `0`, including a lone "0", is a non-text encoding.
    if wire.starts_with('0') {
        return Err(D::Error::custom("expected a JSON0 literal string"));
    }
    Ok(wire)
}

pub fn deserialize_text_map<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<std::collections::BTreeMap<String, String>, D::Error> {
    #[derive(Deserialize)]
    struct Text(#[serde(deserialize_with = "deserialize_text")] String);
    let values = std::collections::BTreeMap::<String, Text>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .map(|(key, value)| (key, value.0))
        .collect())
}

pub fn serialize_text_map<S: Serializer>(
    values: &std::collections::BTreeMap<String, String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    #[derive(serde::Serialize)]
    struct Text<'a>(#[serde(serialize_with = "serialize_text")] &'a str);
    let mut map = serializer.serialize_map(Some(values.len()))?;
    for (key, value) in values {
        map.serialize_entry(key, &Text(value))?;
    }
    map.end()
}

pub fn deserialize_viewpoint<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Viewpoint, D::Error> {
    let wire = String::deserialize(deserializer)?;
    let hex = wire
        .strip_prefix("0x")
        .filter(|hex| !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| D::Error::custom("viewpoint must be a nonnegative JSON0 bigint"))?;
    let integer = BigUint::parse_bytes(hex.as_bytes(), 16)
        .ok_or_else(|| D::Error::custom("invalid viewpoint"))?;
    Viewpoint::parse(&integer.to_str_radix(10)).map_err(D::Error::custom)
}

pub fn deserialize_bytes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    let wire = String::deserialize(deserializer)?;
    let alphabet = match wire.get(..2) {
        Some("0*") => &URL_SAFE,
        Some("0%") => &STANDARD,
        _ => return Err(D::Error::custom("sha384 must be JSON0 bytes")),
    };
    let config = GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true);
    let encoded: Vec<u8> = wire.as_bytes()[2..]
        .iter()
        .copied()
        .filter(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0c))
        .collect();
    GeneralPurpose::new(alphabet, config)
        .decode(encoded)
        .map_err(D::Error::custom)
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Text(
        #[serde(
            serialize_with = "super::serialize_text",
            deserialize_with = "super::deserialize_text"
        )]
        String,
    );

    #[test]
    fn literal_zero_strings_gain_a_zero_and_a_lone_zero_is_rejected() {
        for (value, wire) in [("0", "\"00\""), ("00", "\"000\""), ("0x1", "\"00x1\""), ("", "\"\""), ("a0", "\"a0\"")] {
            assert_eq!(serde_json::to_string(&Text(value.into())).unwrap(), wire);
            assert_eq!(serde_json::from_str::<Text>(wire).unwrap(), Text(value.into()));
        }
        for invalid in ["\"0\"", "\"0x1\"", "\"0z\""] {
            assert!(serde_json::from_str::<Text>(invalid).is_err(), "{invalid}");
        }
    }
}
