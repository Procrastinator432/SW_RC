"""Exercise the exported native ABI with the actual first-scene bundle on Windows."""
import argparse
import ctypes as c
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--output',type=Path,default=root/'analysis/playable/abi-validation.json')
parser.add_argument('--scene',type=Path)
parser.add_argument('--magazine',type=int,default=20)
parser.add_argument('--reserve',type=int,default=200)
args=parser.parse_args()
api = c.CDLL(str(root / 'target/release/rc_native.dll'))
u32p = c.POINTER(c.c_uint32)
api.rc_game_create.argtypes = [c.c_void_p, c.c_size_t, c.c_void_p, c.c_size_t]
api.rc_game_create.restype = c.c_uint64
api.rc_game_advance.argtypes = [c.c_uint64, c.c_double, c.c_float, c.c_float, c.c_float, c.c_float, c.c_uint32]
api.rc_game_advance.restype = c.c_int32
api.rc_game_draw.argtypes = [c.c_uint64, c.c_size_t, c.c_size_t, u32p, c.c_size_t]
api.rc_game_draw.restype = c.c_ssize_t
api.rc_game_status.argtypes = [c.c_uint64, u32p, c.c_size_t]
api.rc_game_status.restype = c.c_int32
api.rc_game_command.argtypes = [c.c_uint64, c.c_uint32]
api.rc_game_command.restype = c.c_int32
api.rc_game_release.argtypes = [c.c_uint64]
api.rc_game_release.restype = None
api.rc_profile_begin.argtypes=[]
api.rc_profile_begin.restype=c.c_int32
api.rc_profile_finish.argtypes=[c.POINTER(c.c_uint64),c.c_size_t]
api.rc_profile_finish.restype=c.c_int32
api.rc_game_observe.argtypes=[c.c_uint64,c.POINTER(c.c_double),c.c_size_t]
api.rc_game_observe.restype=c.c_int32
api.rc_game_jump_status.argtypes=[c.c_uint64,u32p,c.c_size_t]
api.rc_game_jump_status.restype=c.c_int32

data = (args.scene or root / 'analysis/playable/geo_01a.rcplay').read_bytes()
buffer = c.create_string_buffer(data)
error = c.create_string_buffer(512)
handle = api.rc_game_create(buffer, len(data), error, len(error))
assert handle, error.value
def status():
    words = (c.c_uint32 * 12)(*([0xA5A5A5A5] * 12))
    assert api.rc_game_status(handle, words, 9) == -1
    assert list(words) == [0xA5A5A5A5] * 12
    assert api.rc_game_status(handle, words, 10) == 10
    assert list(words)[10:] == [0xA5A5A5A5] * 2
    return list(words)[:10]
def frame():
    pixels = (c.c_uint32 * (320 * 200 + 2))()
    pixels[320 * 200] = pixels[320 * 200 + 1] = 0xA5A5A5A5
    assert api.rc_game_draw(handle, 320, 200, pixels, 1) == -1
    assert api.rc_game_draw(handle, 320, 200, pixels, 320 * 200) == 320 * 200
    assert list(pixels)[320 * 200:] == [0xA5A5A5A5] * 2
    return hashlib.sha256(bytes(pixels)[:320 * 200 * 4]).hexdigest()
try:
    probe=(c.c_uint32*14)(*([0xA5A5A5A5]*14))
    assert api.rc_game_jump_status(handle,probe,11)==-1 and list(probe)==[0xA5A5A5A5]*14
    assert api.rc_game_jump_status(handle,None,12)==-1
    assert api.rc_game_jump_status(0,probe,12)==-1 and list(probe)==[0xA5A5A5A5]*14
    assert api.rc_game_jump_status(handle,probe,12)==12
    assert list(probe)[:12]==[0]*12 and list(probe)[12:]==[0xA5A5A5A5]*2
    initial, first_frame = status(), frame()
    assert initial[:5] == [100, args.magazine, args.reserve, 3, 0]
    assert api.rc_game_advance(handle, float('nan'), 0, 0, 0, 0, 0) == -1
    assert api.rc_game_advance(handle, 1, 0, 0, 0, 0, 0) == -1
    assert status() == initial
    assert api.rc_game_advance(handle, 1 / 120, 0, 0, 0, 0, 1) == 0
    assert api.rc_game_command(handle, 0) == 0
    assert api.rc_game_advance(handle, 1 / 60, 0, 0, 0, 0, 0) == 1
    assert status()[8] == 0
    assert api.rc_game_advance(handle, 1 / 60, 0, 0, 0, 0, 1) == 1
    assert status()[8] == 1
    assert api.rc_game_command(handle, 1) == 0
    assert status() == initial and frame() == first_frame
    assert api.rc_game_command(handle, 2) == -1
    assert api.rc_profile_begin()==0 and api.rc_profile_begin()==-1
    pose=(c.c_double*8)(*([99.]*8))
    assert api.rc_game_observe(handle,pose,5)==-1 and list(pose)==[99.]*8
    assert api.rc_game_observe(handle,pose,6)==6
    initial_pose=list(pose)[:6]
    assert list(pose)[6:]==[99.]*2 and initial_pose[-1]==0
    assert frame()==first_frame
    assert api.rc_game_advance(handle,1/60,1,0,0,0,0)==1
    assert api.rc_game_observe(handle,pose,6)==6 and list(pose)[:3]!=initial_pose[:3]
    timings=(c.c_uint64*22)(*([0xA5A5A5A5A5A5A5A5]*22))
    assert api.rc_profile_finish(timings,19)==-1
    assert list(timings)==[0xA5A5A5A5A5A5A5A5]*22
    assert api.rc_profile_finish(timings,20)==20
    assert list(timings)[20:]==[0xA5A5A5A5A5A5A5A5]*2
    assert timings[0]>=timings[1]>0 and timings[3]>0 and timings[7]>0
    assert timings[14]==0  # coarse capture intentionally has no per-triangle timers
    assert api.rc_profile_finish(timings,20)==-1
    assert api.rc_game_command(handle,1)==0
    assert status()==initial and frame()==first_frame
    api.rc_game_release(handle)
    assert api.rc_game_observe(handle,pose,6)==-1
    assert api.rc_game_advance(handle, 0, 0, 0, 0, 0, 0) == -1
    assert api.rc_game_command(handle, 1) == -1
    result = dict(initial_status=initial, initial_argb_sha256=first_frame,
                  short_buffers_and_canaries=True, invalid_input_rollback=True,
                  pause_clears_pending_jump=True, jump=True, restart_exact=True,
                  released_handle_rejected=True,
                  profile_frame_and_restart_exact=True, profile_short_buffers_and_canaries=True,
                  read_only_pose_and_real_movement=True, coarse_capture_omits_triangle_timers=True,
                  timings_ns=list(timings)[:10])
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))
finally:
    api.rc_game_release(handle)
