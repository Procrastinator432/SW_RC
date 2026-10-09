//! Vertex-mesh section traversal/material routing from Engine!105607b4..10560909.
//! Geometry, resolver/accessor answers and prepared material passes remain inputs.
use crate::{
    d3d_complete::Complete,
    d3d_draw::{self, Draw, PreparedPass, RenderedPass},
    d3d_dynamic_draw::{Caller, Plan as UploadPlan, Request as UploadRequest, Runtime},
    d3d_pools::{IndexRequest, VertexRequest},
};
use serde::{Deserialize, Serialize};

fn signed_count(packed: u32) -> i32 {
    (packed.wrapping_shl(3) as i32) >> 3
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sections {
    pub packed_count: u32,
    /// Original array image, stride 0x50; unused bytes remain uninterpreted.
    pub bytes: Vec<u8>,
    /// Previously computed counts from the native local array.
    pub primitive_counts: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Section {
    pub index: usize,
    pub material_index: u16,
    pub first_index: u16,
    pub min_vertex: u16,
    pub max_vertex: u16,
    pub primitive_count: u32,
}
impl Sections {
    pub fn active(&self) -> Result<Vec<Section>, String> {
        let count = signed_count(self.packed_count).max(0) as usize;
        if count > 256 {
            return Err("Mesh section count exceeds safe limit".into());
        }
        if self.bytes.len() < count * 0x50 || self.primitive_counts.len() < count {
            return Err("Truncated mesh section/count array".into());
        }
        let mut out = vec![];
        for index in 0..count {
            let entry = &self.bytes[index * 0x50..(index + 1) * 0x50];
            let word = |offset| u16::from_le_bytes([entry[offset], entry[offset + 1]]);
            let primitive_count = self.primitive_counts[index];
            if primitive_count != 0 && word(0x10) != 0 {
                out.push(Section {
                    index,
                    material_index: word(0),
                    first_index: word(2),
                    min_vertex: word(4),
                    max_vertex: word(6),
                    primitive_count,
                });
            }
        }
        Ok(out)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Materials {
    pub renderer_address: u32,
    pub mesh_address: u32,
    pub actor_address: u32,
    pub actor_object: u32,
    /// Answer to mesh interface+a0(actor).
    pub resolver: u32,
    /// Native material-slot records contain the token at material_index*8+4.
    pub slots: Vec<u32>,
    /// Answer to resolver+d8(slot, actor), indexed by original section index.
    pub resolved: Vec<u32>,
    pub wire: bool,
    pub debug: u32,
    pub override_packed_count: u32,
    /// Answers to actor_object+cc(material_index).
    pub overrides: Vec<u32>,
    /// Already-created shader at 108b9b28; lazy shader creation is external.
    pub wire_shader: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BoundaryCall {
    pub receiver: u32,
    pub vtable_offset: u32,
    pub arguments: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Selection {
    pub material: u32,
    pub calls: Vec<BoundaryCall>,
}
impl Materials {
    pub fn select(&self, section: &Section) -> Result<Selection, String> {
        let i = usize::from(section.material_index);
        let slot = *self.slots.get(i).ok_or("Missing mesh material slot")?;
        let resolved = *self
            .resolved
            .get(section.index)
            .ok_or("Missing resolver answer")?;
        if self.mesh_address == 0 || self.resolver == 0 || self.renderer_address == 0 {
            return Err("Null mesh/resolver receiver".into());
        }
        let mut calls = vec![
            BoundaryCall {
                receiver: self.mesh_address,
                vtable_offset: 0xa0,
                arguments: vec![self.actor_address],
            },
            BoundaryCall {
                receiver: self.resolver,
                vtable_offset: 0xd8,
                arguments: vec![slot, self.actor_address],
            },
        ];
        let material = if self.wire || self.debug != 0 {
            if self.wire_shader == 0 {
                return Err("Wire shader creation answer required".into());
            }
            self.wire_shader
        } else if i as i32 >= signed_count(self.override_packed_count) {
            resolved
        } else {
            if self.actor_object == 0 {
                return Err("Null actor override receiver".into());
            }
            let answer = *self
                .overrides
                .get(i)
                .ok_or("Missing actor material override answer")?;
            calls.push(BoundaryCall {
                receiver: self.actor_object,
                vtable_offset: 0xcc,
                arguments: vec![i as u32],
            });
            answer
        };
        // SetMaterial itself is an explicit boundary: the bank supplies its result.
        calls.push(BoundaryCall {
            receiver: self.renderer_address,
            vtable_offset: 0x50,
            arguments: vec![material, 0, 0, 0],
        });
        Ok(Selection { material, calls })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialPasses {
    pub material: u32,
    pub passes: Vec<PreparedPass>,
}
fn same(a: &PreparedPass, b: &PreparedPass) -> bool {
    a.pass == b.pass
        && a.resources == b.resources
        && a.shader == b.shader
        && a.fog_color == b.fog_color
}
fn validate_bank(bank: &[MaterialPasses]) -> Result<(), String> {
    if bank.len() > 256 {
        return Err("Material bank exceeds safe limit".into());
    }
    let mut seen: Vec<&PreparedPass> = vec![];
    for (i, material) in bank.iter().enumerate() {
        if bank[..i].iter().any(|m| m.material == material.material) {
            return Err("Duplicate material bank token".into());
        }
        if material.passes.len() > 8 {
            return Err("Material pass count exceeds safe limit".into());
        }
        for p in &material.passes {
            if seen
                .iter()
                .any(|q| q.pass.address == p.pass.address && !same(q, p))
            {
                return Err("Conflicting pass aliases across mesh materials".into());
            }
            seen.push(p);
        }
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub vertex: VertexRequest,
    pub index: IndexRequest,
    pub sections: Sections,
    pub materials: Materials,
    pub bank: Vec<MaterialPasses>,
    pub context: d3d_draw::Context,
}
#[derive(Clone, Debug, Serialize)]
pub struct SectionPlan {
    pub section: Section,
    pub selection: Selection,
    pub draw: Draw,
    pub rendered: Vec<RenderedPass>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub upload: UploadPlan,
    pub sections: Vec<SectionPlan>,
}
impl Runtime {
    /// One vertex/index upload followed by original ordered section draws.
    /// On any failure, runtime, CPU mirrors and the entire request are preserved.
    pub fn submit_mesh(&mut self, request: &mut Request) -> Result<Plan, String> {
        let active = request.sections.active()?;
        validate_bank(&request.bank)?;
        let mut next = self.clone();
        let mut q = request.clone();
        let mut upload_request = UploadRequest {
            caller: Caller::VertMesh { sections: vec![] },
            vertex: q.vertex.clone(),
            index: Some(q.index.clone()),
            passes: vec![],
            context: q.context,
        };
        let upload = next.submit(&mut upload_request)?;
        q.index = upload_request.index.ok_or("Missing mesh index upload")?;
        let offset = upload
            .index
            .as_ref()
            .ok_or("Missing mesh index offset")?
            .offset;
        let mut complete = Complete {
            deferred: next.buffers.deferred.clone(),
            lights: next.lights.clone(),
        };
        let mut sections = vec![];
        for section in active {
            let selection = q.materials.select(&section)?;
            let material = q
                .bank
                .iter()
                .position(|m| m.material == selection.material)
                .ok_or("Missing selected material passes")?;
            let draw = Draw {
                primitive: 5,
                start: offset.wrapping_add(u32::from(section.first_index)),
                primitive_count: section.primitive_count,
                min_vertex: u32::from(section.min_vertex),
                max_vertex: u32::from(section.max_vertex),
                indexed: true,
            };
            let rendered = d3d_draw::render(
                &mut complete,
                &mut next.last_pass,
                &mut next.counters,
                &mut q.bank[material].passes,
                q.context,
                draw,
            )?;
            // Each address is one native pass object, even across material banks.
            for snapshot in q.bank[material].passes.clone() {
                for p in q
                    .bank
                    .iter_mut()
                    .flat_map(|m| &mut m.passes)
                    .filter(|p| p.pass.address == snapshot.pass.address)
                {
                    p.pass = snapshot.pass.clone();
                }
            }
            sections.push(SectionPlan {
                section,
                selection,
                draw,
                rendered,
            });
        }
        next.buffers.deferred = complete.deferred;
        next.lights = complete.lights;
        *self = next;
        *request = q;
        Ok(Plan { upload, sections })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn raw() -> Sections {
        let mut bytes = vec![0; 160];
        for (off, value) in [(0, 2u16), (2, 65535), (4, 9), (6, 3), (16, 0x8000)] {
            bytes[off..off + 2].copy_from_slice(&value.to_le_bytes());
        }
        bytes[80 + 16] = 1;
        Sections {
            packed_count: 2,
            bytes,
            primitive_counts: vec![7, 9],
        }
    }
    fn section() -> Section {
        raw().active().unwrap()[0].clone()
    }
    fn materials() -> Materials {
        Materials {
            renderer_address: 1,
            mesh_address: 2,
            actor_address: 3,
            actor_object: 4,
            resolver: 5,
            slots: vec![10, 11, 12],
            resolved: vec![20, 21],
            wire: false,
            debug: 0,
            override_packed_count: 0,
            overrides: vec![30, 31, 32],
            wire_shader: 40,
        }
    }
    fn pass() -> PreparedPass {
        PreparedPass {
            pass: crate::d3d_pass::Pass {
                address: 100,
                header: [0; 28],
                color_write: 0,
                stages: [[0; 28]; 8],
            },
            resources: [None; 8],
            shader: crate::d3d_complete::ShaderChoice {
                hardware: true,
                resolved: None,
            },
            fog_color: 0,
        }
    }
    #[test]
    fn raw_stride_and_unsigned_fields_are_preserved() {
        let a = raw().active().unwrap();
        assert_eq!(a.len(), 2);
        assert_eq!(
            (
                a[0].first_index,
                a[0].min_vertex,
                a[0].max_vertex,
                a[1].index
            ),
            (65535, 9, 3, 1)
        );
    }
    #[test]
    fn packed_upper_flags_do_not_change_positive_iteration() {
        let mut s = raw();
        s.packed_count = 0xe0000002;
        assert_eq!(s.active().unwrap().len(), 2);
    }
    #[test]
    fn signed_nonpositive_count_never_reads_arrays() {
        for packed_count in [0, 0xe0000000, 0x1fffffff, 0x10000000] {
            assert!(Sections {
                packed_count,
                bytes: vec![],
                primitive_counts: vec![]
            }
            .active()
            .unwrap()
            .is_empty());
        }
    }
    #[test]
    fn raw_section_extent_is_checked() {
        let mut s = raw();
        s.bytes.truncate(159);
        assert!(s.active().is_err());
    }
    #[test]
    fn primitive_count_extent_is_checked() {
        let mut s = raw();
        s.primitive_counts.pop();
        assert!(s.active().is_err());
    }
    #[test]
    fn raw_positive_count_has_safe_bound() {
        let mut s = raw();
        s.packed_count = 257;
        assert!(s.active().is_err());
    }
    #[test]
    fn both_zero_gates_preserve_section_order() {
        let mut s = raw();
        s.primitive_counts[0] = 0;
        assert_eq!(s.active().unwrap()[0].index, 1);
        s.bytes[96] = 0;
        assert!(s.active().unwrap().is_empty());
    }
    #[test]
    fn material_falls_back_to_resolver_result() {
        let v = materials().select(&section()).unwrap();
        assert_eq!(v.material, 20);
        assert_eq!(
            v.calls.iter().map(|c| c.vtable_offset).collect::<Vec<_>>(),
            vec![0xa0, 0xd8, 0x50]
        );
        assert_eq!(v.calls[1].arguments, vec![12, 3]);
    }
    #[test]
    fn override_count_equal_to_index_uses_resolved_material() {
        let mut m = materials();
        m.override_packed_count = 2;
        assert_eq!(m.select(&section()).unwrap().material, 20);
    }
    #[test]
    fn override_accessor_receives_unsigned_material_index() {
        let mut m = materials();
        m.override_packed_count = 0xa0000003;
        let v = m.select(&section()).unwrap();
        assert_eq!(v.material, 32);
        assert_eq!(v.calls[2].arguments, vec![2]);
        assert_eq!(v.calls[2].receiver, 4);
    }
    #[test]
    fn negative_override_count_uses_resolved_material() {
        let mut m = materials();
        m.override_packed_count = 0x1fffffff;
        assert_eq!(m.select(&section()).unwrap().material, 20);
    }
    #[test]
    fn wire_takes_priority_over_actor_override() {
        let mut m = materials();
        m.wire = true;
        m.override_packed_count = 3;
        let v = m.select(&section()).unwrap();
        assert_eq!(v.material, 40);
        assert_eq!(v.calls.len(), 3);
    }
    #[test]
    fn any_debug_word_selects_wire_shader() {
        let mut m = materials();
        m.debug = 0x80000000;
        assert_eq!(m.select(&section()).unwrap().material, 40);
    }
    #[test]
    fn missing_wire_shader_and_resolver_answers_fail_safely() {
        let mut m = materials();
        m.wire = true;
        m.wire_shader = 0;
        assert!(m.select(&section()).is_err());
        m.wire_shader = 40;
        m.resolved.clear();
        assert!(m.select(&section()).is_err());
    }
    #[test]
    fn null_material_answer_is_still_forwarded_to_set_material() {
        let mut m = materials();
        m.resolved[0] = 0;
        let v = m.select(&section()).unwrap();
        assert_eq!(v.calls.last().unwrap().arguments, vec![0, 0, 0, 0]);
        assert_eq!(v.calls.last().unwrap().receiver, 1);
    }
    #[test]
    fn identical_pass_aliases_across_materials_are_valid() {
        assert!(validate_bank(&[
            MaterialPasses {
                material: 1,
                passes: vec![pass()]
            },
            MaterialPasses {
                material: 2,
                passes: vec![pass()]
            }
        ])
        .is_ok());
    }
    #[test]
    fn conflicting_pass_aliases_across_materials_are_rejected() {
        let mut p = pass();
        p.fog_color = 1;
        assert!(validate_bank(&[
            MaterialPasses {
                material: 1,
                passes: vec![pass()]
            },
            MaterialPasses {
                material: 2,
                passes: vec![p]
            }
        ])
        .is_err());
    }
    #[test]
    fn material_tokens_are_unique_and_bounded() {
        let m = MaterialPasses {
            material: 1,
            passes: vec![],
        };
        assert!(validate_bank(&[m.clone(), m]).is_err());
        let bank = (0..257)
            .map(|material| MaterialPasses {
                material,
                passes: vec![],
            })
            .collect::<Vec<_>>();
        assert!(validate_bank(&bank).is_err());
    }
}
