#include <jni.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <time.h>
#include <string.h>
extern intptr_t rc_package_summary(const uint8_t *, size_t, char *, size_t);
extern uint64_t rc_scene_create(const uint8_t *, size_t, char *, size_t);
extern intptr_t rc_scene_draw_view(uint64_t, size_t, size_t, float, float, float, float, float, float, uint32_t, uint32_t *, size_t);
extern void rc_scene_release(uint64_t);
extern intptr_t rc_scene_start(uint64_t, size_t, float *, size_t);
extern int32_t rc_scene_bsp_query(uint64_t, float, float, float, float, float);
extern uint64_t rc_game_create(const uint8_t *, size_t, char *, size_t);
extern int32_t rc_game_advance(uint64_t, double, float, float, float, float, uint32_t);
extern int32_t rc_game_command(uint64_t, uint32_t);
extern intptr_t rc_game_draw(uint64_t, size_t, size_t, uint32_t *, size_t);
extern int32_t rc_game_status(uint64_t, uint32_t *, size_t);
extern int32_t rc_game_campaign_map(uint64_t);
extern void rc_game_release(uint64_t);
extern int32_t rc_profile_begin(void);
extern void rc_profile_add(uint32_t,uint64_t);
extern int32_t rc_profile_finish(uint64_t *,size_t);
extern int32_t rc_game_observe(uint64_t,double *,size_t);
extern intptr_t rc_game_graphics(uint64_t,uint32_t,uint8_t *,size_t);
extern intptr_t rc_game_audio(uint64_t,uint32_t,uint8_t *,size_t);
static void throw_io(JNIEnv *env, const char *message);
JNIEXPORT jbyteArray JNICALL Java_org_rcport_diagnostic_PlayActivity_gameAudio(JNIEnv *env,jclass klass,jlong id,jint kind) {
    (void)klass;
    intptr_t size=rc_game_audio((uint64_t)id,(uint32_t)kind,NULL,0);
    if(size<12 || size>8*1024*1024+1024) {throw_io(env,"Invalid sound packet");return NULL;}
    jbyteArray result=(*env)->NewByteArray(env,(jsize)size);
    if(!result) return NULL;
    uint8_t *bytes=malloc((size_t)size);
    if(!bytes) {throw_io(env,"Sound allocation failed");return NULL;}
    int valid=rc_game_audio((uint64_t)id,(uint32_t)kind,bytes,(size_t)size)==size;
    if(valid) (*env)->SetByteArrayRegion(env,result,0,(jsize)size,(const jbyte *)bytes);
    free(bytes);
    if(!valid) {throw_io(env,"Sound export failed");return NULL;}return result;
}
static uint64_t monotonic_ns(void) {
    struct timespec now; clock_gettime(CLOCK_MONOTONIC,&now);
    return (uint64_t)now.tv_sec*1000000000ull+(uint64_t)now.tv_nsec;
}
static void throw_io(JNIEnv *env, const char *message) {
    jclass cls = (*env)->FindClass(env, "java/io/IOException");
    if (cls) (*env)->ThrowNew(env, cls, message);
}
JNIEXPORT jlong JNICALL Java_org_rcport_diagnostic_PlayActivity_createGame(JNIEnv *env, jclass klass, jbyteArray data) {
    (void)klass;
    if (!data) {throw_io(env,"Missing game scene"); return 0;}
    jsize len=(*env)->GetArrayLength(env,data);
    jbyte *input=(*env)->GetByteArrayElements(env,data,NULL);
    if (!input) return 0;
    char error[512]={0};
    uint64_t id=rc_game_create((const uint8_t *)input,(size_t)len,error,sizeof(error));
    (*env)->ReleaseByteArrayElements(env,data,input,JNI_ABORT);
    if (!id) throw_io(env,error[0] ? error : "Unable to load playable scene");
    return (jlong)id;
}
JNIEXPORT jintArray JNICALL Java_org_rcport_diagnostic_PlayActivity_gameFrame(JNIEnv *env, jclass klass, jlong id,
    jdouble elapsed,jfloat forward,jfloat strafe,jfloat look_x,jfloat look_y,jint buttons,jint width,jint height,jlongArray metrics) {
    (void)klass;
    if(width<=0 || height<=0 || width>2048 || height>2048) {throw_io(env,"Invalid game frame size"); return NULL;}
    int profiling=metrics!=NULL;
    if(profiling && ((*env)->GetArrayLength(env,metrics)<26 || rc_profile_begin()!=0)) {
        throw_io(env,"Invalid performance capture"); return NULL;
    }
    uint64_t array_start=profiling ? monotonic_ns() : 0;
    jsize count=width*height;
    jintArray result=(*env)->NewIntArray(env,count+10);
    jint *output=result ? (*env)->GetIntArrayElements(env,result,NULL) : NULL;
    if(profiling) rc_profile_add(8,monotonic_ns()-array_start);
    if(!output) {
        if(profiling) {uint64_t discarded[20]; rc_profile_finish(discarded,20);}
        return NULL;
    }
    int32_t ticks=rc_game_advance((uint64_t)id,elapsed,forward,strafe,look_x,look_y,(uint32_t)buttons);
    intptr_t drawn=ticks<0 ? -1 : rc_game_draw((uint64_t)id,(size_t)width,(size_t)height,(uint32_t *)(output+10),(size_t)count);
    int32_t status=drawn==count ? rc_game_status((uint64_t)id,(uint32_t *)output,10) : -1;
    array_start=profiling ? monotonic_ns() : 0;
    (*env)->ReleaseIntArrayElements(env,result,output,0);
    if(profiling) {
        rc_profile_add(8,monotonic_ns()-array_start);
        uint64_t values[26]={0};
        rc_profile_finish(values,20);
        double pose[6]={0};
        if(rc_game_observe((uint64_t)id,pose,6)==6) memcpy(values+20,pose,sizeof(pose));
        (*env)->SetLongArrayRegion(env,metrics,0,26,(const jlong *)values);
    }
    if(status!=10) {throw_io(env,"Playable scene step or draw failed"); return NULL;}
    return result;
}
JNIEXPORT void JNICALL Java_org_rcport_diagnostic_PlayActivity_gameCommand(JNIEnv *env,jclass klass,jlong id,jint command) {
    (void)klass;
    if(rc_game_command((uint64_t)id,(uint32_t)command)!=0) throw_io(env,"Invalid game command");
}
JNIEXPORT void JNICALL Java_org_rcport_diagnostic_PlayActivity_releaseGame(JNIEnv *env,jclass klass,jlong id) {
    (void)env; (void)klass; rc_game_release((uint64_t)id);
}
static jbyteArray graphics_packet(JNIEnv *env,uint64_t id,uint32_t kind,int status_prefix) {
    intptr_t size=rc_game_graphics(id,kind,NULL,0);
    if(size<=0 || size>64*1024*1024) {throw_io(env,"Invalid graphics packet"); return NULL;}
    size_t prefix=status_prefix ? 40 : 0;
    uint8_t *bytes=malloc((size_t)size+prefix);
    if(!bytes) {throw_io(env,"Graphics allocation failed"); return NULL;}
    int valid=rc_game_graphics(id,kind,bytes+prefix,(size_t)size)==size;
    if(valid && status_prefix) {
        uint32_t status[10]; valid=rc_game_status(id,status,10)==10;
        // Android targets are little-endian; packet fields have explicit byte order.
        for(int i=0;valid && i<10;i++) for(int j=0;j<4;j++) bytes[i*4+j]=(uint8_t)(status[i]>>(8*j));
    }
    jbyteArray result=valid ? (*env)->NewByteArray(env,(jsize)((size_t)size+prefix)) : NULL;
    if(result) (*env)->SetByteArrayRegion(env,result,0,(jsize)((size_t)size+prefix),(const jbyte *)bytes);
    free(bytes);
    if(!valid) throw_io(env,"Graphics export failed");
    return result;
}
JNIEXPORT jbyteArray JNICALL Java_org_rcport_diagnostic_PlayActivity_gpuWorld(JNIEnv *env,jclass klass,jlong id) {
    (void)klass; return graphics_packet(env,(uint64_t)id,0,0);
}
JNIEXPORT jbyteArray JNICALL Java_org_rcport_diagnostic_PlayActivity_gpuFrame(JNIEnv *env,jclass klass,jlong id,
    jdouble elapsed,jfloat forward,jfloat strafe,jfloat look_x,jfloat look_y,jint buttons,jlongArray metrics) {
    (void)klass;
    int profiling=metrics!=NULL;
    if(profiling && ((*env)->GetArrayLength(env,metrics)<26 || rc_profile_begin()!=0)) {
        throw_io(env,"Invalid performance capture"); return NULL;
    }
    int32_t ticks=rc_game_advance((uint64_t)id,elapsed,forward,strafe,look_x,look_y,(uint32_t)buttons);
    uint64_t start=profiling ? monotonic_ns() : 0;
    jbyteArray result=ticks<0 ? NULL : graphics_packet(env,(uint64_t)id,1,1);
    if(profiling) {
        rc_profile_add(8,monotonic_ns()-start);
        uint64_t values[26]={0}; rc_profile_finish(values,20);
        double pose[6]={0};
        if(rc_game_observe((uint64_t)id,pose,6)==6) memcpy(values+20,pose,sizeof(pose));
        (*env)->SetLongArrayRegion(env,metrics,0,26,(const jlong *)values);
    }
    if(ticks<0) throw_io(env,"Playable scene step failed");
    return result;
}
JNIEXPORT jint JNICALL Java_org_rcport_diagnostic_DiagnosticActivity_checkBsp(JNIEnv *env, jclass klass, jlong id, jfloat x, jfloat y, jfloat z, jfloat yaw, jfloat pitch) {
    (void)env; (void)klass;
    return rc_scene_bsp_query((uint64_t)id,x,y,z,yaw,pitch);
}
JNIEXPORT jfloatArray JNICALL Java_org_rcport_diagnostic_DiagnosticActivity_startCamera(JNIEnv *env, jclass klass, jlong id, jint index) {
    (void)klass;
    float values[6];
    if (index < 0 || rc_scene_start((uint64_t)id, (size_t)index, values, 6) != 6) {
        throw_io(env,"Kein unterstützter Startpunkt in dieser Szene");
        return NULL;
    }
    jfloatArray result = (*env)->NewFloatArray(env, 6);
    if (result) (*env)->SetFloatArrayRegion(env, result, 0, 6, values);
    return result;
}
JNIEXPORT jlong JNICALL Java_org_rcport_diagnostic_DiagnosticActivity_createScene(JNIEnv *env, jclass klass, jbyteArray data) {
    (void)klass;
    if (!data) {throw_io(env, "Missing map"); return 0;}
    jsize len = (*env)->GetArrayLength(env, data);
    jbyte *input = (*env)->GetByteArrayElements(env, data, NULL);
    if (!input) return 0;
    char error[512] = {0};
    uint64_t id = rc_scene_create((const uint8_t *)input, (size_t)len, error, sizeof(error));
    (*env)->ReleaseByteArrayElements(env, data, input, JNI_ABORT);
    if (!id) throw_io(env, error[0] ? error : "Unable to create scene");
    return (jlong)id;
}
JNIEXPORT jintArray JNICALL Java_org_rcport_diagnostic_DiagnosticActivity_drawScene(JNIEnv *env, jclass klass, jlong id, jint width, jint height, jfloat yaw, jfloat pitch, jfloat zoom, jfloat x, jfloat y, jfloat z, jboolean free) {
    (void)klass;
    if (width <= 0 || height <= 0 || width > 2048 || height > 2048) {throw_io(env,"Invalid frame size"); return NULL;}
    jsize count = width * height;
    jintArray pixels = (*env)->NewIntArray(env, count);
    if (!pixels) return NULL;
    jint *output = (*env)->GetIntArrayElements(env, pixels, NULL);
    if (!output) return NULL;
    intptr_t result = rc_scene_draw_view((uint64_t)id, (size_t)width, (size_t)height, yaw, pitch, zoom, x, y, z, free ? 1 : 0, (uint32_t *)output, (size_t)count);
    (*env)->ReleaseIntArrayElements(env, pixels, output, 0);
    if (result != count) {throw_io(env,"Native render failed"); return NULL;}
    return pixels;
}
JNIEXPORT void JNICALL Java_org_rcport_diagnostic_DiagnosticActivity_releaseScene(JNIEnv *env, jclass klass, jlong id) {
    (void)env; (void)klass; rc_scene_release((uint64_t)id);
}
JNIEXPORT jstring JNICALL Java_org_rcport_diagnostic_DiagnosticActivity_inspectPackage(JNIEnv *env, jclass klass, jbyteArray data) {
    (void)klass;
    if (!data) return (*env)->NewStringUTF(env, "Missing package");
    jsize length = (*env)->GetArrayLength(env, data);
    jbyte *input = (*env)->GetByteArrayElements(env, data, NULL);
    if (!input) return NULL;
    char buffer[512] = {0};
    intptr_t needed = rc_package_summary((const uint8_t *)input, (size_t)length, buffer, sizeof(buffer));
    (*env)->ReleaseByteArrayElements(env, data, input, JNI_ABORT);
    if (needed <= 0 || needed > (intptr_t)sizeof(buffer)) return (*env)->NewStringUTF(env, "Native diagnostic buffer error");
    return (*env)->NewStringUTF(env, buffer);
}

extern int32_t rc_game_jump_status(uint64_t, uint32_t *, size_t);
extern intptr_t rc_game_mission_status(uint64_t, uint8_t *, size_t);
extern intptr_t rc_game_actor_status(uint64_t, uint8_t *, size_t);
extern intptr_t rc_game_map_handoff(uint64_t, uint8_t *, size_t);
extern int32_t rc_game_travel(uint64_t,const uint8_t *,size_t,char *,size_t);
extern int32_t rc_game_restore_handoff(uint64_t,const uint8_t *,size_t,char *,size_t);
JNIEXPORT void JNICALL Java_org_rcport_diagnostic_PlayActivity_restoreHandoff(JNIEnv *env, jclass klass, jlong id, jbyteArray bytes) {
    (void)klass;
    if(!bytes){throw_io(env,"Missing saved handoff");return;}
    jsize n=(*env)->GetArrayLength(env,bytes);jbyte *data=(*env)->GetByteArrayElements(env,bytes,NULL);if(!data)return;
    char error[512]={0};int32_t result=rc_game_restore_handoff((uint64_t)id,(const uint8_t *)data,(size_t)n,error,sizeof(error));
    (*env)->ReleaseByteArrayElements(env,bytes,data,JNI_ABORT);
    if(result!=0)throw_io(env,error[0]?error:"Saved map handoff rejected");
}
JNIEXPORT void JNICALL Java_org_rcport_diagnostic_PlayActivity_travelGame(JNIEnv *env, jclass klass, jlong id, jbyteArray bytes) {
    (void)klass;
    if(!bytes){throw_io(env,"Missing destination scene");return;}
    jsize n=(*env)->GetArrayLength(env,bytes);
    jbyte *data=(*env)->GetByteArrayElements(env,bytes,NULL);
    if(!data)return;
    char error[512]={0};int32_t result=rc_game_travel((uint64_t)id,(const uint8_t *)data,(size_t)n,error,sizeof(error));
    (*env)->ReleaseByteArrayElements(env,bytes,data,JNI_ABORT);
    if(result!=0)throw_io(env,error[0]?error:"Map transition failed");
}
JNIEXPORT jbyteArray JNICALL Java_org_rcport_diagnostic_PlayActivity_mapHandoff(JNIEnv *env, jclass klass, jlong id) {
    (void)klass;
    intptr_t n=rc_game_map_handoff((uint64_t)id,NULL,0);
    if(n<0 || n>4*1024*1024){throw_io(env,"Map handoff size failed");return NULL;}
    jbyteArray result=(*env)->NewByteArray(env,(jsize)n);
    if(!result || n==0)return result;
    jbyte *data=(*env)->GetByteArrayElements(env,result,NULL);
    if(!data)return NULL;
    intptr_t copied=rc_game_map_handoff((uint64_t)id,(uint8_t *)data,(size_t)n);
    (*env)->ReleaseByteArrayElements(env,result,data,0);
    if(copied!=n){throw_io(env,"Map handoff copy failed");return NULL;}
    return result;
}
JNIEXPORT jstring JNICALL Java_org_rcport_diagnostic_PlayActivity_actorStatus(JNIEnv *env, jclass klass, jlong id) {
    (void)klass;
    uint8_t buffer[8192];
    intptr_t n=rc_game_actor_status((uint64_t)id,buffer,sizeof(buffer)-1);
    if(n<0 || n>=(intptr_t)sizeof(buffer)){throw_io(env,"Actor status failed");return NULL;}
    buffer[n]=0;
    return (*env)->NewStringUTF(env,(const char *)buffer);
}
JNIEXPORT jstring JNICALL Java_org_rcport_diagnostic_PlayActivity_missionStatus(JNIEnv *env, jclass klass, jlong id) {
    (void)klass;
    uint8_t buffer[2048];
    intptr_t n=rc_game_mission_status((uint64_t)id,buffer,sizeof(buffer)-1);
    if(n<0 || n>=(intptr_t)sizeof(buffer)){throw_io(env,"Mission status failed");return NULL;}
    buffer[n]=0;
    return (*env)->NewStringUTF(env,(const char *)buffer);
}
JNIEXPORT jintArray JNICALL Java_org_rcport_diagnostic_PlayActivity_jumpStatus(JNIEnv *env, jclass klass, jlong id) {
    (void)klass;
    uint32_t words[12];
    if (rc_game_jump_status((uint64_t)id,words,12)!=12) {throw_io(env,"Jump observer failed");return NULL;}
    jintArray result=(*env)->NewIntArray(env,12);
    if (result) (*env)->SetIntArrayRegion(env,result,0,12,(const jint *)words);
    return result;
}

extern intptr_t rc_game_checkpoint(uint64_t,uint8_t*,size_t);
extern int32_t rc_game_restore_checkpoint(uint64_t,const uint8_t*,size_t,char*,size_t);
extern uint64_t rc_game_checkpoint_marker(uint64_t);
JNIEXPORT jbyteArray JNICALL Java_org_rcport_diagnostic_PlayActivity_gameCheckpoint(JNIEnv*env,jclass klass,jlong id) {
 (void)klass;intptr_t n=rc_game_checkpoint((uint64_t)id,NULL,0);
 if(n<20 || n>8*1024*1024){throw_io(env,"Checkpoint export failed");return NULL;}
 uint8_t*b=malloc((size_t)n);if(!b){throw_io(env,"Checkpoint allocation failed");return NULL;}
 jbyteArray result=(*env)->NewByteArray(env,(jsize)n);
 if(!result || rc_game_checkpoint((uint64_t)id,b,(size_t)n)!=n){free(b);throw_io(env,"Checkpoint export failed");return NULL;}
 (*env)->SetByteArrayRegion(env,result,0,(jsize)n,(const jbyte*)b);free(b);return result;
}
JNIEXPORT void JNICALL Java_org_rcport_diagnostic_PlayActivity_restoreCheckpoint(JNIEnv*env,jclass klass,jlong id,jbyteArray data) {
 (void)klass;if(!data){throw_io(env,"Missing checkpoint");return;}
 jsize n=(*env)->GetArrayLength(env,data);jbyte*b=(*env)->GetByteArrayElements(env,data,NULL);if(!b)return;
 char error[512]={0};int valid=rc_game_restore_checkpoint((uint64_t)id,(uint8_t*)b,(size_t)n,error,sizeof(error));
 (*env)->ReleaseByteArrayElements(env,data,b,JNI_ABORT);if(valid!=0)throw_io(env,error[0]?error:"Checkpoint restore failed");
}
JNIEXPORT jlong JNICALL Java_org_rcport_diagnostic_PlayActivity_checkpointMarker(JNIEnv*env,jclass klass,jlong id) {
 (void)env;(void)klass;return (jlong)rc_game_checkpoint_marker((uint64_t)id);
}

JNIEXPORT jintArray JNICALL Java_org_rcport_diagnostic_PlayActivity_gameStatus(JNIEnv*env,jclass klass,jlong id) {
 (void)klass;uint32_t words[10];if(rc_game_status((uint64_t)id,words,10)!=10){throw_io(env,"Invalid game session");return NULL;}
 jintArray out=(*env)->NewIntArray(env,10);if(out)(*env)->SetIntArrayRegion(env,out,0,10,(const jint*)words);return out;
}
JNIEXPORT jint JNICALL Java_org_rcport_diagnostic_PlayActivity_gameMap(JNIEnv*env,jclass klass,jlong id) {
 (void)env;(void)klass;return rc_game_campaign_map((uint64_t)id);
}
