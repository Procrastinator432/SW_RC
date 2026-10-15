//! Compact lossless JSON audio; legacy byte arrays remain readable.
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserializer, Serializer,
};
const LIMIT: usize = 32 * 1024 * 1024;
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
    if bytes.len() > LIMIT {
        return Err(serde::ser::Error::custom("Wave exceeds byte budget"));
    }
    let mut text = String::with_capacity(7 + bytes.len().div_ceil(3) * 4);
    text.push_str("base64:");
    for c in bytes.chunks(3) {
        let a = c[0];
        let b = c.get(1).copied().unwrap_or(0);
        let d = c.get(2).copied().unwrap_or(0);
        text.push(ALPHABET[(a >> 2) as usize] as char);
        text.push(ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        text.push(if c.len() > 1 {
            ALPHABET[(((b & 15) << 2) | (d >> 6)) as usize] as char
        } else {
            '='
        });
        text.push(if c.len() > 2 {
            ALPHABET[(d & 63) as usize] as char
        } else {
            '='
        });
    }
    s.serialize_str(&text)
}
pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    struct Wave;
    impl<'de> Visitor<'de> for Wave {
        type Value = Vec<u8>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("bounded base64 wave or legacy byte array")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<u8>, A::Error> {
            let mut bytes = Vec::new();
            while let Some(b) = seq.next_element::<u8>()? {
                if bytes.len() >= LIMIT {
                    return Err(de::Error::custom("Wave exceeds byte budget"));
                }
                bytes.push(b);
            }
            Ok(bytes)
        }
        fn visit_str<E: de::Error>(self, s: &str) -> Result<Vec<u8>, E> {
            let data = s
                .strip_prefix("base64:")
                .ok_or_else(|| E::custom("Invalid wave encoding"))?
                .as_bytes();
            if data.len() % 4 != 0 || data.len() > LIMIT.div_ceil(3) * 4 {
                return Err(E::custom("Invalid base64 wave size"));
            }
            let value = |b| -> Result<u8, E> {
                ALPHABET
                    .iter()
                    .position(|x| *x == b)
                    .map(|v| v as u8)
                    .ok_or_else(|| E::custom("Invalid base64 character"))
            };
            let mut bytes = Vec::with_capacity((data.len() / 4 * 3).min(LIMIT));
            for (i, c) in data.chunks_exact(4).enumerate() {
                let a = value(c[0])?;
                let b = value(c[1])?;
                let last = i + 1 == data.len() / 4;
                if c[2] == b'=' {
                    if !last || c[3] != b'=' || b & 15 != 0 {
                        return Err(E::custom("Invalid base64 padding"));
                    }
                    bytes.push((a << 2) | (b >> 4));
                } else {
                    let v = value(c[2])?;
                    bytes.push((a << 2) | (b >> 4));
                    bytes.push((b << 4) | (v >> 2));
                    if c[3] == b'=' {
                        if !last || v & 3 != 0 {
                            return Err(E::custom("Invalid base64 padding"));
                        }
                    } else {
                        bytes.push((v << 6) | value(c[3])?);
                    }
                }
                if bytes.len() > LIMIT {
                    return Err(E::custom("Wave exceeds byte budget"));
                }
            }
            Ok(bytes)
        }
    }
    d.deserialize_any(Wave)
}
#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    #[derive(Serialize, Deserialize)]
    struct Audio {
        #[serde(with = "super")]
        wave: Vec<u8>,
    }
    #[test]
    fn compact_wave_preserves_all_values_and_legacy_arrays() {
        for len in 0..=260 {
            let bytes = (0..len).map(|v| v as u8).collect::<Vec<_>>();
            let json = serde_json::to_string(&Audio {
                wave: bytes.clone(),
            })
            .unwrap();
            assert!(json.contains("base64:"));
            assert_eq!(serde_json::from_str::<Audio>(&json).unwrap().wave, bytes);
        }
        assert_eq!(
            serde_json::from_str::<Audio>(r#"{"wave":[0,127,255]}"#)
                .unwrap()
                .wave,
            vec![0, 127, 255]
        );
        assert_eq!(
            serde_json::to_value(Audio {
                wave: b"Man".to_vec()
            })
            .unwrap()["wave"],
            "base64:TWFu"
        );
    }
    #[test]
    fn compact_wave_rejects_ambiguous_truncated_or_invalid_data() {
        for bad in [
            "TWFu",
            "base64:A",
            "base64:====",
            "base64:AA=A",
            "base64:AB==",
            "base64:AAB=",
            "base64:AA==AAAA",
            "base64:AA A",
            "base64:AA?=",
        ] {
            assert!(
                serde_json::from_value::<Audio>(serde_json::json!({"wave":bad})).is_err(),
                "{bad}"
            );
        }
        assert!(serde_json::from_str::<Audio>(r#"{"wave":[256]}"#).is_err());
        assert!(serde_json::from_str::<Audio>(r#"{"wave":[-1]}"#).is_err());
    }
}
