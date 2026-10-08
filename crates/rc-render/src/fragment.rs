//! CPU diagnostic fragment pass. UVs/color are host fixtures, not vertex shader outputs.
use crate::Texture;
use rc_package::pixel_shader::{Inputs, Program};

pub fn sample(texture: &Texture, uv: [f32; 2]) -> Result<[f32; 4], String> {
    if texture.width == 0
        || texture.height == 0
        || texture.width.checked_mul(texture.height) != Some(texture.pixels.len())
        || uv.iter().any(|v| !v.is_finite())
    {
        return Err("Invalid diagnostic texture/UV".into());
    }
    let x = ((uv[0].rem_euclid(1.) * texture.width as f32) as usize).min(texture.width - 1);
    let y = ((uv[1].rem_euclid(1.) * texture.height as f32) as usize).min(texture.height - 1);
    let pixel = texture.pixels[y * texture.width + x];
    Ok([16, 8, 0, 24].map(|shift| ((pixel >> shift) & 255) as f32 / 255.))
}

/// Explicit source-alpha/additive diagnostic composite, rounded to 8-bit opaque RGB.
/// Original alpha test, fog, depth and vertex shader are outside this pass.
pub fn composite(color: [f32; 4], background: u32) -> Result<u32, String> {
    if color.iter().any(|v| !v.is_finite()) {
        return Err("Nonfinite fragment".into());
    }
    let mut pixel = 0xff000000;
    for (i, shift) in [16, 8, 0].into_iter().enumerate() {
        let destination = ((background >> shift) & 255) as f32 / 255.;
        let channel = (color[i] * color[3] + destination).clamp(0., 1.);
        pixel |= ((channel * 255.).round() as u32) << shift;
    }
    Ok(pixel)
}

pub fn shade(
    program: &Program,
    textures: [&Texture; 3],
    uv: [[f32; 2]; 3],
    constants: [[f32; 4]; 8],
    color: [f32; 4],
    background: u32,
) -> Result<u32, String> {
    let mut samples = [[0.; 4]; 4];
    for i in 0..3 {
        samples[i] = sample(textures[i], uv[i])?;
    }
    let result = program.evaluate(&Inputs {
        textures: samples,
        constants,
        colors: [color, [0.; 4]],
    })?;
    composite(result.color, background)
}

/// Point probe linkage, without triangle interpolation/rasterization.
/// The original shader writes only oT0.x; diagnostic sampler fixes T0.y to 0.
pub fn shade_vertex_output(
    vertex: &rc_package::vertex_shader::Evaluation,
    pixel: &Program,
    textures: [&Texture; 3],
    constants: [[f32; 4]; 8],
    background: u32,
) -> Result<u32, String> {
    for (register, axes) in [(1, 15u8), (3, 1), (4, 3), (5, 3)] {
        for axis in 0..4 {
            if axes & (1 << axis) != 0 && !vertex.defined[register][axis] {
                return Err("Missing vertex varying for fragment point probe".into());
            }
        }
    }
    shade(
        pixel,
        textures,
        [
            [vertex.output[3][0], 0.],
            [vertex.output[4][0], vertex.output[4][1]],
            [vertex.output[5][0], vertex.output[5][1]],
        ],
        constants,
        vertex.output[1],
        background,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vertex_linkage_uses_outputs_and_requires_defined_components() {
        let v = rc_package::vertex_shader::Program::parse(
            "vs.1.0\nmov oPos,v0\nmov oD0,c0\nmov oT0.x,c0.x\nmov oT1.xy,v2\nmov oT2.xy,v2",
        )
        .unwrap();
        let mut constants = vec![[1.; 4]; 96];
        constants[0] = [0.5; 4];
        let mut output = v
            .evaluate(&rc_package::vertex_shader::Inputs {
                vertices: [[0.; 4]; 16],
                constants,
            })
            .unwrap();
        let pixel = Program::parse("ps.1.1\ntex t0\nmul r0,t0,v0").unwrap();
        let t = Texture {
            width: 1,
            height: 1,
            pixels: vec![0xffffffff],
        };
        assert_eq!(
            shade_vertex_output(&output, &pixel, [&t; 3], [[0.; 4]; 8], 0xff000000).unwrap(),
            0xff404040
        );
        output.defined[5][1] = false;
        assert!(shade_vertex_output(&output, &pixel, [&t; 3], [[0.; 4]; 8], 0).is_err());
    }
    #[test]
    fn nearest_repeat_and_channel_order() {
        let t = Texture {
            width: 2,
            height: 1,
            pixels: vec![0x80402010, 0xffffffff],
        };
        assert_eq!(
            sample(&t, [-1., 2.]).unwrap(),
            [64. / 255., 32. / 255., 16. / 255., 128. / 255.]
        );
        assert_eq!(sample(&t, [-0.25, 0.]).unwrap(), [1.; 4]);
        assert!(sample(&t, [f32::NAN, 0.]).is_err());
    }
    #[test]
    fn additive_alpha_and_clamping() {
        assert_eq!(
            composite([1., 0., 0., 0.5], 0xff0000ff).unwrap(),
            0xff8000ff
        );
        assert_eq!(composite([2.; 4], 0xff102030).unwrap(), 0xffffffff);
        assert_eq!(composite([1., 1., 1., 0.], 0xff102030).unwrap(), 0xff102030);
    }
}
