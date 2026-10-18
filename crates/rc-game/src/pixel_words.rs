//! Lossless compact material pixels; legacy u32 JSON arrays remain readable.
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserializer, Serializer,
};
const WORDS: usize = 8 * 1024 * 1024;
pub fn serialize<S: Serializer>(pixels: &[u32], serializer: S) -> Result<S::Ok, S::Error> {
    if pixels.len() > WORDS {
        return Err(serde::ser::Error::custom("Pixel word budget exceeded"));
    }
    let bytes: Vec<_> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
    crate::wave_bytes::serialize(&bytes, serializer)
}
pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u32>, D::Error> {
    struct Pixels;
    impl<'de> Visitor<'de> for Pixels {
        type Value = Vec<u32>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("bounded base64 little-endian pixel words or legacy u32 array")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut words = vec![];
            while let Some(word) = seq.next_element::<u32>()? {
                if words.len() >= WORDS {
                    return Err(de::Error::custom("Pixel word budget exceeded"));
                }
                words.push(word);
            }
            Ok(words)
        }
        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            let bytes =
                crate::wave_bytes::deserialize(serde::de::value::StrDeserializer::<E>::new(value))?;
            if bytes.len() % 4 != 0 {
                return Err(de::Error::custom("Incomplete pixel word"));
            }
            Ok(bytes
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect())
        }
    }
    d.deserialize_any(Pixels)
}
#[cfg(test)]
mod tests {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Fixture {
        #[serde(with = "super")]
        pixels: Vec<u32>,
    }
    #[test]
    fn compact_and_legacy_preserve_all_channels_and_reject_incomplete_words() {
        let f = Fixture {
            pixels: vec![0, 0x7f123456, u32::MAX],
        };
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("base64:"));
        assert_eq!(
            serde_json::from_str::<Fixture>(&json).unwrap().pixels,
            f.pixels
        );
        assert_eq!(
            serde_json::from_str::<Fixture>(r#"{"pixels":[0,2131899478,4294967295]}"#)
                .unwrap()
                .pixels,
            f.pixels
        );
        for json in [
            r#"{"pixels":"base64:AQ=="}"#,
            r#"{"pixels":[4294967296]}"#,
            r#"{"pixels":"base64:!"}"#,
        ] {
            assert!(serde_json::from_str::<Fixture>(json).is_err());
        }
    }
}
