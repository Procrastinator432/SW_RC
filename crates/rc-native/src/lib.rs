//! C ABI shared by the Android diagnostic shell and the future native engine.
use std::ffi::c_char;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};
static SCENES: OnceLock<Mutex<HashMap<u64, rc_render::Scene>>> = OnceLock::new();
static NEXT_SCENE: AtomicU64 = AtomicU64::new(1);
fn scenes() -> &'static Mutex<HashMap<u64, rc_render::Scene>> {
    SCENES.get_or_init(|| Mutex::new(HashMap::new()))
}
/// Query normalized camera position and a forward zero-extent line through Model1.
/// Returns bit 0 = outside, bit 1 = clear line; -2 = snapshot has no BSP, -1 = error.
#[no_mangle]
pub extern "C" fn rc_scene_bsp_query(id: u64, x: f32, y: f32, z: f32, yaw: f32, pitch: f32) -> i32 {
    if !yaw.is_finite()
        || !pitch.is_finite()
        || [x, y, z].iter().any(|v| !v.is_finite() || v.abs() > 100.)
    {
        return -1;
    }
    let Ok(registry) = scenes().lock() else {
        return -1;
    };
    let Some(scene) = registry.get(&id) else {
        return -1;
    };
    let Some(solid) = &scene.solid else {
        return -2;
    };
    let start = std::array::from_fn(|i| scene.center[i] + [x, y, z][i] * scene.radius);
    let pitch = pitch.clamp(-1.5, 1.5);
    let direction = [
        -yaw.cos() * pitch.cos(),
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
    ];
    let end = std::array::from_fn(|i| start[i] + direction[i] * 1000.);
    match (solid.point_outside(start), solid.line_clear(start, end)) {
        (Ok(outside), Ok(clear)) => i32::from(outside) | (i32::from(clear) << 1),
        _ => -1,
    }
}
/// Load BSP world. Zero means error.
/// # Safety
/// data points to len readable bytes; error to capacity writable bytes; buffers cannot overlap.
#[no_mangle]
pub unsafe extern "C" fn rc_scene_create(
    data: *const u8,
    len: usize,
    error: *mut c_char,
    capacity: usize,
) -> u64 {
    if data.is_null() || len > 64 * 1024 * 1024 {
        return 0;
    }
    match rc_render::Scene::from_map(std::slice::from_raw_parts(data, len)) {
        Ok(scene) => {
            let id = NEXT_SCENE.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut registry) = scenes().lock() {
                registry.insert(id, scene);
                id
            } else {
                0
            }
        }
        Err(e) => {
            if !error.is_null() && capacity > 0 {
                let n = e.len().min(capacity - 1);
                std::ptr::copy_nonoverlapping(e.as_ptr(), error.cast::<u8>(), n);
                *error.add(n) = 0;
            }
            0
        }
    }
}
/// Render ARGB8888 pixels; returns count or -1.
/// # Safety
/// output points to capacity writable u32 pixels.
#[no_mangle]
pub unsafe extern "C" fn rc_scene_draw(
    id: u64,
    width: usize,
    height: usize,
    yaw: f32,
    pitch: f32,
    output: *mut u32,
    capacity: usize,
) -> isize {
    rc_scene_draw_view(
        id, width, height, yaw, pitch, 1.0, 0.0, 0.0, 0.0, 0, output, capacity,
    )
}
/// Render with orbit zoom or a free camera positioned relative to scene bounds.
/// # Safety
/// output points to capacity writable u32 pixels.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn rc_scene_draw_view(
    id: u64,
    width: usize,
    height: usize,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    x: f32,
    y: f32,
    z: f32,
    free: u32,
    output: *mut u32,
    capacity: usize,
) -> isize {
    if free > 1 {
        return -1;
    }
    if output.is_null() || width.checked_mul(height).is_none_or(|n| n > capacity) {
        return -1;
    }
    let Ok(registry) = scenes().lock() else {
        return -1;
    };
    let Some(scene) = registry.get(&id) else {
        return -1;
    };
    match scene.render_view(
        width,
        height,
        rc_render::View {
            yaw,
            pitch,
            zoom,
            position: (free == 1).then_some([x, y, z]),
        },
    ) {
        Ok(pixels) => {
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), output, pixels.len());
            pixels.len() as isize
        }
        Err(_) => -1,
    }
}
/// Releasing an invalid or previously released handle is harmless.
#[no_mangle]
pub extern "C" fn rc_scene_release(id: u64) {
    if let Ok(mut registry) = scenes().lock() {
        registry.remove(&id);
    }
}
/// Camera anchor: normalized XYZ, yaw, pitch, total anchor count; returns 6 or -1.
/// # Safety
/// output points to capacity writable f32 values.
#[no_mangle]
pub unsafe extern "C" fn rc_scene_start(
    id: u64,
    index: usize,
    output: *mut f32,
    capacity: usize,
) -> isize {
    if output.is_null() || capacity < 6 {
        return -1;
    }
    let Ok(registry) = scenes().lock() else {
        return -1;
    };
    let Some(scene) = registry.get(&id) else {
        return -1;
    };
    let Ok(view) = scene.start_view(index) else {
        return -1;
    };
    let Some(position) = view.position else {
        return -1;
    };
    let values = [
        position[0],
        position[1],
        position[2],
        view.yaw,
        view.pitch,
        scene.starts.len() as f32,
    ];
    std::ptr::copy_nonoverlapping(values.as_ptr(), output, 6);
    6
}
/// Returns a package diagnostic as UTF-8, or -1 for invalid arguments.
/// # Safety
/// Input must point to len readable bytes; output to capacity writable bytes.
/// Buffers must not overlap. The return value includes the terminating NUL.
#[no_mangle]
pub unsafe extern "C" fn rc_package_summary(
    data: *const u8,
    len: usize,
    out: *mut c_char,
    capacity: usize,
) -> isize {
    if data.is_null() || out.is_null() || len > isize::MAX as usize {
        return -1;
    }
    let bytes = std::slice::from_raw_parts(data, len);
    let message = if bytes.starts_with(b"RCSC") {
        match rc_render::Scene::from_snapshot(bytes) {
            Ok(scene) => format!(
                "Original scene: {} triangles, {} textures, {} starts",
                scene.triangles.len(),
                scene.textures.len(),
                scene.starts.len()
            ),
            Err(e) => format!("Scene error: {e}"),
        }
    } else {
        match rc_package::read_package(bytes) {
            Ok(p) => format!(
                "SWRC {}/{}: {} names, {} imports, {} exports",
                p.summary.version,
                p.summary.licensee_version,
                p.names.len(),
                p.imports.len(),
                p.exports.len()
            ),
            Err(e) => format!("Package error: {e}"),
        }
    };
    let needed = message.len() + 1;
    if capacity >= needed {
        std::ptr::copy_nonoverlapping(message.as_ptr(), out.cast::<u8>(), message.len());
        *out.add(message.len()) = 0;
    }
    needed as isize
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bsp_query_flags_direction_missing_data_and_invalid_arguments() {
        let mut scene = rc_render::Scene::new(vec![rc_package::geometry::Triangle {
            points: [[0., -1., -1.], [0., 1., -1.], [0., 0., 1.]],
            surface: 0,
        }])
        .unwrap();
        let id = NEXT_SCENE.fetch_add(1, Ordering::Relaxed);
        scenes().lock().unwrap().insert(id, scene);
        assert_eq!(rc_scene_bsp_query(id, 1., 0., 0., 0., 0.), -2);
        scene = scenes().lock().unwrap().remove(&id).unwrap();
        scene.solid = Some(rc_package::collision::SolidBsp {
            nodes: vec![rc_package::collision::SolidNode {
                plane: [1., 0., 0., 0.],
                back: -1,
                front: -1,
                flags: 0,
            }],
            root_outside: false,
            linked: false,
            zones: 0,
            bounds: 0,
            hull_indices: 0,
            hull_words: vec![],
            leaves: 0,
            lights: 0,
            decoded_bytes: 0,
            remaining_bytes: 0,
        });
        scenes().lock().unwrap().insert(id, scene);
        assert_eq!(rc_scene_bsp_query(id, 1., 0., 0., 0., 0.), 1); // free start, forward toward solid
        assert_eq!(
            rc_scene_bsp_query(id, 1., 0., 0., std::f32::consts::PI, 0.),
            3
        ); // away from solid
        assert_eq!(rc_scene_bsp_query(id, -1., 0., 0., 0., 0.), 0);
        assert_eq!(rc_scene_bsp_query(id, f32::NAN, 0., 0., 0., 0.), -1);
        rc_scene_release(id);
        assert_eq!(rc_scene_bsp_query(id, 1., 0., 0., 0., 0.), -1);
    }
    #[test]
    fn anchor_ffi_returns_indexed_transforms_without_writing_on_failure() {
        let mut scene = rc_render::Scene::new(vec![rc_package::geometry::Triangle {
            points: [[0., -1., -1.], [0., 1., -1.], [0., 0., 1.]],
            surface: 0,
        }])
        .unwrap();
        for x in [0., 1.] {
            scene.starts.push(rc_package::level::PlayerStart {
                actor: format!("Start{x}"),
                location: [x, 0., 0.],
                rotation: [0; 3],
            });
        }
        let id = NEXT_SCENE.fetch_add(1, Ordering::Relaxed);
        scenes().lock().unwrap().insert(id, scene);
        let mut output = [77.; 6];
        assert_eq!(unsafe { rc_scene_start(id, 0, output.as_mut_ptr(), 5) }, -1);
        assert_eq!(output, [77.; 6]);
        assert_eq!(unsafe { rc_scene_start(id, 2, output.as_mut_ptr(), 6) }, -1);
        assert_eq!(output, [77.; 6]);
        assert_eq!(unsafe { rc_scene_start(id, 0, output.as_mut_ptr(), 6) }, 6);
        assert_eq!(output[5], 2.);
        let first = output;
        assert_eq!(unsafe { rc_scene_start(id, 1, output.as_mut_ptr(), 6) }, 6);
        assert!(output[0] > first[0]);
        rc_scene_release(id);
        assert_eq!(unsafe { rc_scene_start(id, 0, output.as_mut_ptr(), 6) }, -1);
    }
    #[test]
    fn scene_handles_reject_use_after_release_and_short_output() {
        let scene = rc_render::Scene::new(vec![rc_package::geometry::Triangle {
            points: [[0.0, -1.0, -1.0], [0.0, 1.0, -1.0], [0.0, 0.0, 1.0]],
            surface: 0,
        }])
        .unwrap();
        let id = NEXT_SCENE.fetch_add(1, Ordering::Relaxed);
        scenes().lock().unwrap().insert(id, scene);
        let mut pixels = vec![0u32; 64 * 64];
        assert_eq!(
            unsafe { rc_scene_draw(id, 64, 64, 0.0, 0.0, pixels.as_mut_ptr(), 1) },
            -1
        );
        assert_eq!(
            unsafe { rc_scene_draw(id, 64, 64, 0.0, 0.0, pixels.as_mut_ptr(), pixels.len()) },
            4096
        );
        rc_scene_release(id);
        rc_scene_release(id);
        assert_eq!(
            unsafe { rc_scene_draw(id, 64, 64, 0.0, 0.0, pixels.as_mut_ptr(), pixels.len()) },
            -1
        );
    }
    #[test]
    fn ffi_buffer_contract() {
        let input = [0u8];
        let mut tiny = [77i8; 2];
        let needed = unsafe {
            rc_package_summary(input.as_ptr(), input.len(), tiny.as_mut_ptr(), tiny.len())
        };
        assert!(needed > 2);
        assert_eq!(tiny, [77, 77]);
        let mut output = vec![0i8; needed as usize];
        assert_eq!(
            unsafe {
                rc_package_summary(
                    input.as_ptr(),
                    input.len(),
                    output.as_mut_ptr(),
                    output.len(),
                )
            },
            needed
        );
        let message = unsafe { std::ffi::CStr::from_ptr(output.as_ptr()) }
            .to_str()
            .unwrap();
        assert!(message.starts_with("Package error:"));
        assert_eq!(
            unsafe { rc_package_summary(std::ptr::null(), 0, output.as_mut_ptr(), output.len()) },
            -1
        );
    }
}
