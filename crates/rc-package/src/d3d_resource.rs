//! Native resource hash/list management, D3DDrv 10015040 and 1002a110..1002a250.
//! Bounded captured wrapper images; COM release calls are planned, not executed.
use crate::d3d_upload::Command;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
pub const BUCKETS: usize = 4096;
pub fn hash(low: u32) -> usize {
    ((((low >> 4) & 0xff0) + ((low >> 16) & 0xf)) ^ (low & 0xfff)) as usize
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Kind {
    Base,
    Vertex,
    Index,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    pub address: u32,
    pub bytes: Vec<u8>,
}
impl Node {
    fn word(&self, offset: usize) -> u32 {
        u32::from_le_bytes(self.bytes[offset..offset + 4].try_into().unwrap())
    }
    fn put(&mut self, offset: usize, value: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cache {
    pub device: u32,
    pub head: u32,
    pub buckets: Vec<u32>,
    pub nodes: Vec<Node>,
}
impl Cache {
    /// Typed upload view over a captured native wrapper, without changing list bytes.
    pub fn upload_resource(
        &self,
        address: u32,
        kind: Kind,
    ) -> Result<crate::d3d_upload::Resource, String> {
        self.validate()?;
        let n = &self.nodes[self.index(address)?];
        let vtable = match kind {
            Kind::Vertex => 0x10072c10,
            Kind::Index => 0x10072d28,
            Kind::Base => return Err("Base wrapper has no static upload layout".into()),
        };
        if n.word(0) != vtable || n.word(4) != self.device {
            return Err("Static wrapper kind or owner mismatch".into());
        }
        Ok(crate::d3d_upload::Resource {
            address,
            handle: n.word(0x30),
            capacity: n.word(if kind == Kind::Vertex { 0x38 } else { 0x34 }),
            revision: n.word(0x10),
            source: n.word(0x3c),
        })
    }
    /// Commit only fields written by the typed upload; preserve links and unknown bytes.
    pub fn store_upload(
        &mut self,
        resource: &crate::d3d_upload::Resource,
        kind: Kind,
        frame: u32,
    ) -> Result<(), String> {
        self.upload_resource(resource.address, kind)?;
        let index = self.index(resource.address)?;
        let n = &mut self.nodes[index];
        n.put(0x30, resource.handle);
        n.put(
            if kind == Kind::Vertex { 0x38 } else { 0x34 },
            resource.capacity,
        );
        n.put(0x10, resource.revision);
        n.put(0x14, frame);
        if kind == Kind::Vertex {
            n.put(0x3c, resource.source);
        }
        Ok(())
    }
    fn validate(&self) -> Result<(), String> {
        if self.device == 0
            || self.device.checked_add(0x40b4).is_none()
            || self.buckets.len() != BUCKETS
            || self.nodes.len() > BUCKETS
        {
            return Err("Invalid bounded resource cache device/buckets/count".into());
        }
        let mut seen = HashSet::new();
        if self.nodes.iter().any(|n| {
            n.address == 0
                || n.address.checked_add(n.bytes.len() as u32).is_none()
                || !matches!(n.bytes.len(), 64 | 72)
                || !seen.insert(n.address)
        }) {
            return Err("Invalid or duplicate resource wrapper image".into());
        }
        Ok(())
    }
    fn index(&self, address: u32) -> Result<usize, String> {
        self.nodes
            .iter()
            .position(|n| n.address == address)
            .ok_or_else(|| format!("Uncaptured resource wrapper {address:08x}"))
    }
    fn chain(&self, mut address: u32, offset: usize) -> Result<Vec<usize>, String> {
        let mut seen = HashSet::new();
        let mut out = vec![];
        while address != 0 {
            if !seen.insert(address) {
                return Err("Cyclic resource chain".into());
            }
            let index = self.index(address)?;
            out.push(index);
            address = self.nodes[index].word(offset);
        }
        Ok(out)
    }
    /// Full 64-bit comparison, newest matching node first; only low DWORD is hashed.
    pub fn lookup(&self, key: [u32; 2]) -> Result<Option<u32>, String> {
        self.validate()?;
        let mut address = self.buckets[hash(key[0])];
        let mut seen = HashSet::new();
        while address != 0 {
            if !seen.insert(address) {
                return Err("Cyclic resource lookup chain".into());
            }
            let n = &self.nodes[self.index(address)?];
            if [n.word(8), n.word(12)] == key {
                return Ok(Some(address));
            }
            address = n.word(0x2c);
        }
        Ok(None)
    }
    /// Initialize a supplied allocation image, prepend to global and hash chains.
    /// Unknown bytes are preserved; repeated insertion of a linked wrapper is rejected.
    pub fn insert(&mut self, address: u32, key: [u32; 2], kind: Kind) -> Result<(), String> {
        self.validate()?;
        let index = self.index(address)?;
        let bucket = hash(key[0]);
        if self.chain(self.head, 0x28)?.contains(&index)
            || self.chain(self.buckets[bucket], 0x2c)?.contains(&index)
        {
            return Err("Resource wrapper is already linked".into());
        }
        let n = &mut self.nodes[index];
        n.put(0, 0x10072bfc);
        n.put(4, self.device);
        n.put(8, key[0]);
        n.put(12, key[1]);
        n.put(0x10, 0);
        n.put(0x14, 0);
        n.put(0x18, bucket as u32);
        n.bytes[0x1c] = 0;
        n.bytes[0x1d] = 0;
        n.put(0x20, 0);
        n.put(0x24, 0);
        n.put(0x28, self.head);
        n.put(0x2c, self.buckets[bucket]);
        match kind {
            Kind::Base => {}
            Kind::Vertex => {
                n.put(0, 0x10072c10);
                for o in [0x30, 0x34, 0x38, 0x3c] {
                    n.put(o, 0);
                }
            }
            Kind::Index => {
                n.put(0, 0x10072d28);
                n.put(0x30, 0);
                n.put(0x34, 0);
            }
        }
        self.head = address;
        self.buckets[bucket] = address;
        Ok(())
    }
    /// Base destructor unlinks both chains, keeps allocation and resource payload.
    pub fn unlink(&mut self, address: u32) -> Result<(), String> {
        self.validate()?;
        let index = self.index(address)?;
        let bucket = self.nodes[index].word(0x18) as usize;
        if bucket >= BUCKETS || self.nodes[index].word(4) != self.device {
            return Err("Invalid resource owner/stored bucket".into());
        }
        let all = self.chain(self.head, 0x28)?;
        let hashed = self.chain(self.buckets[bucket], 0x2c)?;
        let next_all = self.nodes[index].word(0x28);
        let next_hash = self.nodes[index].word(0x2c);
        if let Some(position) = all.iter().position(|i| *i == index) {
            if position == 0 {
                self.head = next_all;
            } else {
                self.nodes[all[position - 1]].put(0x28, next_all);
            }
        }
        if let Some(position) = hashed.iter().position(|i| *i == index) {
            if position == 0 {
                self.buckets[bucket] = next_hash;
            } else {
                self.nodes[hashed[position - 1]].put(0x2c, next_hash);
            }
        }
        self.nodes[index].put(0, 0x10072bfc);
        self.nodes[index].put(0x28, 0);
        self.nodes[index].put(0x2c, 0);
        Ok(())
    }
    /// Vertex reset releases +30 then +34 and clears each; list membership is preserved.
    pub fn reset_vertex(&mut self, address: u32) -> Result<Vec<Command>, String> {
        self.validate()?;
        let index = self.index(address)?;
        let n = &mut self.nodes[index];
        let mut calls = vec![];
        for offset in [0x30, 0x34] {
            let handle = n.word(offset);
            if handle != 0 {
                calls.push(Command {
                    receiver: handle,
                    vtable_offset: 8,
                    arguments: vec![],
                });
                n.put(offset, 0);
            }
        }
        Ok(calls)
    }
    /// Index destructor releases +30, then base-unlinks; it does not clear the handle.
    /// No allocator free or vertex reset is implied by this operation.
    pub fn destroy_index(&mut self, address: u32) -> Result<Vec<Command>, String> {
        self.validate()?;
        let handle = self.nodes[self.index(address)?].word(0x30);
        let mut next = self.clone();
        next.unlink(address)?;
        let calls = if handle == 0 {
            vec![]
        } else {
            vec![Command {
                receiver: handle,
                vtable_offset: 8,
                arguments: vec![],
            }]
        };
        *self = next;
        Ok(calls)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Cache {
        Cache {
            device: 0x22000000,
            head: 0,
            buckets: vec![0; BUCKETS],
            nodes: (0..4)
                .map(|i| Node {
                    address: 0x34000000 + i * 0x100,
                    bytes: vec![0xcc; 64],
                })
                .collect(),
        }
    }
    #[test]
    fn hash_uses_low_word_only_and_stays_in_table() {
        for low in [0, 1, 0xfff, 0xffff, 0x12345678, u32::MAX] {
            assert!(hash(low) < BUCKETS);
        }
        assert_eq!(hash(0), 0);
        assert_eq!(hash(0x10000000), 0);
    }
    #[test]
    fn upload_bridge_preserves_links_padding_and_index_source_tail() {
        let mut c = fixture();
        c.insert(0x34000000, [1, 2], Kind::Index).unwrap();
        let before = c.clone();
        let mut r = c.upload_resource(0x34000000, Kind::Index).unwrap();
        r.handle = 7;
        r.capacity = 9;
        r.revision = 11;
        r.source = 13;
        c.store_upload(&r, Kind::Index, 15).unwrap();
        assert_eq!(c.head, before.head);
        assert_eq!(c.buckets, before.buckets);
        for i in 0..64 {
            if ![0x10..0x18, 0x30..0x38]
                .iter()
                .any(|range| range.contains(&i))
            {
                assert_eq!(c.nodes[0].bytes[i], before.nodes[0].bytes[i]);
            }
        }
        assert_eq!(c.nodes[0].word(0x3c), 0xcccccccc);
        assert_eq!(c.nodes[0].word(0x14), 15);
    }
    #[test]
    fn upload_bridge_checks_type_owner_and_vertex_capacity_offset() {
        let mut c = fixture();
        c.insert(0x34000000, [1, 2], Kind::Vertex).unwrap();
        let mut r = c.upload_resource(0x34000000, Kind::Vertex).unwrap();
        r.capacity = 64;
        r.source = 99;
        c.store_upload(&r, Kind::Vertex, 3).unwrap();
        assert_eq!(c.nodes[0].word(0x38), 64);
        assert_eq!(c.nodes[0].word(0x34), 0);
        assert_eq!(c.nodes[0].word(0x3c), 99);
        assert!(c.upload_resource(r.address, Kind::Index).is_err());
        c.nodes[0].put(4, 1);
        let before = c.clone();
        assert!(c.store_upload(&r, Kind::Vertex, 4).is_err());
        assert_eq!(c, before);
    }
    #[test]
    fn lookup_distinguishes_high_word_in_collision_chain() {
        let mut c = fixture();
        c.insert(0x34000000, [1, 2], Kind::Base).unwrap();
        c.insert(0x34000100, [1, 3], Kind::Base).unwrap();
        assert_eq!(c.lookup([1, 2]).unwrap(), Some(0x34000000));
        assert_eq!(c.lookup([1, 3]).unwrap(), Some(0x34000100));
        assert_eq!(c.lookup([1, 4]).unwrap(), None);
    }
    #[test]
    fn duplicate_keys_return_newest_wrapper() {
        let mut c = fixture();
        for a in [0x34000000, 0x34000100] {
            c.insert(a, [1, 2], Kind::Base).unwrap();
        }
        assert_eq!(c.lookup([1, 2]).unwrap(), Some(0x34000100));
    }
    #[test]
    fn base_initialization_preserves_unknown_bytes() {
        let mut c = fixture();
        c.insert(0x34000000, [1, 2], Kind::Base).unwrap();
        assert_eq!(&c.nodes[0].bytes[0x1c..0x20], &[0, 0, 0xcc, 0xcc]);
        assert_eq!(c.nodes[0].word(0x30), 0xcccccccc);
        assert_eq!(c.nodes[0].word(0x14), 0);
    }
    #[test]
    fn typed_initialization_differs_after_index_capacity() {
        let mut c = fixture();
        c.insert(0x34000000, [1, 2], Kind::Index).unwrap();
        c.insert(0x34000100, [2, 3], Kind::Vertex).unwrap();
        assert_eq!(c.nodes[0].word(0x38), 0xcccccccc);
        assert_eq!(c.nodes[1].word(0x38), 0);
        assert_eq!(c.nodes[1].word(0x3c), 0);
    }
    #[test]
    fn unlink_middle_repairs_both_predecessors() {
        let mut c = fixture();
        for a in [0x34000000, 0x34000100, 0x34000200] {
            c.insert(a, [1, 2], Kind::Base).unwrap();
        }
        c.unlink(0x34000100).unwrap();
        assert_eq!(c.nodes[2].word(0x28), 0x34000000);
        assert_eq!(c.nodes[2].word(0x2c), 0x34000000);
        assert_eq!(c.nodes[1].word(0x28), 0);
    }
    #[test]
    fn unlink_head_then_last_empties_lists() {
        let mut c = fixture();
        c.insert(0x34000000, [1, 2], Kind::Base).unwrap();
        c.unlink(0x34000000).unwrap();
        assert_eq!(c.head, 0);
        assert_eq!(c.buckets[hash(1)], 0);
        c.unlink(0x34000000).unwrap();
    }
    #[test]
    fn repeated_insert_is_atomic_error() {
        let mut c = fixture();
        c.insert(0x34000000, [1, 2], Kind::Base).unwrap();
        let before = c.clone();
        assert!(c.insert(0x34000000, [2, 3], Kind::Base).is_err());
        assert_eq!(c, before);
    }
    #[test]
    fn cycles_and_missing_nodes_are_safe_errors() {
        let mut c = fixture();
        c.insert(0x34000000, [1, 2], Kind::Base).unwrap();
        c.nodes[0].put(0x2c, 0x34000000);
        assert!(c.lookup([1, 3]).is_err());
        assert_eq!(c.lookup([1, 2]).unwrap(), Some(0x34000000));
        let before = c.clone();
        assert!(c.unlink(0x34000000).is_err());
        assert_eq!(c, before);
        c.buckets[hash(1)] = 99;
        assert!(c.lookup([1, 2]).is_err());
    }
    #[test]
    fn vertex_reset_preserves_lists_and_releases_equal_handles_twice() {
        let mut c = fixture();
        c.insert(0x34000000, [1, 2], Kind::Vertex).unwrap();
        c.nodes[0].put(0x30, 7);
        c.nodes[0].put(0x34, 7);
        let calls = c.reset_vertex(0x34000000).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].receiver, 7);
        assert_eq!(c.head, 0x34000000);
        assert_eq!(c.nodes[0].word(0x34), 0);
        assert!(c.reset_vertex(0x34000000).unwrap().is_empty());
    }
    #[test]
    fn index_destructor_preserves_handle_and_capacity() {
        let mut c = fixture();
        c.insert(0x34000000, [1, 2], Kind::Index).unwrap();
        c.nodes[0].put(0x30, 7);
        c.nodes[0].put(0x34, 128);
        assert_eq!(c.destroy_index(0x34000000).unwrap().len(), 1);
        assert_eq!(c.head, 0);
        assert_eq!(c.nodes[0].word(0x30), 7);
        assert_eq!(c.nodes[0].word(0x34), 128);
        assert_eq!(c.nodes[0].word(0), 0x10072bfc);
    }
    #[test]
    fn invalid_stored_bucket_preserves_destructor_input() {
        let mut c = fixture();
        c.insert(0x34000000, [1, 2], Kind::Index).unwrap();
        c.nodes[0].put(0x18, 4096);
        let before = c.clone();
        assert!(c.destroy_index(0x34000000).is_err());
        assert_eq!(c, before);
    }
}
