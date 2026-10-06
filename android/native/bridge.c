#include <jni.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
extern intptr_t rc_package_summary(const uint8_t *, size_t, char *, size_t);
extern uint64_t rc_scene_create(const uint8_t *, size_t, char *, size_t);
extern intptr_t rc_scene_draw_view(uint64_t, size_t, size_t, float, float, float, float, float, float, uint32_t, uint32_t *, size_t);
extern void rc_scene_release(uint64_t);
extern intptr_t rc_scene_start(uint64_t, size_t, float *, size_t);
extern int32_t rc_scene_bsp_query(uint64_t, float, float, float, float, float);
static void throw_io(JNIEnv *env, const char *message) {
    jclass cls = (*env)->FindClass(env, "java/io/IOException");
    if (cls) (*env)->ThrowNew(env, cls, message);
}
JNIEXPORT jint JNICALL Java_org_rcport_diagnostic_MainActivity_checkBsp(JNIEnv *env, jclass klass, jlong id, jfloat x, jfloat y, jfloat z, jfloat yaw, jfloat pitch) {
    (void)env; (void)klass;
    return rc_scene_bsp_query((uint64_t)id,x,y,z,yaw,pitch);
}
JNIEXPORT jfloatArray JNICALL Java_org_rcport_diagnostic_MainActivity_startCamera(JNIEnv *env, jclass klass, jlong id, jint index) {
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
JNIEXPORT jlong JNICALL Java_org_rcport_diagnostic_MainActivity_createScene(JNIEnv *env, jclass klass, jbyteArray data) {
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
JNIEXPORT jintArray JNICALL Java_org_rcport_diagnostic_MainActivity_drawScene(JNIEnv *env, jclass klass, jlong id, jint width, jint height, jfloat yaw, jfloat pitch, jfloat zoom, jfloat x, jfloat y, jfloat z, jboolean free) {
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
JNIEXPORT void JNICALL Java_org_rcport_diagnostic_MainActivity_releaseScene(JNIEnv *env, jclass klass, jlong id) {
    (void)env; (void)klass; rc_scene_release((uint64_t)id);
}
JNIEXPORT jstring JNICALL Java_org_rcport_diagnostic_MainActivity_inspectPackage(JNIEnv *env, jclass klass, jbyteArray data) {
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
