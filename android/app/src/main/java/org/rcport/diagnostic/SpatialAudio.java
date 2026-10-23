package org.rcport.diagnostic;

import java.io.IOException;
import java.nio.ByteBuffer;

/** Original inverse-clamped distance rule with a portable stereo output adapter. */
final class SpatialAudio {
    static final class Mix {
        final float left,right,rate; final boolean spatial;
        Mix(float l,float r,float p,boolean s){left=l;right=r;rate=p;spatial=s;}
    }
    static float finite(ByteBuffer b,float min,float max) throws IOException {
        if(b.remaining()<4)throw new IOException("Truncated spatial audio");
        float v=b.getFloat();if(!Float.isFinite(v) || v<min || v>max)throw new IOException("Invalid spatial audio value");return v;
    }
    static Mix legacy(){return new Mix(.35f,.35f,1f,false);}
    static Mix read(ByteBuffer b,float[] listener,float yaw) throws IOException {
        float x=finite(b,-1e7f,1e7f),y=finite(b,-1e7f,1e7f),z=finite(b,-1e7f,1e7f);
        float volume=finite(b,0,1),pitch=finite(b,.5f,2),radius=finite(b,0,1e6f);
        if(b.remaining()<4)throw new IOException("Truncated spatial flags");
        int flags=b.getInt();
        if((flags&~7)!=0 || (flags&1)==0 && (flags!=0 || volume!=1 || pitch!=1 || radius!=0))throw new IOException("Invalid spatial flags");
        if(flags==0)return legacy();
        if(volume<=.0001f)throw new IOException("Muted spatial voice");
        if((flags&2)!=0)return new Mix(.35f*volume,.35f*volume,pitch,false);
        double dx=(double)x-listener[0],dy=(double)y-listener[1],dz=(double)z-listener[2];
        double distance=Math.sqrt(dx*dx+dy*dy+dz*dz);
        // Original swaps Y/Z and multiplies both positions and radius by .01.
        // This ratio is invariant under that common scale/permutation.
        double attenuation=(flags&4)!=0 || radius==0 || distance<=radius ? 1 : radius/distance;
        // Screen right is (sin(yaw),-cos(yaw),0), matching the game camera.
        double pan=distance==0?0:Math.max(-1,Math.min(1,(dx*Math.sin(yaw)-dy*Math.cos(yaw))/distance));
        double gain=.35*volume*attenuation;
        return new Mix((float)(gain*Math.sqrt((1-pan)*.5)),(float)(gain*Math.sqrt((1+pan)*.5)),pitch,true);
    }
}
