//! Reusable CPU diagnostic linkage for DynamicHologram's output layout.
//! Sampler, half-pixel raster, blending and depth policies remain explicit fixtures.
use crate::{
    fragment,
    shader_raster::{Sample, State, Stats, Target, Vertex},
    Texture,
};
use rc_package::{pixel_shader, shader_constants::Bank, vertex_shader};
pub struct HologramPass {
    vertex: vertex_shader::Program,
    pixel: pixel_shader::Program,
}
pub struct Prepared {
    pub vertices: [Vertex; 3],
    constants: [[f32; 4]; 8],
}
impl HologramPass {
    pub fn new(vertex: &str, pixel: &str) -> Result<Self, String> {
        Ok(Self {
            vertex: vertex_shader::Program::parse(vertex)?,
            pixel: pixel_shader::Program::parse(pixel)?,
        })
    }
    /// Evaluate all vertices before touching the target; keep every bank word,
    /// including retained registers beyond the cached upload count.
    pub fn prepare(
        &self,
        inputs: [[[f32; 4]; 16]; 3],
        vertex: &Bank,
        pixel: &Bank,
    ) -> Result<Prepared, String> {
        if vertex.words.len() != 96
            || pixel.words.len() != 8
            || !(0..=96).contains(&vertex.count)
            || !(0..=8).contains(&pixel.count)
        {
            return Err("Hologram pass requires initialized 96/8-register banks".into());
        }
        let constants: Vec<_> = vertex.words.iter().map(|r| r.map(f32::from_bits)).collect();
        let mut output = Vec::with_capacity(3);
        for vertices in inputs {
            output.push(Vertex::from_output(&self.vertex.evaluate(
                &vertex_shader::Inputs {
                    vertices,
                    constants: constants.clone(),
                },
            )?)?);
        }
        Ok(Prepared {
            vertices: output.try_into().unwrap(),
            constants: std::array::from_fn(|i| pixel.words[i].map(f32::from_bits)),
        })
    }
    pub fn shade(
        &self,
        prepared: &Prepared,
        sample: Sample,
        textures: [&Texture; 3],
    ) -> Result<[f32; 4], String> {
        let v = sample.varying;
        let uv = [[v[0], 0.], [v[1], v[2]], [v[3], v[4]]];
        let mut t = [[0.; 4]; 4];
        for i in 0..3 {
            t[i] = fragment::sample(textures[i], uv[i])?;
        }
        Ok(self
            .pixel
            .evaluate(&pixel_shader::Inputs {
                textures: t,
                constants: prepared.constants,
                colors: [[v[5], v[6], v[7], v[8]], [0.; 4]],
            })?
            .color)
    }
    /// Fragment failures retain prior target writes, like the existing diagnostic raster.
    pub fn draw(
        &self,
        prepared: &Prepared,
        textures: [&Texture; 3],
        target: &mut Target,
        state: State,
    ) -> Result<Stats, String> {
        target.draw(prepared.vertices, state, &mut |sample| {
            self.shade(prepared, sample, textures)
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pass() -> HologramPass {
        HologramPass::new("vs.1.0\nmov oPos,v0\nmov oD0,c17\nmov oT0.x,c17.x\nmov oT1.xy,v2\nmov oT2.xy,v2\nmov oFog.x,c17.x","ps.1.1\ntex t0\nmul r0,t0,v0").unwrap()
    }
    fn banks() -> (Bank, Bank) {
        let mut v = Bank::new(vec![[0; 4]; 96]).unwrap();
        v.count = 1;
        v.words[17] = [0.5f32.to_bits(); 4];
        let mut p = Bank::new(vec![[0; 4]; 8]).unwrap();
        p.count = 1;
        (v, p)
    }
    #[test]
    fn retained_registers_beyond_count_feed_vertex_and_pixel_pass() {
        let (v, p) = banks();
        let pass = pass();
        let prepared = pass.prepare([[[0.; 4]; 16]; 3], &v, &p).unwrap();
        assert_eq!(&prepared.vertices[0].varying[5..9], &[0.5; 4]);
        let texture = Texture {
            width: 1,
            height: 1,
            pixels: vec![0xffffffff],
        };
        assert_eq!(
            pass.shade(
                &prepared,
                Sample {
                    varying: prepared.vertices[0].varying.map(|x| x as f32),
                    depth: 0.,
                    pixel: 0
                },
                [&texture; 3]
            )
            .unwrap(),
            [0.5; 4]
        );
    }
    #[test]
    fn uninitialized_banks_and_incomplete_vertex_outputs_are_rejected() {
        let (mut v, p) = banks();
        v.count = -2;
        assert!(pass().prepare([[[0.; 4]; 16]; 3], &v, &p).is_err());
        v.count = 1;
        let incomplete = HologramPass::new("vs.1.0\nmov oPos,v0", "ps.1.1\nmov r0,c0").unwrap();
        assert!(incomplete.prepare([[[0.; 4]; 16]; 3], &v, &p).is_err());
    }
}
