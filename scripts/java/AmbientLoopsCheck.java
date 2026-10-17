package org.rcport.diagnostic;
import java.nio.*;
import java.io.IOException;
public final class AmbientLoopsCheck {
    static void eq(int a,int b){if(a!=b)throw new AssertionError(a+" != "+b);}
    static byte[] snapshot(int... pairs){ByteBuffer b=ByteBuffer.allocate(12+pairs.length*4).order(ByteOrder.LITTLE_ENDIAN);b.put("RCLOOP01".getBytes(java.nio.charset.StandardCharsets.US_ASCII));b.putInt(pairs.length/2);for(int v:pairs)b.putInt(v);return b.array();}
    public static void main(String[] args)throws Exception {
        AmbientLoops p=new AmbientLoops();byte[] active=snapshot(3,736);
        eq(p.select(active,10).length,0);eq(p.pendingOwners()[0],736);eq(p.sound(736),3);
        p.played(736,0);eq(p.pendingOwners().length,1); // failed play is retried
        p.played(736,17);eq(p.select(active,10).length,0);eq(p.pendingOwners().length,0); // one stream per active owner
        for(byte[] bad:new byte[][]{snapshot(3,736,4,736),snapshot(10,736),snapshot(3,-1),new byte[12]}){
            try{p.select(bad,10);throw new AssertionError("accepted malformed snapshot");}catch(IOException expected){}
            eq(p.pendingOwners().length,0); // rejected snapshots preserve the live stream
        }
        eq(p.select(snapshot(4,736,5,100),10)[0],17);eq(p.pendingOwners().length,2);
        p.played(736,18);p.played(100,19);eq(p.select(snapshot(4,736),10)[0],19);
        eq(p.clear()[0],18);eq(p.pendingOwners().length,0); // pause/map change closes long loops
        eq(p.select(active,10).length,0);eq(p.pendingOwners().length,1); // restore resumes current state, not old event history
        p.played(736,20);eq(p.select(snapshot(),10)[0],20);eq(p.pendingOwners().length,0);
        System.out.println("{\"passed\":true,\"idempotent_loop_state\":true,\"stop_pause_restore\":true,\"invalid_snapshot_atomic\":true}");
    }
}
