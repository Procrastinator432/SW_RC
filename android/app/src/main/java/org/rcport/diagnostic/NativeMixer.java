package org.rcport.diagnostic;
import java.io.IOException;
/** JNI calls stay on the opening producer; OpenAL itself belongs to a dedicated Rust worker. */
final class NativeMixer {
    static { System.loadLibrary("rc_bridge"); }
    private NativeMixer(){}
    static native long open(String library,byte[] wave,int rate,long epoch)throws IOException;
    static native byte[] render(long id,int frames,long epoch,long sequence)throws IOException;
    static native void release(long id)throws IOException;
    static native long mixedOpen(String library,byte[] bundle,long epoch)throws IOException;
    static native void mixedControl(long id,byte[] control)throws IOException;
    static native byte[] mixedExport(long id,int kind)throws IOException;
    static native long sceneOpen(String library,byte[] bank,long epoch)throws IOException;
    static native void sceneUpdate(long id,byte[] events,byte[] loops)throws IOException;
    static native byte[] sceneSnapshot(long id)throws IOException;
    static native void sceneRestore(long id,byte[] snapshot)throws IOException;
    static native void sceneMix(long id,byte[] mix)throws IOException;
    static native byte[] sceneStatus(long id)throws IOException;
    static native void sceneConsume(long id,long cursor)throws IOException;
}
