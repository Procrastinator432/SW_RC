//! D3DDrv renderer virtual +48 (1001fc70), raw copies and deferred marking.
//! Dispatch-selected inverse math is an explicit supplied write/no-write answer.
use crate::{d3d_bindings::Deferred, d3d_dynamic_draw::Runtime, mesh_transform};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Scene {
    /// Render state +2c/+6c/+ac, object/view/projection matrices.
    pub matrices: [[u32; 16]; 3],
    /// Cache+5e4. A singular inverse may leave these previous words untouched.
    pub inverse: [u32; 16],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InverseAnswer {
    Unresolved,
    Unchanged,
    Written { words: [u32; 16] },
}
#[derive(Clone, Debug, Serialize)]
pub struct InverseCall {
    pub matrix: [u32; 16],
    pub determinant_pointer: u32,
}
#[derive(Clone, Debug, Serialize)]
pub struct SetPlan {
    pub slot: u32,
    pub copied: bool,
    pub inverse_call: Option<InverseCall>,
}
impl Scene {
    /// Unknown slots return without reading the matrix or inverse answer.
    /// Known slots copy raw words, with no NaN or identity normalization.
    pub fn set(
        &mut self,
        deferred: &mut Deferred,
        slot: u32,
        matrix: Option<[u32; 16]>,
        inverse: &InverseAnswer,
    ) -> Result<SetPlan, String> {
        let mut plan = SetPlan {
            slot,
            copied: false,
            inverse_call: None,
        };
        if slot > 2 {
            return Ok(plan);
        }
        let matrix = matrix.ok_or("Missing renderer transform matrix")?;
        if slot == 0 && matches!(inverse, InverseAnswer::Unresolved) {
            return Err("Unresolved renderer inverse dispatch answer".into());
        }
        let (index, dirty) = match slot {
            0 => (10, 0x20),
            1 => (8, 0x40),
            _ => (9, 0x40),
        };
        deferred.transforms.words[index] = matrix;
        deferred.transforms.mask |= 1 << index;
        deferred.states.dirty |= dirty;
        self.matrices[slot as usize] = matrix;
        if slot == 0 {
            plan.inverse_call = Some(InverseCall {
                matrix,
                determinant_pointer: 0,
            });
            if let InverseAnswer::Written { words } = inverse {
                self.inverse = *words;
            }
        }
        plan.copied = true;
        Ok(plan)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RendererRuntime {
    pub scene: Scene,
    pub runtime: Runtime,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub mesh: mesh_transform::Request,
    pub inverse: InverseAnswer,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub setter: SetPlan,
    pub mesh: mesh_transform::Plan,
}
impl RendererRuntime {
    /// Persist original +48 writes through the complete CPU render path. No GPU
    /// calls execute here. Scene, deferred state, mesh request and runtime are atomic.
    pub fn submit(&mut self, request: &mut Request) -> Result<Plan, String> {
        let mut next = self.clone();
        let mut q = request.clone();
        let matrix = q.mesh.transform.build()?.matrix;
        let words = std::array::from_fn(|i| matrix[i / 4][i % 4]);
        let setter = next.scene.set(
            &mut next.runtime.buffers.deferred,
            0,
            Some(words),
            &q.inverse,
        )?;
        let mesh = next.runtime.submit_transformed_vertmesh(&mut q.mesh)?;
        *self = next;
        *request = q;
        Ok(Plan { setter, mesh })
    }
}
