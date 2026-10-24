//! Native owner worker; a caller-thread guard only sends a nonblocking shutdown signal.
//! OpenAL creation, rendering and destruction all run on the dedicated Rust worker.
use rc_package::{
    audio_live::Live,
    audio_openal::OpenAl,
    audio_pcm::{HEADER, MAX_FRAMES},
};
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_char,
    panic::{catch_unwind, AssertUnwindSafe},
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{sync_channel, SyncSender},
        Arc, Mutex, OnceLock,
    },
    thread::{self, JoinHandle, ThreadId},
};
enum Engine {
    Scene(Box<rc_package::audio_scene::Scene<OpenAl>>),
    Wave(Box<Live<OpenAl>>),
    Mixed(Box<rc_package::audio_mixed_live::Mixed<OpenAl>>),
}
impl Engine {
    fn render(&mut self, frames: usize, epoch: u64, sequence: u64) -> Result<Vec<u8>, String> {
        match self {
            Self::Scene(v) => v.render(frames, epoch, sequence),
            Self::Wave(v) => v.render(frames, epoch, sequence),
            Self::Mixed(v) => v.render(frames, epoch, sequence),
        }
    }
    fn call(&mut self, op: u8, bytes: &[u8]) -> Result<Vec<u8>, String> {
        match self {
            Self::Scene(v) => {
                if op == 8 {
                    if bytes.len() != 8 {
                        return Err("Mission receipt length".into());
                    }
                    v.consume_through(u64::from_le_bytes(bytes.try_into().unwrap()))?;
                    return Ok(vec![]);
                }
                if op == 7 {
                    return Ok(v.status());
                }
                if op == 6 {
                    v.set_mix(bytes)?;
                    return Ok(vec![]);
                }
                if op == 4 {
                    return v.snapshot();
                }
                if op == 5 {
                    v.restore(bytes)?;
                    return Ok(vec![]);
                }
                if op != 3 || bytes.len() < 8 {
                    return Err("Scene operation".into());
                }
                let n = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
                if n > bytes.len() - 4 {
                    return Err("Scene update length".into());
                }
                v.update(&bytes[4..4 + n], &bytes[4 + n..])?;
                Ok(vec![])
            }
            Self::Mixed(v) => match op {
                0 => {
                    v.submit(bytes)?;
                    Ok(vec![])
                }
                1 => v.checkpoint(),
                2 => Ok([v.remaining().to_le_bytes(), v.next_control().to_le_bytes()].concat()),
                _ => Err("Unknown mixed operation".into()),
            },
            _ => Err("Mixed operation on a WAV session".into()),
        }
    }
}
enum Input {
    Scene(Vec<u8>),
    Wave(Vec<u8>),
    Mixed(Box<rc_package::audio_mixed_live::Bundle>),
}
enum Request {
    Call {
        op: u8,
        bytes: Vec<u8>,
        reply: SyncSender<Result<Vec<u8>, String>>,
    },
    Render {
        frames: usize,
        epoch: u64,
        sequence: u64,
        reply: SyncSender<Result<Vec<u8>, String>>,
    },
    Stop,
}
struct Worker {
    id: u64,
    caller: ThreadId,
    sender: SyncSender<Request>,
    join: Mutex<Option<JoinHandle<()>>>,
    closed: AtomicBool,
}
struct Guard(Arc<Worker>);
impl Drop for Guard {
    fn drop(&mut self) {
        // Windows TLS teardown may hold the loader lock. Never join or unload an AL DLL here.
        self.0.closed.store(true, Ordering::Release);
        let _ = self.0.sender.try_send(Request::Stop);
    }
}
thread_local! { static OWNER:RefCell<Option<Guard>>=const { RefCell::new(None) }; }
static NEXT: AtomicU64 = AtomicU64::new(1);
static WORKERS: OnceLock<Mutex<HashMap<u64, Arc<Worker>>>> = OnceLock::new();
fn registry() -> &'static Mutex<HashMap<u64, Arc<Worker>>> {
    WORKERS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn owned(id: u64) -> Result<Arc<Worker>, String> {
    let r = registry()
        .lock()
        .map_err(|_| "Native mixer registry poisoned")?;
    let w = r.get(&id).ok_or("No such native mixer")?;
    if w.caller != thread::current().id() || w.closed.load(Ordering::Acquire) {
        return Err("Foreign/stale native mixer ID".into());
    }
    Ok(w.clone())
}
#[cfg(test)]
fn open(path: &str, wave: &[u8], rate: u32, epoch: u64) -> Result<u64, String> {
    open_mode(path, wave, rate, epoch, 0)
}
fn open_mode(path: &str, wave: &[u8], rate: u32, epoch: u64, mode: u8) -> Result<u64, String> {
    OWNER.with(|cell| {
        let mut owner = cell.borrow_mut();
        if owner.is_some() {
            return Err("This caller already owns a native mixer".into());
        }
        if epoch == 0 || epoch > i64::MAX as u64 {
            return Err("Invalid native mixer input".into());
        }
        let input = if mode == 2 {
            rc_package::audio_scene::bank(wave)?;
            Input::Scene(wave.to_vec())
        } else if mode == 1 {
            if rate != 48000 {
                return Err("Mixed adapter requires 48000 Hz".into());
            }
            Input::Mixed(Box::new(rc_package::audio_mixed_live::Bundle::parse(wave)?))
        } else {
            if wave.len() > rc_package::audio_live::MAX_WAVE {
                return Err("WAV too large".into());
            }
            rc_package::audio_wave::Pcm::from_wave(wave)?;
            Input::Wave(wave.to_vec())
        };
        // Capacity and unique IDs are assigned without holding this lock during any native IO.
        let mut r = registry()
            .lock()
            .map_err(|_| "Native mixer registry poisoned")?;
        r.retain(|_, w| {
            !w.closed.load(Ordering::Acquire)
                || w.join
                    .lock()
                    .map_or(true, |j| j.as_ref().is_some_and(|h| !h.is_finished()))
        });
        if r.len() >= 16 {
            return Err("Native mixer session budget exhausted".into());
        }
        let id = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                (n < i64::MAX as u64).then_some(n + 1)
            })
            .map_err(|_| "Native mixer IDs exhausted")?;
        let (sender, requests) = sync_channel(1);
        let (ready, startup) = sync_channel(1);
        let path = path.to_owned();
        let join = thread::Builder::new()
            .name("rc-openal-owner".into())
            .spawn(move || {
                let result =
                    OpenAl::open_loopback(Path::new(&path), rate).and_then(|backend| match input {
                        Input::Scene(bank) => {
                            rc_package::audio_scene::Scene::open(backend, &bank, epoch)
                                .map(|v| Engine::Scene(Box::new(v)))
                        }
                        Input::Wave(wave) => Live::open(backend, &wave, rate, epoch)
                            .map(|v| Engine::Wave(Box::new(v))),
                        Input::Mixed(bundle) => {
                            rc_package::audio_mixed_live::Mixed::open(backend, *bundle, epoch)
                                .map(|v| Engine::Mixed(Box::new(v)))
                        }
                    });
                let mut live = match result {
                    Ok(l) => l,
                    Err(e) => {
                        let _ = ready.send(Err(e));
                        return;
                    }
                };
                if ready.send(Ok(())).is_err() {
                    return;
                }
                while let Ok(request) = requests.recv() {
                    match request {
                        Request::Stop => break,
                        Request::Call { op, bytes, reply } => {
                            if reply.send(live.call(op, &bytes)).is_err() {
                                break;
                            }
                        }
                        Request::Render {
                            frames,
                            epoch,
                            sequence,
                            reply,
                        } => {
                            if reply.send(live.render(frames, epoch, sequence)).is_err() {
                                break;
                            }
                        }
                    }
                }
                // Normal worker scope, before thread/TLS teardown; no context destruction in caller TLS.
                drop(live);
            })
            .map_err(|e| e.to_string())?;
        let w = Arc::new(Worker {
            id,
            caller: thread::current().id(),
            sender,
            join: Mutex::new(Some(join)),
            closed: AtomicBool::new(false),
        });
        r.insert(id, w.clone());
        drop(r);
        match startup.recv() {
            Ok(Ok(())) => {
                *owner = Some(Guard(w));
                Ok(id)
            }
            result => {
                w.closed.store(true, Ordering::Release);
                if let Some(h) = w
                    .join
                    .lock()
                    .map_err(|_| "Native join lock poisoned")?
                    .take()
                {
                    let _ = h.join();
                }
                registry()
                    .lock()
                    .map_err(|_| "Native mixer registry poisoned")?
                    .remove(&id);
                Err(match result {
                    Ok(Err(e)) => e,
                    _ => "Native mixer startup worker failed".into(),
                })
            }
        }
    })
}
fn render(id: u64, frames: usize, epoch: u64, sequence: u64) -> Result<Vec<u8>, String> {
    let w = owned(id)?;
    let (reply, response) = sync_channel(1);
    w.sender
        .send(Request::Render {
            frames,
            epoch,
            sequence,
            reply,
        })
        .map_err(|_| "Native worker closed")?;
    response.recv().map_err(|_| "Native worker failed")?
}
fn call(id: u64, op: u8, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let worker = owned(id)?;
    let (reply, response) = sync_channel(1);
    worker
        .sender
        .send(Request::Call { op, bytes, reply })
        .map_err(|_| "Native worker closed")?;
    response.recv().map_err(|_| "Native worker failed")?
}
fn close(id: u64) -> Result<(), String> {
    let w = owned(id)?;
    OWNER.with(|cell| {
        let mut owner = cell.borrow_mut();
        if owner.as_ref().is_none_or(|g| g.0.id != id) {
            return Err("Foreign/stale native mixer guard".into());
        }
        owner.take();
        Ok::<_, String>(())
    })?;
    // Caller guard signalled Stop. Joining is permitted here, outside caller TLS destruction.
    let result = if let Some(h) = w
        .join
        .lock()
        .map_err(|_| "Native join lock poisoned")?
        .take()
    {
        h.join()
            .map_err(|_| "Native mixer worker panicked".to_owned())
    } else {
        Ok(())
    };
    registry()
        .lock()
        .map_err(|_| "Native mixer registry poisoned")?
        .remove(&id);
    result
}

/// Open a five-second looping WAV diagnostic on the calling producer thread.
/// # Safety
/// Path and wave point to their readable lengths; error is writable for capacity bytes. No overlaps.
#[no_mangle]
pub unsafe extern "C" fn rc_live_open(
    path: *const u8,
    path_len: usize,
    wave: *const u8,
    wave_len: usize,
    rate: u32,
    epoch: u64,
    error: *mut c_char,
    capacity: usize,
) -> u64 {
    ffi_open(
        path, path_len, wave, wave_len, rate, epoch, error, capacity, 0,
    )
}
#[allow(clippy::too_many_arguments)]
unsafe fn ffi_open(
    path: *const u8,
    path_len: usize,
    wave: *const u8,
    wave_len: usize,
    rate: u32,
    epoch: u64,
    error: *mut c_char,
    capacity: usize,
    mode: u8,
) -> u64 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if path.is_null()
            || wave.is_null()
            || path_len == 0
            || path_len > 4096
            || wave_len
                > if mode == 2 {
                    rc_package::audio_scene::MAX_BANK + rc_package::mission_music::MAX_BANK + 12
                } else if mode == 1 {
                    rc_package::audio_mixed_live::MAX_BUNDLE
                } else {
                    rc_package::audio_live::MAX_WAVE
                }
        {
            return Err("Invalid native mixer buffers".into());
        }
        let path = std::str::from_utf8(std::slice::from_raw_parts(path, path_len))
            .map_err(|e| e.to_string())?;
        open_mode(
            path,
            std::slice::from_raw_parts(wave, wave_len),
            rate,
            epoch,
            mode,
        )
    }))
    .unwrap_or_else(|_| Err("Native mixer panic".into()));
    match result {
        Ok(id) => id,
        Err(message) => {
            if !error.is_null() && capacity > 0 {
                let n = message.len().min(capacity - 1);
                std::ptr::copy_nonoverlapping(message.as_ptr(), error.cast::<u8>(), n);
                *error.add(n) = 0;
            }
            0
        }
    }
}
/// Open a bounded mixed-host bundle on the existing dedicated owner worker.
/// # Safety
/// Same disjoint readable/writable buffer contract as rc_live_open.
#[no_mangle]
pub unsafe extern "C" fn rc_mixed_open(
    path: *const u8,
    path_len: usize,
    data: *const u8,
    len: usize,
    epoch: u64,
    error: *mut c_char,
    capacity: usize,
) -> u64 {
    ffi_open(path, path_len, data, len, 48000, epoch, error, capacity, 1)
}
/// Open the mission WAV bank on the common owner worker.
/// # Safety
/// Same readable/writable buffer contract as rc_live_open.
#[no_mangle]
pub unsafe extern "C" fn rc_scene_audio_open(
    path: *const u8,
    path_len: usize,
    bank: *const u8,
    len: usize,
    epoch: u64,
    error: *mut c_char,
    capacity: usize,
) -> u64 {
    ffi_open(path, path_len, bank, len, 48000, epoch, error, capacity, 2)
}
/// Export bounded pause continuity on the owner thread. No controlled render clock advance.
/// # Safety
/// Output is null/zero for a size query, otherwise capacity writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_scene_audio_snapshot(
    id: u64,
    output: *mut u8,
    capacity: usize,
) -> isize {
    scene_bytes(id, 4, output, capacity)
}
/// Read tracked pool counters without querying or advancing the mixer.
/// # Safety
/// Output is null/zero for a size query, otherwise capacity writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_scene_audio_status(id: u64, output: *mut u8, capacity: usize) -> isize {
    scene_bytes(id, 7, output, capacity)
}
unsafe fn scene_bytes(id: u64, op: u8, output: *mut u8, capacity: usize) -> isize {
    catch_unwind(AssertUnwindSafe(|| match call(id, op, vec![]) {
        Ok(bytes) => {
            if output.is_null() {
                return if capacity == 0 {
                    bytes.len() as isize
                } else {
                    -1
                };
            }
            if bytes.len() > capacity {
                return -1;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
            bytes.len() as isize
        }
        Err(_) => -1,
    }))
    .unwrap_or(-1)
}
/// Confirm received/discarded events on the owning producer, without rendering.
#[no_mangle]
pub extern "C" fn rc_scene_audio_consume(id: u64, cursor: u64) -> i32 {
    if cursor > i64::MAX as u64 {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        if call(id, 8, cursor.to_le_bytes().to_vec()).is_ok() {
            0
        } else {
            -1
        }
    }))
    .unwrap_or(-1)
}
/// Restore into a new mission scene, before the first render or input frame.
/// # Safety
/// Data points to len readable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_scene_audio_restore(id: u64, data: *const u8, len: usize) -> i32 {
    if data.is_null() || !(12..=rc_package::audio_scene::MAX_RESUME).contains(&len) {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        if call(id, 5, std::slice::from_raw_parts(data, len).to_vec()).is_ok() {
            0
        } else {
            -1
        }
    }))
    .unwrap_or(-1)
}
/// Apply bounded master/effects/dialog/ambient gains on the owning mission worker.
/// # Safety
/// Data points to len readable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_scene_audio_mix(id: u64, data: *const u8, len: usize) -> i32 {
    if data.is_null() || !(len == 24 || len == 28) {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        if call(id, 6, std::slice::from_raw_parts(data, len).to_vec()).is_ok() {
            0
        } else {
            -1
        }
    }))
    .unwrap_or(-1)
}
/// Submit one fully validated mission event/loop pair.
/// # Safety
/// Events and loops point to their readable lengths; no overlapping live native storage.
#[no_mangle]
pub unsafe extern "C" fn rc_scene_audio_update(
    id: u64,
    events: *const u8,
    event_len: usize,
    loops: *const u8,
    loop_len: usize,
) -> i32 {
    if events.is_null()
        || loops.is_null()
        || event_len > 12 + 256 * 48
        || loop_len > 12 + 32 + 16 * 44 + 80
    {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let mut bytes = Vec::with_capacity(4 + event_len + loop_len);
        bytes.extend((event_len as u32).to_le_bytes());
        bytes.extend(std::slice::from_raw_parts(events, event_len));
        bytes.extend(std::slice::from_raw_parts(loops, loop_len));
        if call(id, 3, bytes).is_ok() {
            0
        } else {
            -1
        }
    }))
    .unwrap_or(-1)
}
/// Submit one sequenced control. No render clock is advanced.
/// # Safety
/// Data points to len readable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_mixed_control(id: u64, data: *const u8, len: usize) -> i32 {
    if data.is_null() || len > rc_package::audio_mixed_live::MAX_CONTROL {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        if call(id, 0, std::slice::from_raw_parts(data, len).to_vec()).is_ok() {
            0
        } else {
            -1
        }
    }))
    .unwrap_or(-1)
}
/// Query snapshot (kind 1) or remaining frames/control sequence (kind 2).
/// A null/zero buffer is a pure size query; short capacity never mutates state.
/// # Safety
/// Non-null output points to capacity writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_mixed_export(
    id: u64,
    kind: u32,
    output: *mut u8,
    capacity: usize,
) -> isize {
    if kind != 1 && kind != 2 {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| match call(id, kind as u8, vec![]) {
        Ok(bytes) => {
            if output.is_null() {
                return if capacity == 0 {
                    bytes.len() as isize
                } else {
                    -1
                };
            }
            if capacity < bytes.len() {
                return -1;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
            bytes.len() as isize
        }
        Err(_) => -1,
    }))
    .unwrap_or(-1)
}
/// Render exactly one previously reserved packet. Insufficient output capacity never advances time.
/// # Safety
/// Output points to capacity writable bytes and cannot overlap any live mixer storage.
#[no_mangle]
pub unsafe extern "C" fn rc_live_render(
    id: u64,
    frames: usize,
    epoch: u64,
    sequence: u64,
    output: *mut u8,
    capacity: usize,
) -> isize {
    if frames == 0 || frames > MAX_FRAMES || output.is_null() || capacity < HEADER + frames * 4 {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        match render(id, frames, epoch, sequence) {
            Ok(bytes) => {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
                bytes.len() as isize
            }
            Err(_) => -1,
        }
    }))
    .unwrap_or(-1)
}
/// Release only on the owning thread; repeated/foreign release returns -1.
#[no_mangle]
pub extern "C" fn rc_live_close(id: u64) -> i32 {
    catch_unwind(AssertUnwindSafe(|| if close(id).is_ok() { 0 } else { -1 })).unwrap_or(-1)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    const WAVE: &[u8] = include_bytes!("../../../fixtures/audio-live-tone.wav");
    fn library() -> String {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll")
            .to_string_lossy()
            .into_owned()
    }
    fn start() -> u64 {
        open(&library(), WAVE, 48000, 1).unwrap()
    }
    #[test]
    fn soundtrack_bank_pose_mix_and_resume_reach_owner_worker() {
        unsafe {
            let path = library();
            let wave = [b"RCSND001".as_slice(), &0u32.to_le_bytes()].concat();
            let ogg = include_bytes!("../../../fixtures/audio-vorbis/stereo.ogg");
            let music = [
                b"RCMUB001".as_slice(),
                &1u32.to_le_bytes(),
                &(ogg.len() as u32).to_le_bytes(),
                ogg,
            ]
            .concat();
            let bank = [
                b"RCSND002".as_slice(),
                &(wave.len() as u32).to_le_bytes(),
                &wave,
                &music,
            ]
            .concat();
            let mut error = [0u8; 512];
            let id = rc_scene_audio_open(
                path.as_ptr(),
                path.len(),
                bank.as_ptr(),
                bank.len(),
                1,
                error.as_mut_ptr().cast(),
                512,
            );
            assert_ne!(id, 0);
            let mut pose = b"RCLOOP04".to_vec();
            pose.extend([0; 24]);
            let commands = [
                rc_package::mission_music::Command {
                    track: 0,
                    activation: 1,
                    looped: true,
                    ..Default::default()
                },
                Default::default(),
            ];
            let packet = [
                b"RCLOOP05".as_slice(),
                &(pose.len() as u32).to_le_bytes(),
                &pose,
                &rc_package::mission_music::packet(&commands),
            ]
            .concat();
            let empty = [b"RCAUD003".as_slice(), &0u32.to_le_bytes()].concat();
            assert_eq!(
                rc_scene_audio_update(
                    id,
                    empty.as_ptr(),
                    empty.len(),
                    packet.as_ptr(),
                    packet.len()
                ),
                0
            );
            let mix = [b"RCMIX002".as_slice(), &1f32.to_le_bytes().repeat(5)].concat();
            assert_eq!(rc_scene_audio_mix(id, mix.as_ptr(), mix.len()), 0);
            let pcm = render(id, 480, 1, 0).unwrap();
            assert!(pcm[40..]
                .chunks_exact(2)
                .any(|v| i16::from_le_bytes(v.try_into().unwrap()).abs() > 2));
            let saved = call(id, 4, vec![]).unwrap();
            assert_eq!(rc_live_close(id), 0);
            let id = rc_scene_audio_open(
                path.as_ptr(),
                path.len(),
                bank.as_ptr(),
                bank.len(),
                2,
                error.as_mut_ptr().cast(),
                512,
            );
            assert_ne!(id, 0);
            assert_eq!(rc_scene_audio_restore(id, saved.as_ptr(), saved.len()), 0);
            let restored = call(id, 4, vec![]).unwrap();
            let a: serde_json::Value = serde_json::from_slice(&saved[12..]).unwrap();
            let b: serde_json::Value = serde_json::from_slice(&restored[12..]).unwrap();
            assert_eq!(a["music"], b["music"]);
            assert_eq!(rc_live_close(id), 0);
        }
    }
    #[test]
    fn scene_pause_snapshot_size_query_and_offset_restore_cross_owner() {
        unsafe {
            let path = library();
            let mut bank = b"RCSND001".to_vec();
            bank.extend(1u32.to_le_bytes());
            bank.extend((WAVE.len() as u32).to_le_bytes());
            bank.extend(WAVE);
            let mut error = [0u8; 512];
            let id = rc_scene_audio_open(
                path.as_ptr(),
                path.len(),
                bank.as_ptr(),
                bank.len(),
                1,
                error.as_mut_ptr().cast(),
                error.len(),
            );
            assert_ne!(id, 0);
            let mut events = b"RCAUD003".to_vec();
            events.extend(1u32.to_le_bytes());
            events.extend(1u64.to_le_bytes());
            events.extend(0u64.to_le_bytes());
            for v in [0u32, 42, 1, 0] {
                events.extend(v.to_le_bytes());
            }
            for v in [1f32, 1., 0.] {
                events.extend(v.to_le_bytes());
            }
            events.extend(0u32.to_le_bytes());
            let mut loops = b"RCLOOP04".to_vec();
            loops.resize(32, 0);
            assert_eq!(
                rc_scene_audio_update(
                    id,
                    events.as_ptr(),
                    events.len(),
                    loops.as_ptr(),
                    loops.len()
                ),
                0
            );
            let mut pcm = vec![0u8; 360];
            assert_eq!(
                rc_live_render(id, 80, 1, 0, pcm.as_mut_ptr(), pcm.len()),
                360
            );
            let n = rc_scene_audio_snapshot(id, std::ptr::null_mut(), 0);
            assert!(n > 12);
            let mut saved = vec![0; n as usize];
            assert_eq!(
                rc_scene_audio_snapshot(id, saved.as_mut_ptr(), saved.len() - 1),
                -1
            );
            assert_eq!(
                rc_scene_audio_snapshot(id, saved.as_mut_ptr(), saved.len()),
                n
            );
            let old: serde_json::Value = serde_json::from_slice(&saved[12..]).unwrap();
            assert_eq!(old["dialogs"].as_array().unwrap().len(), 1);
            assert!(old["dialogs"][0][1].as_u64().unwrap() > 0);
            assert_eq!(rc_live_close(id), 0);
            let next = rc_scene_audio_open(
                path.as_ptr(),
                path.len(),
                bank.as_ptr(),
                bank.len(),
                7,
                error.as_mut_ptr().cast(),
                error.len(),
            );
            assert_ne!(next, 0);
            assert_eq!(rc_scene_audio_restore(next, saved.as_ptr(), saved.len()), 0);
            assert_eq!(
                rc_scene_audio_restore(next, saved.as_ptr(), saved.len()),
                -1
            );
            let m = rc_scene_audio_snapshot(next, std::ptr::null_mut(), 0);
            let mut rebuilt = vec![0; m as usize];
            assert_eq!(
                rc_scene_audio_snapshot(next, rebuilt.as_mut_ptr(), rebuilt.len()),
                m
            );
            let new: serde_json::Value = serde_json::from_slice(&rebuilt[12..]).unwrap();
            assert_eq!(old["dialogs"], new["dialogs"]);
            assert_eq!(
                rc_live_render(next, 80, 1, 0, pcm.as_mut_ptr(), pcm.len()),
                -1
            );
            assert_eq!(
                rc_live_render(next, 80, 7, 0, pcm.as_mut_ptr(), pcm.len()),
                360
            );
            assert_eq!(rc_live_close(next), 0);
        }
    }
    #[test]
    fn resume_ffi_rejects_wrong_type_and_malformed_payload_without_advancing() {
        unsafe {
            let id = start();
            assert_eq!(rc_scene_audio_snapshot(id, std::ptr::null_mut(), 0), -1);
            assert_eq!(rc_scene_audio_restore(id, [0u8; 12].as_ptr(), 12), -1);
            assert_eq!(rc_scene_audio_restore(id, std::ptr::null(), 0), -1);
            let mix = [b"RCMIX001".as_slice(), &1f32.to_le_bytes().repeat(4)].concat();
            assert_eq!(rc_scene_audio_mix(id, mix.as_ptr(), mix.len()), -1);
            assert!(render(id, 480, 1, 0).is_ok());
            assert_eq!(rc_live_close(id), 0);
        }
    }
    #[test]
    fn mission_mix_abi_validates_packet_and_preserves_pcm_epoch() {
        unsafe {
            let path = library();
            let bank = [
                b"RCSND001".as_slice(),
                &1u32.to_le_bytes(),
                &(WAVE.len() as u32).to_le_bytes(),
                WAVE,
            ]
            .concat();
            let mut error = [0; 1024];
            let id = rc_scene_audio_open(
                path.as_ptr(),
                path.len(),
                bank.as_ptr(),
                bank.len(),
                1,
                error.as_mut_ptr(),
                error.len(),
            );
            assert_ne!(id, 0);
            assert_eq!(rc_scene_audio_mix(id, std::ptr::null(), 24), -1);
            let mut mix = [b"RCMIX001".as_slice(), &1f32.to_le_bytes().repeat(4)].concat();
            assert_eq!(rc_scene_audio_mix(id, mix.as_ptr(), 23), -1);
            mix[8..12].copy_from_slice(&f32::NAN.to_le_bytes());
            assert_eq!(rc_scene_audio_mix(id, mix.as_ptr(), 24), -1);
            mix[8..12].copy_from_slice(&0.5f32.to_le_bytes());
            assert_eq!(rc_scene_audio_mix(id, mix.as_ptr(), 24), 0);
            assert!(render(id, 480, 1, 0).is_ok());
            assert_eq!(rc_live_close(id), 0);
            assert_eq!(rc_scene_audio_mix(id, mix.as_ptr(), 24), -1);
        }
    }
    #[test]
    fn mission_pool_status_is_readonly_and_rejects_other_hosts() {
        unsafe {
            let other = start();
            assert_eq!(rc_scene_audio_status(other, std::ptr::null_mut(), 0), -1);
            assert_eq!(rc_live_close(other), 0);
            let path = library();
            let bank = [
                b"RCSND001".as_slice(),
                &1u32.to_le_bytes(),
                &(WAVE.len() as u32).to_le_bytes(),
                WAVE,
            ]
            .concat();
            let mut error = [0; 1024];
            let id = rc_scene_audio_open(
                path.as_ptr(),
                path.len(),
                bank.as_ptr(),
                bank.len(),
                1,
                error.as_mut_ptr(),
                error.len(),
            );
            assert_ne!(id, 0);
            assert_eq!(rc_scene_audio_status(id, std::ptr::null_mut(), 0), 88);
            assert_eq!(rc_scene_audio_status(id, std::ptr::null_mut(), 1), -1);
            let mut bytes = [0; 88];
            assert_eq!(rc_scene_audio_status(id, bytes.as_mut_ptr(), 87), -1);
            assert_eq!(rc_scene_audio_status(id, bytes.as_mut_ptr(), 88), 88);
            let before = bytes;
            assert_eq!(rc_scene_audio_status(id, bytes.as_mut_ptr(), 88), 88);
            assert_eq!(before, bytes);
            assert!(render(id, 480, 1, 0).is_ok());
            assert_eq!(rc_scene_audio_status(id, bytes.as_mut_ptr(), 88), 88);
            assert_eq!(u64::from_le_bytes(bytes[32..40].try_into().unwrap()), 480);
            assert_eq!(&bytes[..8], b"RCPOOL01");
            assert_eq!(rc_live_close(id), 0);
            assert_eq!(rc_scene_audio_status(id, bytes.as_mut_ptr(), 88), -1);
        }
    }
    #[test]
    fn mission_consumption_receipt_rejects_stale_or_wrong_host_without_rendering() {
        unsafe {
            let other = start();
            assert_eq!(rc_scene_audio_consume(other, 1), -1);
            assert_eq!(rc_live_close(other), 0);
            let path = library();
            let bank = [
                b"RCSND001".as_slice(),
                &1u32.to_le_bytes(),
                &(WAVE.len() as u32).to_le_bytes(),
                WAVE,
            ]
            .concat();
            let mut error = [0; 1024];
            let id = rc_scene_audio_open(
                path.as_ptr(),
                path.len(),
                bank.as_ptr(),
                bank.len(),
                1,
                error.as_mut_ptr(),
                error.len(),
            );
            assert_ne!(id, 0);
            assert_eq!(rc_scene_audio_consume(id, u64::MAX), -1);
            assert_eq!(rc_scene_audio_consume(id, 3), 0);
            assert_eq!(rc_scene_audio_consume(id, 3), 0);
            assert_eq!(rc_scene_audio_consume(id, 2), -1);
            let mut status = [0; 88];
            assert_eq!(rc_scene_audio_status(id, status.as_mut_ptr(), 88), 88);
            assert_eq!(u64::from_le_bytes(status[32..40].try_into().unwrap()), 0);
            assert_eq!(u64::from_le_bytes(status[40..48].try_into().unwrap()), 3);
            assert!(render(id, 480, 1, 0).is_ok());
            assert_eq!(rc_live_close(id), 0);
            assert_eq!(rc_scene_audio_consume(id, 4), -1);
        }
    }
    #[test]
    fn scene_audio_abi_rebuilds_with_new_owner_and_rejects_stale_epoch() {
        unsafe {
            let path = library();
            let mut bank = b"RCSND001".to_vec();
            bank.extend(1u32.to_le_bytes());
            bank.extend((WAVE.len() as u32).to_le_bytes());
            bank.extend(WAVE);
            let mut error = [0u8; 512];
            let id = rc_scene_audio_open(
                path.as_ptr(),
                path.len(),
                bank.as_ptr(),
                bank.len(),
                4,
                error.as_mut_ptr().cast(),
                error.len(),
            );
            assert_ne!(id, 0);
            let events = [b"RCAUD003".as_slice(), &0u32.to_le_bytes()].concat();
            let mut loops = b"RCLOOP04".to_vec();
            loops.resize(32, 0);
            assert_eq!(
                rc_scene_audio_update(
                    id,
                    events.as_ptr(),
                    events.len(),
                    loops.as_ptr(),
                    loops.len()
                ),
                0
            );
            let mut out = vec![0u8; 1960];
            assert_eq!(
                rc_live_render(id, 480, 3, 0, out.as_mut_ptr(), out.len()),
                -1
            );
            assert_eq!(
                rc_live_render(id, 480, 4, 0, out.as_mut_ptr(), out.len()),
                1960
            );
            assert_eq!(rc_live_close(id), 0);
            let next = rc_scene_audio_open(
                path.as_ptr(),
                path.len(),
                bank.as_ptr(),
                bank.len(),
                5,
                error.as_mut_ptr().cast(),
                error.len(),
            );
            assert_ne!(next, 0);
            assert_ne!(next, id);
            assert_eq!(
                rc_scene_audio_update(
                    id,
                    events.as_ptr(),
                    events.len(),
                    loops.as_ptr(),
                    loops.len()
                ),
                -1
            );
            assert_eq!(rc_live_close(next), 0);
        }
    }
    #[test]
    fn scene_audio_wrong_session_type_and_bad_bank_are_rejected() {
        unsafe {
            let id = start();
            let events = b"RCAUD003\0\0\0\0";
            let mut loops = b"RCLOOP04".to_vec();
            loops.resize(32, 0);
            assert_eq!(
                rc_scene_audio_update(
                    id,
                    events.as_ptr(),
                    events.len(),
                    loops.as_ptr(),
                    loops.len()
                ),
                -1
            );
            assert!(render(id, 480, 1, 0).is_ok());
            assert_eq!(rc_live_close(id), 0);
            let path = library();
            let mut error = [0u8; 512];
            assert_eq!(
                rc_scene_audio_open(
                    path.as_ptr(),
                    path.len(),
                    events.as_ptr(),
                    events.len(),
                    1,
                    error.as_mut_ptr().cast(),
                    error.len()
                ),
                0
            );
            assert_ne!(error[0], 0);
        }
    }
    #[test]
    fn mixed_exports_and_controls_reject_wave_session_without_changing_clock() {
        unsafe {
            let id = start();
            let c = br#"{"sequence":0,"action":"StopAll"}"#;
            assert_eq!(rc_mixed_control(id, c.as_ptr(), c.len()), -1);
            assert_eq!(rc_mixed_export(id, 1, std::ptr::null_mut(), 0), -1);
            assert_eq!(rc_mixed_export(id, 2, std::ptr::null_mut(), 0), -1);
            assert!(render(id, 480, 1, 0).is_ok());
            assert_eq!(rc_live_close(id), 0);
        }
    }
    #[test]
    fn malformed_mixed_bundle_does_not_create_an_owner() {
        unsafe {
            let p = library();
            let bad = b"{}";
            let mut error = [0u8; 512];
            assert_eq!(
                rc_mixed_open(
                    p.as_ptr(),
                    p.len(),
                    bad.as_ptr(),
                    bad.len(),
                    1,
                    error.as_mut_ptr().cast(),
                    error.len()
                ),
                0
            );
            assert_ne!(error[0], 0);
            let id = start();
            assert_eq!(rc_live_close(id), 0);
        }
    }
    #[test]
    fn ffi_short_output_does_not_render_or_advance() {
        unsafe {
            let id = start();
            let mut out = [0u8; 40 + 480 * 4];
            assert_eq!(rc_live_render(id, 480, 1, 0, out.as_mut_ptr(), 40), -1);
            assert_eq!(
                rc_live_render(id, 480, 1, 0, out.as_mut_ptr(), out.len()),
                out.len() as isize
            );
            let p = rc_package::audio_pcm::Packet::decode(&out).unwrap();
            assert_eq!(p.sequence, 0);
            assert!(p.samples.iter().any(|&s| s != 0));
            assert_eq!(
                rc_live_render(id, 480, 1, 0, out.as_mut_ptr(), out.len()),
                -1
            );
            assert_eq!(rc_live_close(id), 0);
        }
    }
    #[test]
    fn foreign_thread_and_stale_ids_cannot_touch_owner() {
        let id = start();
        std::thread::spawn(move || {
            assert!(render(id, 480, 1, 0).is_err());
            assert_eq!(rc_live_close(id), -1);
            let own = start();
            assert_ne!(id, own);
            assert!(render(id, 480, 1, 0).is_err());
            assert_eq!(rc_live_close(own), 0);
        })
        .join()
        .unwrap();
        assert!(render(id, 480, 1, 0).is_ok());
        assert_eq!(rc_live_close(id), 0);
        assert_eq!(rc_live_close(id), -1);
    }
    #[test]
    fn duplicate_open_preserves_session_and_reopen_has_new_id() {
        let id = start();
        assert!(open(&library(), WAVE, 48000, 1).is_err());
        assert!(render(id, 480, 1, 0).is_ok());
        assert_eq!(rc_live_close(id), 0);
        let next = start();
        assert_ne!(id, next);
        assert!(render(id, 480, 1, 0).is_err());
        assert_eq!(rc_live_close(next), 0);
    }
    #[test]
    fn ffi_rejects_bad_buffers_and_reports_bounded_error() {
        unsafe {
            let mut error = [77u8; 16];
            let result = rc_live_open(
                std::ptr::null(),
                0,
                WAVE.as_ptr(),
                WAVE.len(),
                48000,
                1,
                error.as_mut_ptr().cast(),
                8,
            );
            assert_eq!(result, 0);
            assert_eq!(error[7], 0);
            assert_eq!(error[8], 77);
            let p = library();
            assert_eq!(
                rc_live_open(
                    p.as_ptr(),
                    p.len(),
                    WAVE.as_ptr(),
                    WAVE.len(),
                    0,
                    1,
                    error.as_mut_ptr().cast(),
                    error.len()
                ),
                0
            );
            assert_eq!(rc_live_close(0), -1);
            assert_eq!(rc_live_render(0, 0, 0, 0, std::ptr::null_mut(), 0), -1);
        }
    }
    #[test]
    fn ffi_finite_session_stops_at_exact_budget() {
        unsafe {
            let id = start();
            let mut out = vec![0u8; 40 + 4000 * 4];
            for seq in 0..60 {
                assert_eq!(
                    rc_live_render(id, 4000, 1, seq, out.as_mut_ptr(), out.len()),
                    out.len() as isize
                );
            }
            assert_eq!(
                rc_live_render(id, 1, 1, 60, out.as_mut_ptr(), out.len()),
                -1
            );
            assert_eq!(rc_live_close(id), 0);
        }
    }
    #[test]
    fn thread_exit_drops_its_unreleased_context() {
        std::thread::spawn(|| {
            let id = start();
            assert!(render(id, 480, 1, 0).is_ok());
        })
        .join()
        .unwrap();
        let id = start();
        assert!(render(id, 480, 1, 0).is_ok());
        assert_eq!(rc_live_close(id), 0);
    }
}
