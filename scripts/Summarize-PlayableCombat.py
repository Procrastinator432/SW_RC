"""Bind the first M3 integration to its actual sources, APK, local assets and validation receipts."""
import hashlib
import json
import re
import shutil
import struct
import zipfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT/'analysis/milestone3'
GAME = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')

def read(path):
    return json.loads((OUT/path).read_text(encoding='utf-8-sig'))

def digest(path):
    return {'bytes': path.stat().st_size, 'sha256': hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()}

def truth(record, fields):
    for field in fields:
        assert record[field] is True, field

combat = read('check/combat-validation.json')
truth(combat, ['delayed_hit','cadence_120_30_10_exact','all_three_enemies_15_hits',
               'reload_conserves_ammo','sound_events_once','enemy_bodyfalls_per_instance',
               'reset_exact','invalid_input_transaction','render_read_only','truncated_bundle_rejected'])
assert combat['won']['phase'] == 'Won' and combat['lost']['phase'] == 'Lost'
assert read('legacy-replay.json')['exact_replay'] is True
m2 = read('m2-regression/animation-validation.json')
truth(m2, ['corrupt_mapping_rejected','death_holds_last_frame','fixed_tick_visual_cadence_exact',
           'instance_isolation','invalid_tick_rollback','pause_render_read_only','restart_exact','weapon_camera_attachment'])
truth(read('abi-validation.json'), ['short_buffers_and_canaries','invalid_input_rollback','pause_clears_pending_jump','jump','restart_exact','released_handle_rejected','read_only_pose_and_real_movement'])
truth(read('audio-abi-validation.json'), ['query_and_short_copy_preserve_events',
      'successful_copy_consumes_once','reset_restarts_sequence','suspend_discards_stale_events','copy_canaries_intact'])
for path in ['android/validation.json','android-cpu/validation.json']:
    truth(read(path), ['normal_picker_load','touch_aim_and_fire','three_kills','touch_move_and_exit_victory',
          'touch_jump','enemy_damage_and_defeat','touch_reload','restart','pause_frames_exact','invalid_file_and_recovery','no_crash'])
graphics = read('graphics/validation.json')
truth(graphics, ['actual_context_recreated','context_rebuild_pixels_exact','context_rebuild_pose_exact',
      'backend_switch_preserves_state','short_reload_memory_growth_bounded','no_crash'])
truth(read('input/validation.json'), ['final_look_release_exact','simultaneous_move_look_fire',
      'simultaneous_jump_fire','action_cancel_clears_inputs','focus_loss_stops_simulation',
      'resume_preserves_state_and_clears_inputs','picker_cancel_recovers','no_crash'])
audio = {}
for backend, path in [('gpu','android/audio-host.log'),('cpu','android-cpu/audio-host.log')]:
    log = (OUT/path).read_text(encoding='utf-8-sig')
    assert 'bank sounds=4' in log and log.count('loaded sample=') >= 4
    assert 'failed' not in log.lower()
    events = [tuple(map(int,m)) for m in re.findall(r'event sequence=(\d+) tick=(\d+) sound=(\d+) owner=(-?\d+)',log)]
    assert {e[2] for e in events} == {0,1,2,3}
    audio[backend] = {'sample_load_confirmations': log.count('loaded sample='), 'successful_play_calls': len(events),
                      'sound_indices': sorted({e[2] for e in events}), 'auditory_test': False}

test_log=(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig')
assert not re.search(r'test result: FAILED|error:',test_log)
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',test_log)))
assert tests == 942, tests
badging=(OUT/'apk-badging.log').read_text(encoding='utf-8-sig')
assert "versionCode='13'" in badging and "'arm64-v8a'" in badging and "'x86_64'" in badging
assert 'Verifies' in (OUT/'apksigner.log').read_text(encoding='utf-8-sig')
assert 'Verification succesful' in (OUT/'zipalign.log').read_text(encoding='utf-8-sig')
assert 'BUILD SUCCESSFUL' in (OUT/'android-build.log').read_text(encoding='utf-8-sig')
assert not (OUT/'format.log').read_text(encoding='utf-8-sig').strip()
assert not (OUT/'diff-check.log').read_text(encoding='utf-8-sig').strip()
elf=[]
for abi in ['arm64-v8a','x86_64']:
    for name in ['librc_native.so','librc_bridge.so']:
        path=ROOT/'android/app/src/main/jniLibs'/abi/name
        b=path.read_bytes(); assert b[:6] == b'\x7fELF\x02\x01'
        offset=struct.unpack_from('<Q',b,32)[0];size,count=struct.unpack_from('<HH',b,54)
        alignments=[]
        for i in range(count):
            at=offset+i*size
            if struct.unpack_from('<I',b,at)[0] == 1:
                file_offset,virtual=struct.unpack_from('<QQ',b,at+8)
                align=struct.unpack_from('<Q',b,at+48)[0]
                assert align >= 16384 and file_offset%16384 == virtual%16384
                alignments.append(align)
        assert alignments
        elf.append({'path':str(path.relative_to(ROOT)), 'load_alignments':alignments, **digest(path)})

sources=set(ROOT.glob('crates/**/*.rs')) | set(ROOT.glob('crates/**/Cargo.toml'))
sources |= {ROOT/'Cargo.toml', ROOT/'Cargo.lock', ROOT/'README.md'}
sources |= set(ROOT.glob('docs/*.md')) | set(ROOT.glob('scripts/*.*'))
for pattern in ['android/native/*.c','android/app/src/main/java/**/*.java','android/app/src/main/AndroidManifest.xml',
                'android/*.gradle','android/app/*.gradle','android/gradle.properties']:
    sources |= set(ROOT.glob(pattern))
source_hashes={str(p.relative_to(ROOT)):digest(p) for p in sorted(sources) if p.is_file()}
shutil.copyfile(ROOT/'android/app/build/outputs/apk/debug/app-debug.apk',OUT/'republic-commando-0.11.0.apk')
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
    for p in sorted(sources):
        if p.is_file(): z.write(p,str(p.relative_to(ROOT)))
originals={str(p.relative_to(GAME)):digest(p) for p in [GAME/'Maps/geo_01a.ctm',GAME/'Animations/hudarms.ukx',
           GAME/'Animations/geonosian.ukx',GAME/'Sounds/gen_sound.uax',GAME/'System/engine.u',
           GAME/'System/ctinventory.u',GAME/'System/ctcharacters.u',GAME/'System/ctgame.u',GAME/'Properties/properties.u']}
artifacts={str(p.relative_to(ROOT)):digest(p) for p in sorted(OUT.rglob('*'))
           if p.is_file() and p.name!='validation-manifest.json'}
manifest={'created_utc':datetime.now(timezone.utc).isoformat(), 'version':'0.11.0-combat-scene', 'version_code':13,
          'milestone':'M3 first integration; native AI/projectile/damage parity still pending',
          'workspace_tests':tests,'audio_host':audio,'elf':elf,
          'graphics_comparison':{k:v for k,v in graphics.items() if k.startswith('cpu_gpu')},
          'source_files':source_hashes,'original_packages':originals,'artifacts':artifacts,
          'm2_base_scene':digest(ROOT/'analysis/milestone2/geo_01a-animated.rcplay'),
          'physical_m2_function_confirmed_by_user':True,'physical_m3_test':False,
          'limits':['No native AI or complete combat parity','No subjective listening or latency measurement',
                    'No quantitative ARM64 duration/performance acceptance','Debug APK, x86_64 API35 emulator validation']}
(OUT/'validation-manifest.json').write_text(json.dumps(manifest,indent=2,ensure_ascii=False),encoding='utf-8')
print(json.dumps({'tests':tests,'sources':len(source_hashes),'artifacts':len(artifacts),
                  'scene':digest(OUT/'geo_01a-combat.rcplay'),'apk':digest(OUT/'republic-commando-0.11.0.apk'),
                  'audio_host':audio,'graphics':manifest['graphics_comparison']},ensure_ascii=False))
