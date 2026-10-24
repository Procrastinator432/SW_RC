//! LoadMap's directed zone reachability, engine.dll 1041f33e..1041f4bc.
//! Each iteration SQUARES the current relation; this is not a shortest-path BFS.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Distances {
    pub bytes: Vec<u8>,
}
impl Distances {
    pub fn build(connectivity: &[u64]) -> Result<Self, String> {
        if connectivity.len() > 64 {
            return Err("Too many audio zones".into());
        }
        let mut reach = [0u64; 64];
        reach[..connectivity.len()].copy_from_slice(connectivity);
        let mut bytes = vec![255; 4096];
        for i in 0..64 {
            bytes[i * 64 + i] = 0;
        }
        for distance in 1..64u8 {
            for (i, &mask) in reach.iter().enumerate() {
                for j in 0..64 {
                    if mask & (1u64 << j) != 0 && bytes[i * 64 + j] > distance {
                        bytes[i * 64 + j] = distance;
                    }
                }
            }
            reach = std::array::from_fn(|i| {
                (0..64)
                    .filter(|&j| reach[i] & (1u64 << j) != 0)
                    .fold(0, |mask, j| mask | reach[j])
            });
        }
        Ok(Self { bytes })
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.bytes.len() != 4096 || (0..64).any(|i| self.bytes[i * 64 + i] != 0) {
            return Err("Invalid audio zone distances".into());
        }
        Ok(())
    }
    pub fn get(&self, source: u8, listener: u8) -> Result<u8, String> {
        if source >= 64 || listener >= 64 || self.bytes.len() != 4096 {
            return Err("Invalid audio zone distance query".into());
        }
        Ok(self.bytes[usize::from(source) * 64 + usize::from(listener)])
    }
}
/// FZoneProperties serializer 1045c230: actor, Connectivity, Visibility,
/// LastRenderTime. Only the first bitset feeds LoadMap's audio table.
pub fn read_connectivity(
    pkg: &crate::Package,
    data: &[u8],
    export: &crate::Export,
) -> Result<Vec<u64>, String> {
    let geometry = crate::geometry::read_bsp(pkg, data, export)?;
    let payload = pkg.payload(data, export)?;
    let mut r = crate::Reader::at(payload, geometry.geometry_bytes)?;
    r.u32()?;
    let count = r.u32()?;
    if count > 64 {
        return Err("Too many model zones".into());
    }
    let mut connectivity = vec![];
    for _ in 0..count {
        pkg.object_path(r.index()?)?;
        let mask = u64::from(r.u32()?) | (u64::from(r.u32()?) << 32);
        connectivity.push(mask);
        r.take(12)?;
    }
    Ok(connectivity)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_zone_table_fixtures_preserve_directed_power_reachability() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/zone-distances-native.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let links: Vec<_> = case["connectivity_hex"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| u64::from_str_radix(v.as_str().unwrap(), 16).unwrap())
                .collect();
            let table = Distances::build(&links).unwrap();
            let hex: String = table.bytes.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(
                hex,
                case["distances_hex"].as_str().unwrap(),
                "{}",
                case["name"]
            );
        }
        let chain: Vec<_> = (0..64)
            .map(|i| if i < 63 { 1 << (i + 1) } else { 0 })
            .collect();
        let table = Distances::build(&chain).unwrap();
        assert_eq!(table.get(0, 3).unwrap(), 255); // NOT a three-hop shortest path.
        assert_eq!(table.get(0, 4).unwrap(), 3);
        assert_eq!(table.get(4, 0).unwrap(), 255);
        assert!(Distances::build(&[0; 65]).is_err());
        assert!(table.get(64, 0).is_err());
    }
}
