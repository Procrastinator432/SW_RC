//! Computed scalar53 inverse joined transactionally to the renderer CPU path.
use crate::{renderer_inverse, renderer_transform};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub renderer: renderer_transform::Request,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub inverse: renderer_inverse::Output,
    pub renderer: renderer_transform::Plan,
}
impl renderer_transform::RendererRuntime {
    /// Explicit scalar fallback; never treats a supplied legacy answer as math.
    /// Missing/failed inverse writes preserve the previous scene inverse image.
    pub fn submit_scalar53(&mut self, request: &mut Request) -> Result<Plan, String> {
        let mut q = request.clone();
        let matrix = q.renderer.mesh.transform.build()?.matrix;
        let words = std::array::from_fn(|i| matrix[i / 4][i % 4]);
        let inverse = renderer_inverse::scalar53(words, false)?;
        q.renderer.inverse = match inverse.matrix {
            Some(words) => renderer_transform::InverseAnswer::Written { words },
            None => renderer_transform::InverseAnswer::Unchanged,
        };
        let renderer = self.submit(&mut q.renderer)?;
        *request = q;
        Ok(Plan { inverse, renderer })
    }
}
