package org.rcport.diagnostic;
import java.nio.*;
import java.io.IOException;
public final class AudioPlaybackCheck {
    static ByteBuffer packet(float volume,float pitch,float radius,int flags){return ByteBuffer.allocate(16).order(ByteOrder.LITTLE_ENDIAN).putFloat(volume).putFloat(pitch).putFloat(radius).putInt(flags).flip();}
    public static void main(String[] args)throws Exception {
        AudioPlayback a=AudioPlayback.read(packet(0.5f,0.8f,1024f,1));
        if(a.gain()!=0.175f || a.pitch!=0.8f || !a.original || a.radius!=1024f)throw new AssertionError();
        a=AudioPlayback.legacy();if(a.gain()!=0.35f || a.pitch!=1f || a.original)throw new AssertionError();
        ByteBuffer[] invalid={packet(0.0001f,1,0,1),packet(Float.NaN,1,0,1),packet(2,1,0,1),packet(1,0.4f,0,1),packet(1,Float.POSITIVE_INFINITY,0,1),packet(1,1,-1,1),packet(1,1,0,2),packet(0.5f,1,0,0),ByteBuffer.allocate(15)};
        for(ByteBuffer b:invalid){try{AudioPlayback.read(b);throw new AssertionError("Accepted bad playback");}catch(IOException expected){}}
        System.out.println("{\"passed\":true,\"original_gain_and_rate\":true,\"legacy_unchanged\":true,\"invalid_parameters\":9}");
    }
}
