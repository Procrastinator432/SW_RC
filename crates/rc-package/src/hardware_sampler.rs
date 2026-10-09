//! Exact raw resource/address/filter-state writes from D3DDrv 100044b0.
//! Enum meanings and eventual GPU state application are intentionally not inferred.
use crate::hardware_stages::Stage;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Texture {
    pub address_u: u8,
    pub address_v: u8,
    pub flags_7c: u8,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Resource {
    pub address: u32,
    pub word_3c: u32,
}
pub fn bind(
    stage: &mut Stage,
    renderer_flags: &mut u32,
    texture: Option<Texture>,
    resource: Resource,
    filter: u32,
) {
    let Some(texture) = texture else {
        stage[0] = 0;
        return;
    };
    stage[0] = resource.address;
    if texture.flags_7c & 1 != 0 {
        *renderer_flags |= 1;
    }
    let mut flags = stage[2];
    if resource.address != 0 && resource.word_3c != 0 {
        flags = (flags & !0xff) | (filter & 15) | ((filter << 4) & 0xf0);
    } else {
        for (byte, shift) in [(texture.address_u, 0), (texture.address_v, 4)] {
            if byte < 2 {
                flags = (flags & !(15 << shift)) | ((if byte == 0 { 1 } else { 3 }) << shift);
            }
        }
    }
    stage[2] = (flags & !0xf00) | ((filter << 8) & 0xf00);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn null_texture_changes_only_resource_word() {
        let mut s = [0xa5a5a5a5; 28];
        let mut flags = 0x8000;
        bind(
            &mut s,
            &mut flags,
            None,
            Resource {
                address: 9,
                word_3c: 1,
            },
            7,
        );
        assert_eq!(s[0], 0);
        assert_eq!(s[2], 0xa5a5a5a5);
        assert_eq!(flags, 0x8000);
    }
    #[test]
    fn resource_gate_unknown_address_modes_and_full_filter_word() {
        let texture = Texture {
            address_u: 255,
            address_v: 0,
            flags_7c: 1,
        };
        let mut s = [0xa5a5a5a5; 28];
        let mut flags = 0x8000;
        bind(
            &mut s,
            &mut flags,
            Some(texture),
            Resource {
                address: 9,
                word_3c: 0,
            },
            0xfffffffd,
        );
        assert_eq!(s[2], 0xa5a5ad15);
        assert_eq!(flags, 0x8001);
        bind(
            &mut s,
            &mut flags,
            Some(texture),
            Resource {
                address: 9,
                word_3c: 1,
            },
            0xfffffffe,
        );
        assert_eq!(s[2], 0xa5a5aeee);
        assert_eq!(s[0], 9);
    }
}
