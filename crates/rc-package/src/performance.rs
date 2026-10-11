//! Opt-in, thread-local CPU timings. Nested categories are inclusive, not additive.
//! No timers run while disabled; counters never participate in simulation state.
use std::{cell::RefCell, time::Instant};

pub const CATEGORIES: usize = 10;
pub const NAMES: [&str; CATEGORIES] = [
    "simulation",
    "collision",
    "actor_geometry",
    "scene_draw",
    "raster",
    "frame_buffers",
    "hud",
    "native_pixel_copy",
    "jni_arrays",
    "bundle_load",
];
#[derive(Clone, Copy)]
pub enum Category {
    Simulation,
    Collision,
    ActorGeometry,
    SceneDraw,
    Raster,
    FrameBuffers,
    Hud,
    NativePixelCopy,
    JniArrays,
    BundleLoad,
}
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counters {
    pub ns: [u64; CATEGORIES],
    pub calls: [u64; CATEGORIES],
}
thread_local! {
    static ACTIVE: RefCell<Option<(Counters, bool)>> = const { RefCell::new(None) };
}
pub fn begin() -> Result<(), &'static str> {
    start(false)
}
/// Per-triangle timers are useful on the desktop and very expensive in some emulators.
pub fn begin_detailed() -> Result<(), &'static str> {
    start(true)
}
fn start(detailed: bool) -> Result<(), &'static str> {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        if active.is_some() {
            return Err("Performance capture already active");
        }
        *active = Some((Counters::default(), detailed));
        Ok(())
    })
}
pub fn finish() -> Result<Counters, &'static str> {
    ACTIVE.with(|active| {
        active
            .borrow_mut()
            .take()
            .map(|(counters, _)| counters)
            .ok_or("No performance capture")
    })
}
pub fn enabled() -> bool {
    ACTIVE.with(|active| active.borrow().is_some())
}
pub fn detailed() -> bool {
    ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .is_some_and(|(_, detailed)| *detailed)
    })
}
pub fn add(category: usize, ns: u64) {
    if category >= CATEGORIES {
        return;
    }
    ACTIVE.with(|active| {
        if let Some((counters, _)) = active.borrow_mut().as_mut() {
            counters.ns[category] = counters.ns[category].saturating_add(ns);
            counters.calls[category] = counters.calls[category].saturating_add(1);
        }
    });
}
pub struct Span {
    category: Category,
    start: Option<Instant>,
}
impl Span {
    pub fn new(category: Category) -> Self {
        Self {
            category,
            start: enabled().then(Instant::now),
        }
    }
    /// Used in hot triangle loops after checking capture once per frame.
    pub fn when(enabled: bool, category: Category) -> Self {
        Self {
            category,
            start: enabled.then(Instant::now),
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            add(
                self.category as usize,
                start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_is_opt_in_thread_local_and_nested_begin_preserves_active_capture() {
        assert!(!enabled());
        add(0, 100);
        assert!(finish().is_err());
        begin().unwrap();
        assert!(!detailed());
        add(1, 7);
        assert!(begin().is_err());
        std::thread::spawn(|| {
            assert!(!enabled());
            begin().unwrap();
            add(1, 99);
            assert_eq!(finish().unwrap().ns[1], 99);
        })
        .join()
        .unwrap();
        add(usize::MAX, 88);
        let result = finish().unwrap();
        assert_eq!(result.ns[1], 7);
        assert_eq!(result.calls[1], 1);
        assert!(!enabled());
        begin_detailed().unwrap();
        assert!(detailed());
        finish().unwrap();
        assert!(!detailed());
    }
}
