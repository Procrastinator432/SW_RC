//! Original intro lifecycle fixture driven by live opening events and native LOD end callbacks.
//! Placement and MoveToGoal execution remain explicit pending host operations.
use rc_game::{
    intro_flow::{Flow, State, WarriorPhase},
    Input, STEP,
};
use rc_package::{
    animation_call::PlayAnimParameters,
    channel_playback::{
        play_channel, ChannelPlaybackHost, ChannelPlaybackSnapshot, PlaybackSequence,
    },
    event_lookup::EventNameSnapshot,
    mesh_animation::AnimationChannel,
    skeletal_animation_tick::{TickEndHost, TickNotify, TickNotifyHost},
    skeletal_lod_tick::{update_lod_animation, LodTickHost, TickSequence},
    skeletal_sequence::SequenceLookupHost,
};
use std::{env, fs, path::Path};
struct Start;
impl ChannelPlaybackHost for Start {
    fn random_int(&mut self) -> Result<i32, String> {
        Err("Unexpected randomized intro start".into())
    }
    fn post_init(&mut self, _: &mut AnimationChannel) -> Result<(), String> {
        Ok(())
    }
    fn replicate(&mut self, _: usize, _: &AnimationChannel) -> Result<(), String> {
        Ok(())
    }
}
struct Tick {
    identity: u32,
    sequence: TickSequence,
    ended: bool,
    events: Vec<(u64, u32)>,
    tick: u64,
}
impl SequenceLookupHost for Tick {
    fn find_sequence(&mut self, id: u32, _: bool) -> Result<u32, String> {
        Ok(id)
    }
}
impl TickNotifyHost for Tick {
    fn notify(&mut self, id: u32, _: &mut Vec<AnimationChannel>) -> Result<(), String> {
        self.events.push((self.tick, id));
        Ok(())
    }
}
impl TickEndHost for Tick {
    fn clear(&mut self, _: [u32; 8], _: &mut AnimationChannel) -> Result<(), String> {
        Err("Unexpected clear callback".into())
    }
    fn anim_end(&mut self, _: i32, _: &mut AnimationChannel) -> Result<(), String> {
        self.ended = true;
        Ok(())
    }
}
impl LodTickHost for Tick {
    fn set_locked(&mut self, _: bool) -> Result<(), String> {
        Ok(())
    }
    fn flags(&mut self) -> Result<u32, String> {
        Ok(0)
    }
    fn request_destroy(&mut self) -> Result<(), String> {
        Err("Unexpected mesh destruction".into())
    }
    fn sequence(&mut self, id: u32) -> Result<TickSequence, String> {
        if id != self.identity {
            return Err("Wrong sequence".into());
        }
        Ok(self.sequence.clone())
    }
    fn random_int(&mut self) -> Result<i32, String> {
        Err("Unexpected jitter".into())
    }
    fn actor_has_mesh(&mut self) -> Result<bool, String> {
        Ok(true)
    }
    fn replicate(&mut self, _: usize, _: &mut Vec<AnimationChannel>) -> Result<(), String> {
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-intro-flow-check ASSET_DIR SCENE".into());
    }
    let out = Path::new(&args[1]);
    let a: Flow = serde_json::from_slice(&fs::read(out.join("flow.json"))?)?;
    let mut flow = State::default();
    flow.begin(&a)?;
    let mut game = rc_game::bundle::decode(&fs::read(&args[2])?)?;
    let mesh: rc_game::visual::Mesh =
        serde_json::from_slice(&fs::read(out.join("actor-530-mesh.json"))?)?;
    let p = mesh.prepare_reference()?;
    let c = p.clip("WarriorIntro")?;
    let seq = &p.mesh.clips[c].sequence;
    let identity = c as u32 + 1;
    let mut tick = Tick {
        identity,
        sequence: TickSequence {
            frames: seq.frames,
            rate: f32::from_bits(seq.rate_bits),
            jitter_amplitude: f32::from_bits(seq.word_18),
            notifies: seq
                .notifies
                .iter()
                .map(|n| TickNotify {
                    time_bits: n.time_bits,
                    object: n.object_index as u32,
                })
                .collect(),
        },
        ended: false,
        events: vec![],
        tick: 0,
    };
    let mut playback: Option<ChannelPlaybackSnapshot> = None;
    let mut cursor = 0;
    let mut ended_tick = None;
    let mut rappel_tick = None;
    for _ in 0..2400 {
        game.advance(STEP, Input::default())?;
        flow.tick(&a, STEP)?;
        let events = &game.state.opening.as_ref().unwrap().events;
        for (n, _, event) in &events[cursor..] {
            if event == "Rappel" {
                flow.signal(&a, event)?;
                rappel_tick = Some(*n);
                let mut channel = p.channel(c, 0.)?;
                channel.words[0] = 999;
                let mut state = ChannelPlaybackSnapshot {
                    channels: vec![channel],
                    byte_60: 1,
                    byte_61: 0,
                };
                play_channel(
                    &mut state,
                    0,
                    &PlayAnimParameters {
                        sequence: EventNameSnapshot {
                            handle: identity,
                            resolved_index: identity as i32,
                        },
                        looping: false,
                        bone: EventNameSnapshot {
                            handle: 0,
                            resolved_index: 0,
                        },
                        channel: 0,
                        float_10: 1.,
                        rate: 1.,
                        start_frame: -1.,
                    },
                    Some(PlaybackSequence {
                        token: identity,
                        frames: seq.frames,
                        rate: f32::from_bits(seq.rate_bits),
                        minimum_blend: f32::from_bits(seq.minimum_blend_bits),
                        randomize_start: seq.randomize_start != 0,
                    }),
                    &mut Start,
                )?;
                playback = Some(state);
            }
            if event == "GunshipWarrior" {
                flow.signal(&a, event)?;
            }
        }
        cursor = events.len();
        if let Some(state) = &mut playback {
            tick.tick = game.state.statistics.ticks;
            tick.ended = false;
            update_lod_animation(
                &mut state.channels,
                &mut state.byte_61,
                STEP as f32,
                false,
                &mut tick,
            )?;
            if tick.ended {
                assert!(ended_tick.is_none());
                ended_tick = Some(tick.tick);
                flow.animation_ended(&a, 530)?;
            }
        }
        if game.state.opening.as_ref().unwrap().done {
            break;
        }
    }
    assert!(flow.trooper_destroyed);
    assert_eq!(flow.warrior, WarriorPhase::Moving);
    assert_eq!(tick.events.len(), 4);
    for id in 190..=193 {
        assert_eq!(tick.events.iter().filter(|(_, n)| *n == id).count(), 1);
    }
    let destroy = flow
        .events
        .iter()
        .find(|e| e.original == 1262)
        .unwrap()
        .seconds;
    let start = flow
        .events
        .iter()
        .find(|e| e.original == 1259)
        .unwrap()
        .seconds;
    assert!((destroy - start - a.first_voice_seconds - a.destroy_delay).abs() < 1e-9);
    assert!(destroy < ended_tick.unwrap() as f64 * STEP);
    assert!(flow.events.iter().any(|e| e.original == 1771));
    assert!(!flow.events.iter().any(|e| e.original == 1772));
    fs::write(
        out.join("flow-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"scope":"original-script lifecycle fixture; live opening signals and native Warrior LOD callback; no NPC gameplay attachment or executed teleport/movement","rappel_tick":rappel_tick,"warrior_anim_end_tick":ended_tick,"warrior_notify_events":tick.events,"trooper_destroy_seconds":destroy,"state":flow,"teleport_and_movement_execution_pending":true}),
        )?,
    )?;
    println!("Intro flow passed: Rappel {:?}, native Warrior AnimEnd {:?}, trooper destroyed at {destroy:.6}s; placement/movement hosts pending",rappel_tick,ended_tick);
    Ok(())
}
