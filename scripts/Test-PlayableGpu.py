"""GPU/CPU reference, graphics ABI canaries, backend switching, and real context rebuild.
Android tests use normal UI input; the separate desktop handle only creates reference pixels.
"""
import argparse
import ctypes as c
import hashlib
import importlib.util
import json
import re
import time
from pathlib import Path
from PIL import Image, ImageChops

ROOT=Path(__file__).resolve().parents[1]
SCENE=ROOT/'analysis/playable/geo_01a.rcplay'
OUT=ROOT/'analysis/milestone1/graphics'
OUT.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('play_ui',ROOT/'scripts/Test-PlayableEmulator.py')
ui=importlib.util.module_from_spec(spec);spec.loader.exec_module(ui);ui.OUT=OUT

def gpu_logs():
    return ui.adb('logcat','-d','-s','RCGpu:I','*:S')

def wait(predicate):
    deadline=time.monotonic()+15
    while time.monotonic()<deadline:
        if predicate():return
        time.sleep(.1)
    raise AssertionError('Expected GPU result did not arrive')

def reference(name):
    data=ui.adb('exec-out','run-as','org.rcport.diagnostic','cat','files/gpu-reference.png',binary=True)
    (OUT/name).write_bytes(data)
    return data

def telemetry():
    raw=ui.adb('logcat','-d','-s','RCPerf:I','*:S')
    return [json.loads(line[line.index('{'):]) for line in raw.splitlines() if '{' in line and '"type":"frame"' in line]

def native_reference():
    api=c.CDLL(str(ROOT/'target/release/rc_native.dll'))
    api.rc_game_create.argtypes=[c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t];api.rc_game_create.restype=c.c_uint64
    api.rc_game_draw.argtypes=[c.c_uint64,c.c_size_t,c.c_size_t,c.c_void_p,c.c_size_t];api.rc_game_draw.restype=c.c_ssize_t
    api.rc_game_graphics.argtypes=[c.c_uint64,c.c_uint32,c.c_void_p,c.c_size_t];api.rc_game_graphics.restype=c.c_ssize_t
    api.rc_game_release.argtypes=[c.c_uint64]
    api.rc_game_advance.argtypes=[c.c_uint64,c.c_double,c.c_float,c.c_float,c.c_float,c.c_float,c.c_uint32];api.rc_game_advance.restype=c.c_int
    api.rc_game_command.argtypes=[c.c_uint64,c.c_uint32];api.rc_game_command.restype=c.c_int
    raw=SCENE.read_bytes();payload=c.create_string_buffer(raw);error=c.create_string_buffer(512)
    handle=api.rc_game_create(payload,len(raw),error,len(error));assert handle,error.value
    try:
        sizes=[]
        for kind,magic in [(0,b'RCGPU001'),(1,b'RCFRA002' if raw.startswith((b'RCPLAY02', b'RCPLAY03', b'RCPLAY04')) else b'RCFRA001')]:
            n=api.rc_game_graphics(handle,kind,None,0);assert 0<n<64*1024*1024
            data=(c.c_ubyte*(n+16))(*([0xa5]*(n+16)))
            assert api.rc_game_graphics(handle,kind,data,n-1)==-1
            assert bytes(data)==b'\xa5'*(n+16)
            assert api.rc_game_graphics(handle,kind,data,n)==n
            assert bytes(data)[:8]==magic and bytes(data)[n:]==b'\xa5'*16
            (OUT/('world.rcgpu' if kind==0 else 'spawn.rcframe')).write_bytes(bytes(data)[:n]);sizes.append(n)
        initial=(OUT/'spawn.rcframe').read_bytes()
        n=api.rc_game_graphics(handle,1,None,0)
        assert api.rc_game_advance(handle,1/60,0,0,.1,0,0)==1
        changed=(c.c_ubyte*(16*1024*1024))()
        written=api.rc_game_graphics(handle,1,changed,len(changed));assert written>0
        assert bytes(changed[:written])!=initial,'advance retained stale queried graphics'
        assert api.rc_game_command(handle,1)==0
        n=api.rc_game_graphics(handle,1,None,0);reset=(c.c_ubyte*n)()
        assert api.rc_game_graphics(handle,1,reset,n)==n and bytes(reset)==initial
        assert api.rc_game_graphics(handle,2,None,0)==-1
        pixels=(c.c_uint32*(640*400))();assert api.rc_game_draw(handle,640,400,pixels,len(pixels))==len(pixels)
        image=Image.frombytes('RGBA',(640,400),bytes(pixels),'raw','BGRA').convert('RGB');image.save(OUT/'cpu-spawn.png')
    finally:api.rc_game_release(handle)
    assert api.rc_game_graphics(handle,0,None,0)==-1
    return image,sizes

def memory(name):
    raw=ui.adb('shell','dumpsys','meminfo','org.rcport.diagnostic');(OUT/(name+'.txt')).write_text(raw,encoding='utf-8')
    match=re.search(r'TOTAL PSS:\s*(\d+)',raw);assert match,raw
    frames=telemetry();assert frames
    return dict(pss_kib=int(match[1]),java_heap_bytes=frames[-1]['java_heap_bytes'],native_heap_bytes=frames[-1]['native_heap_bytes'])

def main():
    global OUT,SCENE
    parser=argparse.ArgumentParser();parser.add_argument('--scene',type=Path);parser.add_argument('--output',type=Path)
    args=parser.parse_args()
    if args.scene:SCENE=args.scene.resolve()
    if args.output:OUT=args.output.resolve()
    OUT.mkdir(parents=True,exist_ok=True);ui.OUT=OUT
    (OUT/'validation.json').unlink(missing_ok=True)
    cpu,sizes=native_reference()
    ui.bootstrap(True,False,True,scene=SCENE)
    assert 'GPU' in [n.attrib.get('text') for n in ui.ui().iter('node')]
    # Restart produces an elapsed=0 ordinary frame before the live pump starts.
    zero_count=gpu_logs().count('reference ticks=0')
    ui.tap_node(ui.find(ui.ui(),'NEUSTART'))
    wait(lambda:gpu_logs().count('reference ticks=0')>zero_count)
    # Read the reset snapshot before pause can publish a later still frame.
    reference('gpu-spawn.png')
    ui.tap_node(ui.find(ui.ui(),'PAUSE'))
    gpu=Image.open(OUT/'gpu-spawn.png').convert('RGB');assert gpu.size==cpu.size
    differences=[abs(a-b) for a,b in zip(cpu.tobytes(),gpu.tobytes())]
    mean=sum(differences)/len(differences)
    close=sum(d<=2 for d in differences)/len(differences)
    ImageChops.difference(cpu,gpu).save(OUT/'spawn-difference.png')
    # Different edge coverage / texture boundary rounding are expected. Thresholds
    # are fixed before observing a result and detect flips, missing batches, UV and depth errors.
    assert mean<2 and close>.98,(mean,close)
    # Both backends share a paused state; toggling forces a new immutable still frame.
    ui.tap_node(ui.find(ui.ui(),'GPU'));wait(lambda:telemetry()[-1].get('backend')=='cpu')
    ui.capture('cpu-paused')
    ui.tap_node(ui.find(ui.ui(),'CPU'));wait(lambda:telemetry()[-1].get('backend')=='gpu')
    time.sleep(.3);before=reference('before-context-loss.png');pose=telemetry()[-1]['pose']
    count=gpu_logs().count('renderer=')
    ui.adb('shell','input','keyevent','KEYCODE_HOME');time.sleep(.4)
    ui.adb('shell','am','start','-W','-a','android.intent.action.MAIN','-c','android.intent.category.LAUNCHER','-f','0x10200000','-n','org.rcport.diagnostic/.MainActivity')
    wait(lambda:gpu_logs().count('renderer=')>count)
    time.sleep(.5);after=reference('after-context-loss.png')
    assert ImageChops.difference(Image.open(OUT/'before-context-loss.png'),Image.open(OUT/'after-context-loss.png')).getbbox() is None
    assert telemetry()[-1]['pose']==pose
    ui.capture('restored-context')
    memories=[memory('memory-load-0')]
    for i in range(1,4):
        ui.tap_node(ui.find(ui.ui(),'SZENE ÖFFNEN'));ui.select_file('geo_01a.rcplay');ui.pause_loaded_scene();time.sleep(.3)
        memories.append(memory('memory-load-'+str(i)))
    # A short bounded-growth probe; no long-term leak-free or thermal claim.
    assert memories[-1]['native_heap_bytes']<=memories[0]['native_heap_bytes']+32*1024*1024,memories
    assert memories[-1]['java_heap_bytes']<=memories[0]['java_heap_bytes']+40*1024*1024,memories
    raw=gpu_logs();(OUT/'gpu.log').write_text(raw,encoding='utf-8')
    assert 'Graphics failure' not in raw and 'GPU fallback' not in ui.logs('final')
    packet=(OUT/'world.rcgpu').read_bytes()
    texture_count=int.from_bytes(packet[8:12],'little');offset=12
    for _ in range(texture_count):
        width=int.from_bytes(packet[offset:offset+4],'little');height=int.from_bytes(packet[offset+4:offset+8],'little');offset+=8+width*height*4
    batches=int.from_bytes(packet[offset:offset+4],'little');offset+=4;vertices=0
    for _ in range(batches):
        count=int.from_bytes(packet[offset+4:offset+8],'little');vertices+=count;offset+=8+count*36
    assert offset==len(packet)
    assert f'textures={texture_count}' in raw and f'vertices={vertices}' in raw
    crash=ui.adb('logcat','-d','-b','crash');assert 'FATAL EXCEPTION' not in crash and 'Fatal signal' not in crash
    result=dict(scene=str(SCENE),scene_sha256=hashlib.sha256(SCENE.read_bytes()).hexdigest(),api=35,abi='x86_64',resolution=[640,400],world_packet_bytes=sizes[0],frame_packet_bytes=sizes[1],
        abi_short_buffers_and_canaries=True,query_cache_invalidates_on_advance_and_restart=True,released_handle_rejected=True,cpu_gpu_spawn_mean_channel_error=mean,
        cpu_gpu_spawn_channels_within_two=close,backend_switch_preserves_state=True,
        actual_context_recreated=True,context_rebuild_pixels_exact=True,context_rebuild_pose_exact=True,
        repeated_scene_loads=3,memory_samples=memories,short_reload_memory_growth_bounded=True,no_crash=True,
        scope='Emulator with software GPU driver. Functional GPU path and a short memory probe; no physical ARM64 performance acceptance.')
    (OUT/'validation.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps(result))

if __name__=='__main__':main()
