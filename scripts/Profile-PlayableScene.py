"""Bind an input replay to SHA256; paired CPU profiles, debug check and negative probes."""
import hashlib
import json
import math
import os
import platform
import statistics
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/milestone0'
BUNDLE = ROOT / 'analysis/playable/geo_01a.rcplay'


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024*1024), b''):
            digest.update(block)
    return digest.hexdigest()


def execute(args, name, success=True):
    result = subprocess.run(list(map(str,args)), cwd=ROOT, capture_output=True, env={**os.environ,'CARGO_INCREMENTAL':'0'})
    (OUT / name).write_bytes(result.stdout + result.stderr)
    if success and result.returncode:
        raise RuntimeError((result.stdout + result.stderr).decode('utf-8','replace')[-4000:])
    if not success and result.returncode == 0:
        raise AssertionError(f'Invalid replay accepted: {name}')
    return result


def percentiles(values):
    values = sorted(values)
    assert values
    return {'n':len(values), 'p50_ms':statistics.median(values)/1e6,
            'p95_ms':values[math.ceil(len(values)*0.95)-1]/1e6,
            'p99_ms':values[math.ceil(len(values)*0.99)-1]/1e6}


def check_sha_binding(trace):
    if trace['bundle_sha256'] != sha(BUNDLE):
        raise ValueError('Replay SHA256 does not match local scene')


def main():
    OUT.mkdir(parents=True,exist_ok=True)
    execute(['cargo','build','--release','--locked','--offline','-p','rc-inspect','--bin','rc-play-replay'],'release-build.log')
    release = ROOT/'target/release/rc-play-replay.exe'
    trace_path = OUT/'replay.json'
    execute([release,'record',BUNDLE,trace_path,OUT/'record-profile.json'],'record.log')
    trace = json.loads(trace_path.read_text())
    trace['bundle_sha256'] = sha(BUNDLE)
    check_sha_binding(trace)
    trace_path.write_text(json.dumps(trace,indent=2),encoding='utf-8')
    execute([release,'verify',BUNDLE,trace_path,OUT/'cpu-profile.json'],'release-replay.log')
    execute(['cargo','run','--locked','--offline','-p','rc-inspect','--bin','rc-play-replay','--','check',BUNDLE,trace_path,OUT/'debug-check.json'],'debug-replay.log')
    negative = json.loads(json.dumps(trace)); negative['entries'][0]['expected_state'] = '0000000000000000'
    bad = OUT/'corrupt-replay.json'; bad.write_text(json.dumps(negative),encoding='utf-8')
    rejected = execute([release,'check',BUNDLE,bad,OUT/'unexpected.json'],'rejected-state.log',False)
    assert b'mismatch' in rejected.stderr
    negative = json.loads(json.dumps(trace)); negative['entries'][0]['action']['input']['hidden_position']=[0,0,0]
    bad.write_text(json.dumps(negative),encoding='utf-8')
    rejected = execute([release,'check',BUNDLE,bad,OUT/'unexpected.json'],'rejected-field.log',False)
    assert b'unknown field' in rejected.stderr
    negative = dict(trace); negative['bundle_sha256']='0'*64
    try:
        check_sha_binding(negative)
    except ValueError:
        pass
    else:
        raise AssertionError('SHA mismatch accepted')
    negative = dict(trace); negative['bundle_fingerprint']='0'*16
    bad.write_text(json.dumps(negative),encoding='utf-8')
    rejected = execute([release,'check',BUNDLE,bad,OUT/'unexpected.json'],'rejected-bundle.log',False)
    assert b'bundle mismatch' in rejected.stderr
    report = json.loads((OUT/'cpu-profile.json').read_text())
    debug = json.loads((OUT/'debug-check.json').read_text())
    assert all(run['exact_replay'] and run['final_state']==debug['final_state'] for run in report['runs'])
    enabled = [r for r in report['runs'] if r['profile_enabled']]
    disabled = [r for r in report['runs'] if not r['profile_enabled']]
    render = [s for r in enabled for s in r['samples'] if s['rendered']]
    times = [[sum(r['totals_ns'][i] for r in enabled),name] for i,name in enumerate(report['categories'])]
    totals = dict((name,total) for total,name in times)
    assert totals['simulation'] >= totals['collision'] > 0
    assert totals['scene_draw'] >= totals['raster'] + totals['frame_buffers'] > 0
    def render_cost(runs):
        return statistics.median(sum(s['render_and_pixel_check_ns'] for s in r['samples'])/r['rendered_frames'] for r in runs)
    old_manifest = json.loads((ROOT/'analysis/baselines/0.8.0-playable-scene/manifest.json').read_text())
    baseline = ROOT/'analysis/baselines/0.8.0-playable-scene'
    assert sha(baseline/'source.zip') == old_manifest['source_zip_sha256']
    assert all(sha(baseline/name)==expected for name,expected in old_manifest['artifacts'].items())
    summary = {'schema':1,'host':platform.platform(),'processor':platform.processor(),'abi':platform.machine(),
        'build':'Rust release','resolution':[640,400],'entries':len(trace['entries']),
        'exact_debug_release_replay':True,'profiling_preserves_states_and_reference_pixels':True,
        'baseline_archive_verified':True,'invalid_state_field_sha_and_bundle_rejected':True,
        'bundle_sha256':sha(BUNDLE),'replay_sha256':sha(trace_path),
        'load':percentiles([r['load_ns'] for r in report['runs']]),
        'simulation':percentiles([s['ns'][0] for r in enabled for s in r['samples'] if s['ticks']]),
        'scene_draw':percentiles([s['ns'][3] for s in render]),
        'raster':percentiles([s['ns'][4] for s in render]),
        'render_including_pixel_fingerprint':percentiles([s['render_and_pixel_check_ns'] for s in render]),
        'uninstrumented_render_including_pixel_fingerprint':percentiles([s['render_and_pixel_check_ns'] for r in disabled for s in r['samples'] if s['rendered']]),
        'paired_profile_overhead_ratio':render_cost(enabled)/render_cost(disabled),
        'collision_share_of_simulation':totals['collision']/totals['simulation'],
        'raster_share_of_scene_draw':totals['raster']/totals['scene_draw'],
        'dominant_render_stage':max(('raster',totals['raster']),('geometry_projection_and_other',totals['scene_draw']-totals['raster']-totals['frame_buffers']),key=lambda x:x[1])[0],
        'next_optimization':'Separate simulation/render data and implement a small GPU static-world raster path; retain CPU reference. Profile collision broadphase next if needed.',
        'scope':'Offline sampled CPU frames; not interactive FPS. Categories are inclusive. Per-triangle timers add measurable overhead. Android and physical-device results are separate.',
    }
    legacy=OUT/'legacy'; legacy.mkdir(exist_ok=True)
    execute(['cargo','run','--release','--locked','--offline','-p','rc-inspect','--bin','rc-play-check','--',BUNDLE,legacy],'legacy-check.log')
    old=json.loads((baseline/'analysis/playable/validation/validation.json').read_text())
    new=json.loads((legacy/'validation.json').read_text())
    old.pop('render_seconds_640x400');new.pop('render_seconds_640x400')
    assert old==new,'Legacy gameplay state differs from 0.8 reference'
    receipts=json.loads((baseline/'analysis/playable/validation-manifest.json').read_text())
    assert all(sha(legacy/name)==expected for name,expected in receipts['frame_sha256'].items())
    summary['legacy_states_and_five_frames_match_0_8']=True
    (OUT/'cpu-summary.json').write_text(json.dumps(summary,indent=2),encoding='utf-8')
    print(json.dumps(summary,indent=2))


if __name__=='__main__':
    main()
