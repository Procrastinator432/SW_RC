//! Joined RenderPreProcess / Project / camera LOD / instance GetFrame / draw path.
//! MeshToWorld, stored tangent, asset lookup, physical allocators and GPU remain boundaries.
use crate::{
    d3d_dynamic_draw::Runtime, d3d_mesh_draw::BoundaryCall, scene_projection, vertmesh_indices,
    vertmesh_instance, vertmesh_lod, vertmesh_preprocess,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub preprocessing: vertmesh_preprocess::Input,
    pub actor_draw_scale: [u32; 3],
    pub world_to_screen: crate::shader_constants::Matrix,
    pub actor_position: [u32; 3],
    pub camera: vertmesh_lod::Camera,
    pub gates: vertmesh_lod::Gates,
    /// Addresses describe original +dc arguments; count_after is derived, not supplied.
    pub vertex: vertmesh_lod::VertexAnswer,
    pub instance: vertmesh_instance::Input,
    pub geometry: vertmesh_indices::Request,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    /// Snapshot after preprocessing, before GetFrame and index-header mutations.
    pub preprocessing: vertmesh_preprocess::Prepared,
    pub actor_abs_max: u32,
    pub projection: [u32; 4],
    pub lod: vertmesh_lod::Lod,
    pub gate: vertmesh_lod::Gate,
    pub vertex_call: Option<BoundaryCall>,
    pub count_before: Option<u32>,
    pub count_after: Option<u32>,
    pub instance: Option<vertmesh_instance::Output>,
    pub draw: Option<vertmesh_indices::Plan>,
}
impl Runtime {
    /// Commit preprocessing, derived camera, channels/cache, stream, headers and draw together.
    pub fn submit_vertmesh_render(&mut self, request: &mut Request) -> Result<Plan, String> {
        let preprocessing = request.preprocessing.prepare()?;
        let actor_abs_max = vertmesh_preprocess::abs_max(request.actor_draw_scale)?;
        let projection =
            scene_projection::project(request.world_to_screen, request.actor_position)?;
        let mut q = request.clone();
        q.preprocessing.state = preprocessing.state.clone();
        q.camera.actor_abs_max = actor_abs_max;
        q.camera.projected_w = projection[3];
        q.gates.packed_sections = preprocessing.state.packed_sections;
        let geometry = &mut q.geometry.geometry;
        geometry.packed_count = preprocessing.state.packed_sections;
        geometry.section_bytes = preprocessing.state.section_bytes.clone();
        if preprocessing.processed {
            let nf = (q.preprocessing.packed_faces & 0x1fffffff) as usize;
            let nw = (q.preprocessing.packed_wedges & 0x1fffffff) as usize;
            geometry.faces = q.preprocessing.faces[..nf].to_vec();
            geometry.vertex_thresholds = q.preprocessing.wedges[..nw]
                .iter()
                .map(|w| w[0] as u16)
                .collect();
        }
        let lod = q.camera.select()?;
        let gate = q.gates.evaluate();
        let mut plan = Plan {
            preprocessing,
            actor_abs_max,
            projection,
            lod,
            gate,
            vertex_call: None,
            count_before: None,
            count_after: None,
            instance: None,
            draw: None,
        };
        if plan.gate != vertmesh_lod::Gate::Enabled {
            // Native preprocessing/camera work happens before both early draw gates.
            *request = q;
            return Ok(plan);
        }
        let stream = &q.preprocessing.state;
        let nw = (stream.packed_stream_count.wrapping_shl(3) as i32) >> 3;
        if !(0..=65536).contains(&q.camera.vertex_count)
            || q.instance.animation.clock.vertex_count != q.camera.vertex_count as u32
            || q.instance.mesh != q.geometry.mesh.materials.mesh_address
            || !(0..=65536).contains(&nw)
            || stream.stream.len() != nw as usize * 32
            || q.geometry.geometry.vertex_thresholds.len() != nw as usize
        {
            return Err("Render mesh/stream snapshot mismatch".into());
        }
        if q.vertex.instance == 0
            || q.vertex.count_pointer == 0
            || q.vertex.count_pointer.checked_add(3).is_none()
            || q.vertex.source == 0
            || q.vertex
                .source
                .checked_add(stream.stream.len() as u32)
                .is_none()
        {
            return Err("Invalid integrated GetFrame addresses".into());
        }
        let count_before = plan.lod.selected as u32;
        plan.vertex_call = Some(BoundaryCall {
            receiver: q.vertex.instance,
            vtable_offset: 0xdc,
            arguments: vec![
                q.geometry.mesh.materials.actor_address,
                q.vertex.scene,
                q.vertex.source,
                32,
                q.vertex.count_pointer,
                0,
            ],
        });
        q.geometry.geometry.lod = plan.lod.selected;
        let a = &mut q.instance.animation;
        a.requested_count = plan.lod.selected;
        a.mode = 0;
        a.stride = 32;
        a.wedges = q.geometry.geometry.vertex_thresholds.clone();
        a.bytes = stream.stream.clone();
        let mut nested = vertmesh_instance::Request {
            instance: q.instance,
            geometry: q.geometry,
        };
        // The nested operation is already atomic and no fallible work follows its success.
        let result = self.submit_vertmesh_instance(&mut nested)?;
        let count_after = result.instance.animation.frame.count as u32;
        q.vertex.count_after = count_after;
        q.preprocessing.state.stream = result.instance.animation.frame.bytes.clone();
        q.preprocessing.state.section_bytes = nested.geometry.mesh.sections.bytes.clone();
        q.instance = nested.instance;
        q.geometry = nested.geometry;
        plan.count_before = Some(count_before);
        plan.count_after = Some(count_after);
        plan.instance = Some(result.instance);
        plan.draw = Some(result.draw);
        *request = q;
        Ok(plan)
    }
}
