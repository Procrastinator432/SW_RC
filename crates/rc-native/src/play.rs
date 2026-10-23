//! C ABI for the shared playable session. Handles are independent of diagnostic scene handles.
use rc_game::{Game, Input, Phase};
use rc_package::performance;
use std::{
    collections::HashMap,
    ffi::c_char,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};
struct Session {
    game: Game,
    graphics: [Option<Vec<u8>>; 2],
    scene_identity: String,
}
impl std::ops::Deref for Session {
    type Target = Game;
    fn deref(&self) -> &Game {
        &self.game
    }
}
impl std::ops::DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Game {
        &mut self.game
    }
}
static GAMES: OnceLock<Mutex<HashMap<u64, Session>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
/// Read-only enemy observations for opt-in profiling; no simulation or cache mutation.
/// # Safety
/// out is null with capacity zero or points to capacity writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_game_actor_status(id: u64, out: *mut u8, capacity: usize) -> isize {
    let Ok(registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get(&id) else {
        return -1;
    };
    let mut actors: Vec<_> = game
        .state
        .enemies
        .iter()
        .map(|enemy| {
            let target = [
                enemy.position[0],
                enemy.position[1],
                enemy.position[2] + 15.,
            ];
            serde_json::json!({"position":enemy.position,"health":enemy.health,
            "visible":game.wall_fraction(game.eye(),target).is_ok_and(|f|f>=1.)})
        })
        .collect();
    if let Some(c) = game.state.map.as_ref().and_then(|s| s.companion.as_ref()) {
        if c.joined {
            actors.push(serde_json::json!({"position":c.position,"health":c.health,"friendly":true,"role":"Scorch","visible":game.wall_fraction(game.eye(),c.position).is_ok_and(|f|f>=1.)}));
        }
    }
    if let Some(c) = game.state.map.as_ref().and_then(|s| {
        s.fixer
            .as_ref()
            .or_else(|| s.gameplay.as_ref().map(|g| &g.fixer))
    }) {
        if c.health > 0 {
            actors.push(serde_json::json!({"position":c.position,"health":c.health,"friendly":true,"role":"Fixer","visible":game.wall_fraction(game.eye(),c.position).is_ok_and(|f|f>=1.)}));
        }
    }

    if let Some(c) = game.state.map.as_ref().and_then(|m| {
        m.sev
            .as_ref()
            .or_else(|| m.opening.as_ref().map(|s| &s.sev))
    }) {
        actors.push(serde_json::json!({"position":c.position,"health":c.health,"friendly":true,"role":"Sev","joined":c.joined,"visible":game.wall_fraction(game.eye(),c.position).is_ok_and(|f|f>=1.)}));
    }
    let Ok(bytes) = serde_json::to_vec(&actors) else {
        return -1;
    };
    if out.is_null() {
        return if capacity == 0 {
            bytes.len() as isize
        } else {
            -1
        };
    }
    if capacity < bytes.len() {
        return -1;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
    bytes.len() as isize
}
/// UTF-8 mission guidance, excluding terminator; zero bytes for legacy scenes.
fn campaign_map_id(destination: Option<&str>, opening: bool) -> i32 {
    match destination {
        Some("GEO_01B.ctm#playerstart") => 2,
        Some("GEO_01C.ctm#playerstart") => 3,
        Some("GEO_03A.ctm#playerstart") => 4,
        None if opening => 1,
        _ => 0,
    }
}
/// Stable campaign identity from loaded assets, independent of localized guidance.
/// 1=A, 2=B, 3=C, 4=factory; 0=legacy scene, -1=invalid handle.
#[no_mangle]
pub extern "C" fn rc_game_campaign_map(id: u64) -> i32 {
    let Ok(registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get(&id) else {
        return -1;
    };
    campaign_map_id(
        game.map.as_ref().map(|m| m.assets.destination.as_str()),
        game.opening.is_some(),
    )
}
/// UTF-8 mission guidance, excluding terminator; zero bytes for legacy scenes.
/// # Safety
/// out is null for a size query or points to capacity writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_game_mission_status(id: u64, out: *mut u8, capacity: usize) -> isize {
    let Ok(registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get(&id) else {
        return -1;
    };
    let text = game.mission_status();
    if out.is_null() {
        return if capacity == 0 {
            text.len() as isize
        } else {
            -1
        };
    }
    if capacity < text.len() {
        return -1;
    }
    std::ptr::copy_nonoverlapping(text.as_ptr(), out, text.len());
    text.len() as isize
}
/// WAV bank (0), legacy 24-byte RCAUD001 events (1), or 32-byte RCAUD002
/// events with original category/no-override metadata (2).
/// Versioned UTF-8 map handoff JSON; zero bytes before the original exit.
/// Queries and copies are repeatable and do not consume the handoff.
/// # Safety
/// out is null with zero capacity or points to capacity writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_game_map_handoff(id: u64, out: *mut u8, capacity: usize) -> isize {
    if out.is_null() && capacity != 0 {
        return -1;
    }
    let Ok(registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get(&id) else {
        return -1;
    };
    let bytes = match game.campaign_handoff() {
        Ok(Some(h)) => match serde_json::to_vec(&h) {
            Ok(b) => b,
            Err(_) => return -1,
        },
        Ok(None) => Vec::new(),
        Err(_) => return -1,
    };
    if out.is_null() {
        return bytes.len() as isize;
    }
    if capacity < bytes.len() {
        return -1;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
    bytes.len() as isize
}
/// WAV bank (0), consumable legacy/enriched cues (1/2), or current ambient
/// snapshot RCLOOP01 (3), parameters RCAUD003 (4), spatial RCLOOP02 (5).
/// Queries/short copies preserve cues; full 1/2/4 copies consume them. 3/5 are read-only.
/// # Safety
/// out is null with capacity zero, or points to capacity writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_game_audio(id: u64, kind: u32, out: *mut u8, capacity: usize) -> isize {
    let Ok(mut registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get_mut(&id) else {
        return -1;
    };
    let packet = match kind {
        5 | 6 => {
            let Ok(sources) = game.spatial_ambient_sources() else {
                return -1;
            };
            if sources.len() > 16 {
                return -1;
            }
            let (position, yaw) = game.audio_listener();
            let mut b = if kind == 6 {
                b"RCLOOP03".to_vec()
            } else {
                b"RCLOOP02".to_vec()
            };
            b.extend_from_slice(&(sources.len() as u32).to_le_bytes());
            for v in position.into_iter().chain([yaw]) {
                b.extend_from_slice(&v.to_le_bytes());
            }
            for s in sources {
                b.extend_from_slice(&(s.sound as u32).to_le_bytes());
                b.extend_from_slice(&s.owner.to_le_bytes());
                for v in s.location {
                    b.extend_from_slice(&v.to_le_bytes());
                }
                for v in [
                    s.playback.as_ref().map_or(1., |p| p.volume),
                    s.playback.as_ref().map_or(1., |p| p.pitch),
                    s.playback.as_ref().map_or(0., |p| p.radius),
                ] {
                    b.extend_from_slice(&v.to_le_bytes());
                }
                b.extend_from_slice(&s.flags.to_le_bytes());
                if kind == 6 {
                    b.extend_from_slice(&s.activation.to_le_bytes());
                }
            }
            b
        }
        3 => {
            let loops = game.ambient_loops();
            let mut b = b"RCLOOP01".to_vec();
            b.extend_from_slice(&(loops.len() as u32).to_le_bytes());
            for (sound, owner) in loops {
                b.extend_from_slice(&(sound as u32).to_le_bytes());
                b.extend_from_slice(&owner.to_le_bytes());
            }
            b
        }
        0 => {
            let sounds = game.audio_bank();
            let mut b = b"RCSND001".to_vec();
            b.extend_from_slice(&(sounds.len() as u32).to_le_bytes());
            for s in sounds {
                b.extend_from_slice(&(s.wave.len() as u32).to_le_bytes());
                b.extend_from_slice(&s.wave);
            }
            b
        }
        1 | 2 | 4 => {
            let mut b = if kind == 4 {
                b"RCAUD003".to_vec()
            } else if kind == 2 {
                b"RCAUD002".to_vec()
            } else {
                b"RCAUD001".to_vec()
            };
            let events = game
                .state
                .combat
                .as_ref()
                .map(|c| c.events.as_slice())
                .unwrap_or_else(|| {
                    game.state
                        .mission
                        .as_ref()
                        .map(|s| s.events.as_slice())
                        .or_else(|| game.state.map.as_ref().map(|s| s.events.as_slice()))
                        .unwrap_or(&[])
                });
            b.extend_from_slice(&(events.len() as u32).to_le_bytes());
            for e in events {
                b.extend_from_slice(&e.sequence.to_le_bytes());
                b.extend_from_slice(&e.tick.to_le_bytes());
                b.extend_from_slice(&(e.sound as u32).to_le_bytes());
                b.extend_from_slice(&e.owner.to_le_bytes());
                if kind == 2 || kind == 4 {
                    let policy = game
                        .opening
                        .as_ref()
                        .and_then(|a| a.assets.sound_policy(e.sound, e.owner));
                    b.extend_from_slice(
                        &game
                            .weapon_playback(e.sequence)
                            .map_or_else(
                                || policy.map_or(2, |p| u32::from(p.category)),
                                |p| u32::from(p.category),
                            )
                            .to_le_bytes(),
                    );
                    b.extend_from_slice(
                        &policy.map_or(0, |p| u32::from(p.no_override)).to_le_bytes(),
                    );
                }
                if kind == 4 {
                    let p = game.weapon_playback(e.sequence);
                    b.extend_from_slice(&p.map_or(1., |p| p.volume).to_le_bytes());
                    b.extend_from_slice(&p.map_or(1., |p| p.pitch).to_le_bytes());
                    b.extend_from_slice(&p.map_or(0., |p| p.radius).to_le_bytes());
                    b.extend_from_slice(&u32::from(p.is_some()).to_le_bytes());
                }
            }
            b
        }
        _ => return -1,
    };
    if out.is_null() {
        return if capacity == 0 {
            packet.len() as isize
        } else {
            -1
        };
    }
    if capacity < packet.len() {
        return packet.len() as isize;
    }
    std::ptr::copy_nonoverlapping(packet.as_ptr(), out, packet.len());
    if kind == 1 || kind == 2 || kind == 4 {
        game.clear_weapon_playback();
        if let Some(m) = &mut game.state.map {
            m.events.clear();
        }
        if let Some(c) = &mut game.state.combat {
            c.events.clear();
        }
        if let Some(m) = &mut game.state.mission {
            m.events.clear();
        }
    }
    packet.len() as isize
}
fn games() -> &'static Mutex<HashMap<u64, Session>> {
    GAMES.get_or_init(|| Mutex::new(HashMap::new()))
}
/// Atomically replace a completed campaign session with its validated next map.
/// An invalid/mismatching payload leaves the old game and graphics untouched.
/// # Safety
/// data points to len readable bytes, error to capacity writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_game_travel(
    id: u64,
    data: *const u8,
    len: usize,
    error: *mut c_char,
    capacity: usize,
) -> i32 {
    let result = (|| -> Result<(), String> {
        if data.is_null() || len == 0 || len > 112 * 1024 * 1024 {
            return Err("Invalid destination payload".into());
        }
        let mut registry = games().lock().map_err(|_| "Game registry unavailable")?;
        let session = registry.get_mut(&id).ok_or("Invalid game handle")?;
        let next = session
            .game
            .travel_to(std::slice::from_raw_parts(data, len))?;
        *session = Session {
            game: next,
            graphics: [None, None],
            scene_identity: format!(
                "{}:{}",
                len,
                rc_game::replay::fingerprint(std::slice::from_raw_parts(data, len))
            ),
        };
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(message) => {
            if !error.is_null() && capacity > 0 {
                let n = message.len().min(capacity - 1);
                std::ptr::copy_nonoverlapping(message.as_ptr(), error as *mut u8, n);
                *error.add(n) = 0;
            }
            -1
        }
    }
}
/// # Safety
/// data points to len readable bytes, error to capacity writable bytes; no overlap.
/// Restore a saved, versioned handoff on a fresh destination map. Invalid
/// inventory/destination data leaves its initial state unchanged.
/// # Safety
/// data points to len readable bytes, error to capacity writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_game_restore_handoff(
    id: u64,
    data: *const u8,
    len: usize,
    error: *mut c_char,
    capacity: usize,
) -> i32 {
    let result = (|| -> Result<(), String> {
        if data.is_null() || len == 0 || len > 4 * 1024 * 1024 {
            return Err("Invalid saved handoff payload".into());
        }
        let h = serde_json::from_slice(std::slice::from_raw_parts(data, len))
            .map_err(|e| e.to_string())?;
        let mut registry = games().lock().map_err(|_| "Game registry unavailable")?;
        {
            let session = registry.get_mut(&id).ok_or("Invalid game handle")?;
            session.restore_handoff(&h)?;
            session.graphics[1] = None;
            Ok(())
        }
    })();
    match result {
        Ok(()) => 0,
        Err(message) => {
            if !error.is_null() && capacity > 0 {
                let n = message.len().min(capacity - 1);
                std::ptr::copy_nonoverlapping(message.as_ptr(), error as *mut u8, n);
                *error.add(n) = 0;
            }
            -1
        }
    }
}
/// Create an independent playable session.
/// # Safety
/// data points to len readable bytes, error to capacity writable bytes; no overlap.
#[no_mangle]
pub unsafe extern "C" fn rc_game_create(
    data: *const u8,
    len: usize,
    error: *mut c_char,
    capacity: usize,
) -> u64 {
    let _timing = performance::Span::new(performance::Category::BundleLoad);
    let result = if data.is_null() || len > 112 * 1024 * 1024 {
        Err("Invalid game payload".into())
    } else {
        rc_game::bundle::decode(std::slice::from_raw_parts(data, len))
    };
    let result = result.and_then(|game| {
        let mut registry = games()
            .lock()
            .map_err(|_| "Game registry unavailable".to_string())?;
        if registry.len() >= 4 {
            return Err("Too many active games".into());
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        registry.insert(
            id,
            Session {
                game,
                graphics: [None, None],
                scene_identity: format!(
                    "{}:{}",
                    len,
                    rc_game::replay::fingerprint(std::slice::from_raw_parts(data, len))
                ),
            },
        );
        Ok(id)
    });
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
/// Buttons: bit0 jump, bit1 fire, bit2 reload, bit3 walk, bit4 use, bit5 grenade. Returns tick count or -1.
#[no_mangle]
pub extern "C" fn rc_game_advance(
    id: u64,
    elapsed: f64,
    forward: f32,
    strafe: f32,
    look_x: f32,
    look_y: f32,
    buttons: u32,
) -> i32 {
    if buttons & !127 != 0 {
        return -1;
    }
    let Ok(mut registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get_mut(&id) else {
        return -1;
    };
    game.graphics = [None, None];
    game.advance(
        elapsed,
        Input {
            forward: f64::from(forward),
            strafe: f64::from(strafe),
            look: [f64::from(look_x), f64::from(look_y)],
            jump: buttons & 1 != 0,
            fire: buttons & 2 != 0,
            reload: buttons & 4 != 0,
            walking: buttons & 8 != 0,
            use_action: buttons & 16 != 0,
            throw_grenade: buttons & 32 != 0,
            crouch: buttons & 64 != 0,
        },
    )
    .map_or(-1, |n| n as i32)
}
/// Returns zero for a live handle and -1 otherwise. Command 0 pauses/clears input; 1 restarts.
#[no_mangle]
pub extern "C" fn rc_game_command(id: u64, command: u32) -> i32 {
    let Ok(mut registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get_mut(&id) else {
        return -1;
    };
    game.graphics = [None, None];
    match command {
        0 => game.suspend(),
        1 => game.restart(),
        _ => return -1,
    };
    0
}
/// # Safety
/// output points to capacity writable ARGB8888 words. Returns pixel count or -1.
#[no_mangle]
pub unsafe extern "C" fn rc_game_draw(
    id: u64,
    width: usize,
    height: usize,
    output: *mut u32,
    capacity: usize,
) -> isize {
    if output.is_null() || width.checked_mul(height).is_none_or(|n| n > capacity) {
        return -1;
    }
    let Ok(registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get(&id) else {
        return -1;
    };
    match game.render(width, height) {
        Ok(pixels) => {
            let _timing = rc_package::performance::Span::new(
                rc_package::performance::Category::NativePixelCopy,
            );
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), output, pixels.len());
            pixels.len() as isize
        }
        Err(_) => -1,
    }
}
/// Status words: HP, ammo, reserve, enemies alive, phase(0 play/1 won/2 lost), ticks,
/// shots, kills, jumps, enemy shots. Counters truncate to low32 after long sessions.
/// # Safety
/// output points to capacity writable u32 words. Returns 10 or -1.
#[no_mangle]
pub unsafe extern "C" fn rc_game_status(id: u64, output: *mut u32, capacity: usize) -> i32 {
    if output.is_null() || capacity < 10 {
        return -1;
    }
    let Ok(registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get(&id) else {
        return -1;
    };
    let s = &game.state;
    let words = [
        s.health,
        s.ammo,
        s.reserve,
        s.enemies.iter().filter(|e| e.health > 0).count() as u32,
        match s.phase {
            Phase::Playing => 0,
            Phase::Won => 1,
            Phase::Lost => 2,
        },
        s.statistics.ticks as u32,
        s.statistics.shots,
        s.statistics.kills + s.squad.as_ref().map_or(0, |q| q.kills),
        s.statistics.jumps,
        s.statistics.enemy_shots,
    ];
    std::ptr::copy_nonoverlapping(words.as_ptr(), output, words.len());
    10
}
/// Stable-checkpoint eligibility; -1 for an invalid session.
#[no_mangle]
pub extern "C" fn rc_game_checkpoint_safe(id: u64) -> i32 {
    let Ok(registry) = games().lock() else {
        return -1;
    };
    registry
        .get(&id)
        .map_or(-1, |game| i32::from(game.checkpoint_safe()))
}

/// Per-mille energy for the passive touch indicator; -1 for legacy weapons.
#[no_mangle]
pub extern "C" fn rc_game_energy_status(id: u64) -> i32 {
    let Ok(registry) = games().lock() else {
        return -2;
    };
    let Some(game) = registry.get(&id) else {
        return -2;
    };
    match (&game.dc15, &game.state.dc15) {
        (Some(a), Some(s)) => (s.charge / a.maximum as f32 * 1000.).round() as i32,
        _ => -1,
    }
}
/// Read-only defensive-jump counters and first four owner phases (0 idle,
/// 1 start, 2 flight, 3 land, 4 sleep). Short buffers remain untouched.
/// # Safety
/// `output` must be writable for `capacity` u32 words when non-null.
#[no_mangle]
pub unsafe extern "C" fn rc_game_jump_status(id: u64, output: *mut u32, capacity: usize) -> i32 {
    if output.is_null() || capacity < 12 {
        return -1;
    }
    let Ok(registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get(&id) else {
        return -1;
    };
    let mut words = [0u32; 12];
    if let Some(s) = game.state.combat.as_ref().and_then(|c| c.jump.as_ref()) {
        words = s.observation();
    }
    std::ptr::copy_nonoverlapping(words.as_ptr(), output, 12);
    12
}
#[no_mangle]
pub extern "C" fn rc_game_release(id: u64) {
    if let Ok(mut registry) = games().lock() {
        registry.remove(&id);
    }
}

/// Copy immutable world (kind 0) or current frame (kind 1, 640x400) graphics packet.
/// A null/zero output queries size. Short buffers are untouched and return -1.
/// # Safety
/// Non-null output points to capacity writable bytes and does not alias game data.
#[no_mangle]
pub unsafe extern "C" fn rc_game_graphics(
    id: u64,
    kind: u32,
    output: *mut u8,
    capacity: usize,
) -> isize {
    if kind > 1 || output.is_null() && capacity != 0 {
        return -1;
    }
    let Ok(mut registry) = games().lock() else {
        return -1;
    };
    let Some(session) = registry.get_mut(&id) else {
        return -1;
    };
    let slot = kind as usize;
    if session.graphics[slot].is_none() {
        let result = if kind == 0 {
            rc_render::gpu::world(&session.scene)
        } else {
            session
                .render_frame(640, 400)
                .map(|frame| frame.gpu_packet(&session.scene))
        };
        let Ok(packet) = result else {
            return -1;
        };
        session.graphics[slot] = Some(packet);
    }
    let packet = session.graphics[slot].as_ref().unwrap();
    if output.is_null() {
        return packet.len() as isize;
    }
    if capacity < packet.len() {
        return -1;
    }
    let size = packet.len();
    std::ptr::copy_nonoverlapping(packet.as_ptr(), output, size);
    // A successful copy releases the query receipt, avoiding an extra retained world/frame.
    session.graphics[slot] = None;
    size as isize
}

/// Begin opt-in capture on the calling thread; nested begin is rejected.
#[no_mangle]
pub extern "C" fn rc_profile_begin() -> i32 {
    performance::begin().map_or(-1, |()| 0)
}
/// Add a host timing to an active capture. Category 8 is JNI array work.
#[no_mangle]
pub extern "C" fn rc_profile_add(category: u32, ns: u64) {
    performance::add(category as usize, ns);
}
/// # Safety
/// output points to capacity writable u64 words. First ten are inclusive ns,
/// next ten call counts. Short buffers preserve the capture for a retry.
#[no_mangle]
pub unsafe extern "C" fn rc_profile_finish(output: *mut u64, capacity: usize) -> i32 {
    if output.is_null() || capacity < 20 {
        return -1;
    }
    let Ok(counters) = performance::finish() else {
        return -1;
    };
    std::ptr::copy_nonoverlapping(counters.ns.as_ptr(), output, 10);
    std::ptr::copy_nonoverlapping(counters.calls.as_ptr(), output.add(10), 10);
    20
}
/// Read-only observer used by opt-in profiling to prove input caused movement.
/// # Safety
/// output points to capacity writable f64 values: x/y/z/yaw/pitch/discarded_seconds.
#[no_mangle]
pub unsafe extern "C" fn rc_game_observe(id: u64, output: *mut f64, capacity: usize) -> i32 {
    if output.is_null() || capacity < 6 {
        return -1;
    }
    let Ok(registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get(&id) else {
        return -1;
    };
    let p = game.state.pawn.body.position;
    let values = [
        p[0],
        p[1],
        p[2],
        game.state.yaw,
        game.state.pitch,
        game.state.statistics.discarded_seconds,
    ];
    std::ptr::copy_nonoverlapping(values.as_ptr(), output, 6);
    6
}

/// Selected squad state. Short buffers are untouched.
/// # Safety
/// `output` must hold `capacity` writable u32 words when non-null.
#[no_mangle]
pub unsafe extern "C" fn rc_game_squad_status(id: u64, output: *mut u32, capacity: usize) -> i32 {
    if output.is_null() || capacity < 12 {
        return -1;
    }
    let Ok(registry) = games().lock() else {
        return -1;
    };
    let Some(game) = registry.get(&id) else {
        return -1;
    };
    let mut words = [0u32; 12];
    if let Some(s) = &game.state.squad {
        words = [
            s.health,
            s.shields.ceil() as u32,
            u32::from(s.joined),
            u32::from(s.down),
            u32::from(s.player_down),
            match s.order {
                rc_game::squad::Order::Follow => 0,
                rc_game::squad::Order::Hold => 1,
                rc_game::squad::Order::Focus(_) => 2,
            },
            (s.progress * 1000.) as u32,
            s.shots as u32,
            s.hits as u32,
            s.kills,
            s.player_revives,
            s.scorch_revives,
        ];
    }
    if game.state.squad.is_none() {
        if let Some(c) = game.state.map.as_ref().and_then(|s| s.companion.as_ref()) {
            words[0] = c.health;
            words[1] = c.shields.ceil() as u32;
            words[2] = u32::from(c.joined);
            words[3] = u32::from(c.down);
        }
    }
    std::ptr::copy_nonoverlapping(words.as_ptr(), output, 12);
    12
}

/// Full build/scene-bound checkpoint, query size with null output. Does not consume audio/input.
/// # Safety
/// out is null with zero capacity or points to capacity writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_game_checkpoint(id: u64, out: *mut u8, capacity: usize) -> isize {
    let Ok(registry) = games().lock() else {
        return -1;
    };
    let Some(session) = registry.get(&id) else {
        return -1;
    };
    let Ok(bytes) = session.checkpoint(&session.scene_identity) else {
        return -1;
    };
    if out.is_null() {
        return if capacity == 0 {
            bytes.len() as isize
        } else {
            -1
        };
    }
    if capacity < bytes.len() {
        return bytes.len() as isize;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
    bytes.len() as isize
}
/// Restore a checkpoint atomically into the matching scene; graphics include dynamic collisions.
/// # Safety
/// data points to len readable bytes; error points to capacity writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rc_game_restore_checkpoint(
    id: u64,
    data: *const u8,
    len: usize,
    error: *mut c_char,
    capacity: usize,
) -> i32 {
    let result = (|| -> Result<(), String> {
        if data.is_null() || len > rc_game::checkpoint::LIMIT {
            return Err("Invalid checkpoint payload".into());
        }
        let mut registry = games().lock().map_err(|_| "Game registry unavailable")?;
        let session = registry.get_mut(&id).ok_or("Invalid game handle")?;
        let identity = session.scene_identity.clone();
        session.restore_checkpoint(std::slice::from_raw_parts(data, len), &identity)?;
        session.graphics = [None, None];
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(message) => {
            if !error.is_null() && capacity > 0 {
                let n = message.len().min(capacity - 1);
                std::ptr::copy_nonoverlapping(message.as_ptr(), error.cast::<u8>(), n);
                *error.add(n) = 0;
            }
            -1
        }
    }
}
/// Major objective token for checkpoint scheduling; zero denotes an invalid session.
#[no_mangle]
pub extern "C" fn rc_game_checkpoint_marker(id: u64) -> u64 {
    games()
        .lock()
        .ok()
        .and_then(|r| r.get(&id).map(|s| s.checkpoint_marker()))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn campaign_map_identity_is_independent_of_status_text() {
        assert_eq!(campaign_map_id(None, true), 1);
        assert_eq!(campaign_map_id(None, false), 0);
        for (destination, id) in [
            ("GEO_01B.ctm#playerstart", 2),
            ("GEO_01C.ctm#playerstart", 3),
            ("GEO_03A.ctm#playerstart", 4),
        ] {
            assert_eq!(campaign_map_id(Some(destination), false), id);
        }
        assert_eq!(campaign_map_id(Some("unknown"), true), 0);
        assert_eq!(rc_game_campaign_map(0), -1);
    }
    #[test]
    fn profile_short_buffers_preserve_capture_and_observer_rejects_invalid_handle() {
        let mut output = [0xa5a5a5a5a5a5a5a5_u64; 22];
        assert_eq!(rc_profile_begin(), 0);
        assert_eq!(rc_profile_begin(), -1);
        rc_profile_add(8, 123);
        unsafe {
            assert_eq!(rc_profile_finish(output.as_mut_ptr(), 19), -1);
            assert_eq!(rc_profile_finish(output.as_mut_ptr(), 20), 20);
            assert_eq!(rc_profile_finish(output.as_mut_ptr(), 20), -1);
            let mut pose = [99.; 5];
            assert_eq!(rc_game_observe(u64::MAX, pose.as_mut_ptr(), 5), -1);
            assert_eq!(pose, [99.; 5]);
        }
        assert_eq!(output[8], 123);
        assert_eq!(output[18], 1);
        assert_eq!(output[20], 0xa5a5a5a5a5a5a5a5);
    }
    #[test]
    fn invalid_handles_buffers_flags_and_payload_are_rejected() {
        let mut words = [0xa5a5a5a5; 10];
        let mut error = [0_i8; 32];
        unsafe {
            assert_eq!(
                rc_game_create(std::ptr::null(), 5, error.as_mut_ptr(), error.len()),
                0
            );
            assert_ne!(error[0], 0);
            assert_eq!(rc_game_status(u64::MAX, words.as_mut_ptr(), 10), -1);
            assert_eq!(rc_game_status(0, words.as_mut_ptr(), 9), -1);
            assert_eq!(rc_game_draw(0, usize::MAX, 100, words.as_mut_ptr(), 10), -1);
        }
        assert_eq!(rc_game_advance(0, 0., 0., 0., 0., 0., 16), -1);
        assert_eq!(rc_game_command(0, 1), -1);
        assert_eq!(rc_game_energy_status(u64::MAX), -2);
        rc_game_release(0);
        assert_eq!(words, [0xa5a5a5a5; 10]);
    }
}
