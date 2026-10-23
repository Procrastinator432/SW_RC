package org.rcport.diagnostic;

import java.io.IOException;
import java.nio.ByteBuffer;

/** Native immutable per-event PC parameters; owner-local Foley has no distance falloff. */
final class AudioPlayback {
    final float volume,pitch,radius;
    final boolean original;
    private AudioPlayback(float v,float p,float r,boolean o){volume=v;pitch=p;radius=r;original=o;}
    static AudioPlayback legacy(){return new AudioPlayback(1f,1f,0f,false);}
    static AudioPlayback read(ByteBuffer b)throws IOException {
        if(b.remaining()<16)throw new IOException("Truncated sound parameters");
        float v=b.getFloat(),p=b.getFloat(),r=b.getFloat();int flags=b.getInt();
        if(!Float.isFinite(v) || v<0 || v>1 || !Float.isFinite(p) || p<0.5f || p>2f || !Float.isFinite(r) || r<0 || r>1e6f || flags<0 || flags>1
            || flags==1 && v<=0.0001f || flags==0 && (v!=1f || p!=1f || r!=0f))throw new IOException("Invalid sound parameters");
        return new AudioPlayback(v,p,r,flags==1);
    }
    float gain(){return 0.35f*volume;}
}
