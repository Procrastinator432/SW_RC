//! Camera LOD arithmetic/gates and GetFrame handoff, Engine!10560194..1056032a.
//! Projection W, stored FPTAN result and GetAbsMax answer remain explicit inputs.
use crate::{d3d_dynamic_draw::Runtime, d3d_mesh_draw::BoundaryCall, vertmesh_indices};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Camera {
    pub vertex_count: i32,
    pub viewport_width: i32,
    pub minimum: i32,
    pub mesh_lod_scale: u32,
    pub mesh_distance_scale: u32,
    pub distance_bias: u32,
    pub projected_w: u32,
    pub tan_fov: u32,
    pub actor_lod_bias: u32,
    pub actor_draw_scale: u32,
    pub actor_abs_max: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Lod {
    pub vertex_factor: u32,
    pub viewport_factor: u32,
    pub distance: u32,
    pub denominator: u32,
    pub absolute_scale: u32,
    pub candidate: u32,
    pub selected_float: u32,
    pub rounded: i32,
    pub selected: i32,
}
/// Default MXCSR round-to-nearest-even; invalid conversion yields integer indefinite.
pub fn convert(value: f32) -> i32 {
    let rounded = value.round_ties_even();
    if !rounded.is_finite() || !(-2147483648.0..2147483648.0).contains(&rounded) {
        i32::MIN
    } else {
        rounded as i32
    }
}
impl Camera {
    pub fn select(&self) -> Result<Lod, String> {
        let floats = [
            self.mesh_lod_scale,
            self.mesh_distance_scale,
            self.distance_bias,
            self.projected_w,
            self.tan_fov,
            self.actor_lod_bias,
            self.actor_draw_scale,
            self.actor_abs_max,
        ];
        if floats.iter().any(|&v| !f32::from_bits(v).is_finite()) {
            return Err("Nonfinite camera LOD input".into());
        }
        let vertex_factor = self.vertex_count as f32 * f32::from_bits(0x3b449ba6) + 0.25;
        let viewport_factor =
            self.viewport_width as f32 * f32::from_bits(0x3a8f5c29) + f32::from_bits(0x3e99999a);
        let delta = f32::from_bits(self.projected_w) - f32::from_bits(self.distance_bias);
        let distance = if 1.0 < delta { delta } else { 1.0 };
        let denominator = ((f32::from_bits(self.mesh_distance_scale) * distance) * vertex_factor)
            * f32::from_bits(self.tan_fov);
        if denominator == 0.0 || !denominator.is_finite() {
            return Err("Zero or nonfinite camera LOD denominator".into());
        }
        let scale = f32::from_bits(self.actor_draw_scale);
        // COMISS/JC preserves negative zero on its nonnegative branch.
        let absolute_scale = if scale < 0.0 { 0.0 - scale } else { scale };
        let factor = (((((f32::from_bits(self.actor_lod_bias) / denominator)
            * f32::from_bits(self.mesh_lod_scale))
            * f32::from_bits(self.actor_abs_max))
            * absolute_scale)
            * viewport_factor)
            * 530.0;
        let candidate = self.vertex_count as f32 * factor;
        if !candidate.is_finite() {
            return Err("Nonfinite camera LOD candidate".into());
        }
        let minimum = self.minimum as f32;
        let selected_float = if minimum < candidate {
            candidate
        } else {
            minimum
        };
        let rounded = convert(selected_float);
        // Original signed upper clamp has no second lower clamp.
        let selected = if rounded > self.vertex_count {
            self.vertex_count
        } else {
            rounded
        };
        Ok(Lod {
            vertex_factor: vertex_factor.to_bits(),
            viewport_factor: viewport_factor.to_bits(),
            distance: distance.to_bits(),
            denominator: denominator.to_bits(),
            absolute_scale: absolute_scale.to_bits(),
            candidate: candidate.to_bits(),
            selected_float: selected_float.to_bits(),
            rounded,
            selected,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gates {
    /// Mesh count after optional RenderPreProcess; that helper is external.
    pub packed_sections: u32,
    pub actor_state: u32,
    pub owner_state: Option<u32>,
    pub fallback_state: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Gate {
    Enabled,
    NoSections,
    NoActorState,
}
impl Gates {
    pub fn evaluate(&self) -> Gate {
        if self.packed_sections & 0x1fffffff == 0 {
            Gate::NoSections
        } else if self.actor_state != 0
            || self.owner_state.is_some_and(|v| v != 0)
            || self.fallback_state != 0
        {
            Gate::Enabled
        } else {
            Gate::NoActorState
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VertexAnswer {
    pub instance: u32,
    pub scene: u32,
    pub source: u32,
    pub count_pointer: u32,
    /// External GetFrame may write this pointer. Geometry keeps the pre-call LOD.
    pub count_after: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub camera: Camera,
    pub gates: Gates,
    pub vertex: VertexAnswer,
    pub geometry: vertmesh_indices::Request,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub lod: Lod,
    pub gate: Gate,
    pub vertex_call: Option<BoundaryCall>,
    pub count_before: Option<u32>,
    pub count_after: Option<u32>,
    pub draw: Option<vertmesh_indices::Plan>,
}
impl Runtime {
    pub fn submit_lod(&mut self, request: &mut Request) -> Result<Plan, String> {
        let lod = request.camera.select()?;
        let gate = request.gates.evaluate();
        if gate != Gate::Enabled {
            return Ok(Plan {
                lod,
                gate,
                vertex_call: None,
                count_before: None,
                count_after: None,
                draw: None,
            });
        }
        if request.vertex.instance == 0
            || request.vertex.count_pointer == 0
            || request.vertex.count_pointer.checked_add(3).is_none()
        {
            return Err("Invalid vertex callback receiver/count pointer".into());
        }
        if request.gates.packed_sections != request.geometry.geometry.packed_count {
            return Err("LOD gate and geometry section snapshot mismatch".into());
        }
        let vertex_call = BoundaryCall {
            receiver: request.vertex.instance,
            vtable_offset: 0xdc,
            arguments: vec![
                request.geometry.mesh.materials.actor_address,
                request.vertex.scene,
                request.vertex.source,
                0x20,
                request.vertex.count_pointer,
                0,
            ],
        };
        let count_before = lod.selected as u32;
        let mut q = request.clone();
        let mut next = self.clone();
        q.geometry.geometry.lod = lod.selected;
        let draw = next.submit_geometry(&mut q.geometry)?;
        let count_after = q.vertex.count_after;
        *self = next;
        *request = q;
        Ok(Plan {
            lod,
            gate,
            vertex_call: Some(vertex_call),
            count_before: Some(count_before),
            count_after: Some(count_after),
            draw: Some(draw),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn camera() -> Camera {
        Camera {
            vertex_count: 10,
            viewport_width: 640,
            minimum: 1,
            mesh_lod_scale: 1f32.to_bits(),
            mesh_distance_scale: 1f32.to_bits(),
            distance_bias: 0,
            projected_w: 2f32.to_bits(),
            tan_fov: 1f32.to_bits(),
            actor_lod_bias: 0.001f32.to_bits(),
            actor_draw_scale: 1f32.to_bits(),
            actor_abs_max: 1f32.to_bits(),
        }
    }
    fn gates() -> Gates {
        Gates {
            packed_sections: 1,
            actor_state: 0,
            owner_state: None,
            fallback_state: 0,
        }
    }
    #[test]
    fn viewport_and_vertex_constants_match_original_words() {
        let l = camera().select().unwrap();
        assert_eq!(l.vertex_factor, 0.28f32.to_bits());
        assert_eq!(l.viewport_factor, 1f32.to_bits());
    }
    #[test]
    fn projected_distance_is_clamped_to_one() {
        let mut c = camera();
        c.projected_w = 0;
        c.distance_bias = 2f32.to_bits();
        assert_eq!(c.select().unwrap().distance, 1f32.to_bits());
    }
    #[test]
    fn projected_distance_above_one_is_retained() {
        assert_eq!(camera().select().unwrap().distance, 2f32.to_bits());
    }
    #[test]
    fn negative_actor_scale_uses_absolute_magnitude() {
        let mut c = camera();
        let a = c.select().unwrap();
        c.actor_draw_scale = (-1f32).to_bits();
        assert_eq!(c.select().unwrap().candidate, a.candidate);
    }
    #[test]
    fn negative_zero_scale_keeps_original_bits() {
        let mut c = camera();
        c.actor_draw_scale = (-0f32).to_bits();
        assert_eq!(c.select().unwrap().absolute_scale, 0x80000000);
    }
    #[test]
    fn rounding_uses_nearest_even_for_both_signs() {
        assert_eq!(
            [convert(2.5), convert(3.5), convert(-2.5), convert(-3.5)],
            [2, 4, -2, -4]
        );
    }
    #[test]
    fn invalid_integer_conversion_is_indefinite() {
        assert_eq!(convert(2147483648.0), i32::MIN);
        assert_eq!(convert(f32::NAN), i32::MIN);
    }
    #[test]
    fn selected_lod_has_signed_upper_clamp() {
        let mut c = camera();
        c.actor_lod_bias = 1f32.to_bits();
        assert_eq!(c.select().unwrap().selected, 10);
    }
    #[test]
    fn minimum_is_applied_before_integer_conversion() {
        let mut c = camera();
        c.minimum = 7;
        c.actor_draw_scale = 0;
        assert_eq!(c.select().unwrap().selected, 7);
    }
    #[test]
    fn no_lower_clamp_is_added_after_conversion_overflow() {
        let mut c = camera();
        c.minimum = i32::MAX;
        assert_eq!(c.select().unwrap().selected, i32::MIN);
    }
    #[test]
    fn zero_denominator_and_nonfinite_inputs_are_safe_errors() {
        let mut c = camera();
        c.tan_fov = 0;
        assert!(c.select().is_err());
        c.tan_fov = f32::NAN.to_bits();
        assert!(c.select().is_err());
    }
    #[test]
    fn section_gate_uses_low_bits_without_signed_test() {
        let mut g = gates();
        g.actor_state = 1;
        g.packed_sections = 0xe0000000;
        assert_eq!(g.evaluate(), Gate::NoSections);
        g.packed_sections = 0x1fffffff;
        assert_eq!(g.evaluate(), Gate::Enabled);
    }
    #[test]
    fn owner_and_fallback_can_enable_actor_gate() {
        let mut g = gates();
        assert_eq!(g.evaluate(), Gate::NoActorState);
        g.owner_state = Some(1);
        assert_eq!(g.evaluate(), Gate::Enabled);
        g.owner_state = Some(0);
        g.fallback_state = 0x80000000;
        assert_eq!(g.evaluate(), Gate::Enabled);
    }
}
