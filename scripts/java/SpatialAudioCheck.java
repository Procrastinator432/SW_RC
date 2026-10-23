package org.rcport.diagnostic;
import java.nio.*;
import java.io.IOException;
public final class SpatialAudioCheck {
    static void near(double a,double b){if(Math.abs(a-b)>1e-6)throw new AssertionError(a+" != "+b);}
    static byte[] packet(float x,float y,float z,float volume,float pitch,float radius,int flags,float yaw){
        return ByteBuffer.allocate(64).order(ByteOrder.LITTLE_ENDIAN).put("RCLOOP02".getBytes(java.nio.charset.StandardCharsets.US_ASCII)).putInt(1)
            .putFloat(0).putFloat(0).putFloat(0).putFloat(yaw).putInt(3).putInt(736)
            .putFloat(x).putFloat(y).putFloat(z).putFloat(volume).putFloat(pitch).putFloat(radius).putInt(flags).array();
    }
    static SpatialAudio.Mix mix(byte[] b)throws Exception{AmbientLoops a=new AmbientLoops();a.select(b,10);return a.mix(736);}
    static byte[] identified(long activation){byte[] bytes=java.util.Arrays.copyOf(packet(0,100,0,1,1,100,1,0),72);bytes[7]='3';ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).putLong(64,activation);return bytes;}
    public static void main(String[] args)throws Exception {
        SpatialAudio.Mix a=mix(packet(100,0,0,1,1,100,1,0));near(a.left,.35/Math.sqrt(2));near(a.right,a.left);
        SpatialAudio.Mix twice=mix(packet(200,0,0,1,1,100,1,0));near(twice.left,a.left*.5);near(twice.right,a.right*.5);
        a=mix(packet(0,-100,0,.5f,.97f,100,1,0));near(a.left,0);near(a.right,.175);near(a.rate,.97f);
        a=mix(packet(0,100,0,1,1,100,1,0));near(a.left,.35);near(a.right,0);
        a=mix(packet(0,100,0,1,1,100,1,(float)Math.PI));near(a.left,0);near(a.right,.35);
        a=mix(packet(0,0,100,1,1,100,1,0));near(a.left,a.right);near(a.left,.35/Math.sqrt(2));
        a=mix(packet(0,0,0,1,1,100,1,0));near(a.left,a.right);
        a=mix(packet(10000,0,0,1,1,100,5,0));near(a.left,.35/Math.sqrt(2));
        a=mix(packet(10000,0,0,1,1,0,1,0));near(a.left,.35/Math.sqrt(2));
        a=mix(packet(10000,0,0,.5f,1,100,3,0));near(a.left,.175);near(a.right,.175);if(a.spatial)throw new AssertionError();
        a=mix(packet(0,0,0,1,1,0,0,0));near(a.left,.35);near(a.right,.35);
        AmbientLoops loops=new AmbientLoops();loops.select(packet(0,-100,0,1,1,100,1,0),10);loops.played(736,42);
        loops.select(packet(0,100,0,1,.96f,100,1,0),10);if(loops.stream(736)!=42 || loops.pendingOwners().length!=0)throw new AssertionError("Movement restarted stream");
        near(loops.mix(736).right,0);near(loops.mix(736).rate,.96f);
        for(byte[] bad:new byte[][] {packet(Float.NaN,0,0,1,1,100,1,0),packet(0,0,0,1,Float.POSITIVE_INFINITY,100,1,0),packet(0,0,0,0,1,100,1,0),packet(0,0,0,1,1,-1,1,0),packet(0,0,0,1,1,100,8,0),packet(0,0,0,1,1,100,1,Float.NaN),packet(0,0,0,.5f,1,0,0,0),new byte[28]}) {
            try{loops.select(bad,10);throw new AssertionError("Accepted bad snapshot");}catch(IOException expected){}
            if(loops.stream(736)!=42)throw new AssertionError("Invalid packet changed live stream");
        }
        if(loops.clear()[0]!=42)throw new AssertionError();
        loops.select(packet(0,100,0,1,.96f,100,1,0),10);if(loops.pendingOwners().length!=1)throw new AssertionError("Restore did not request current voice");
        loops.clear();loops.select(identified(10),10);loops.played(736,50);
        if(loops.select(identified(10),10).length!=0 || loops.stream(736)!=50)throw new AssertionError("Same activation restarted");
        try{loops.select(identified(-1),10);throw new AssertionError("Accepted negative activation");}catch(IOException expected){}
        if(loops.stream(736)!=50)throw new AssertionError("Bad activation changed stream");
        if(loops.select(identified(11),10)[0]!=50 || loops.pendingOwners().length!=1)throw new AssertionError("New activation reused previous PCM stream");
        System.out.println("{\"passed\":true,\"inverse_clamped_and_orientation\":true,\"live_update_without_restart\":true,\"legacy_and_nonpositional\":true,\"invalid_snapshot_atomic\":true}");
    }
}
