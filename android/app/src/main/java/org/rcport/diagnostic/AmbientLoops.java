package org.rcport.diagnostic;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Desired native loop state; no event cursor or playback position survives a pause. */
final class AmbientLoops {
    private static final class Voice {
        final int sound; int stream;
        Voice(int sound,int stream){this.sound=sound;this.stream=stream;}
    }
    private Map<Integer,Voice> desired=new LinkedHashMap<>();
    int[] select(byte[] packet,int bankCount) throws IOException {
        if(packet.length<12 || !new String(packet,0,8,StandardCharsets.US_ASCII).equals("RCLOOP01"))throw new IOException("Invalid ambient snapshot");
        ByteBuffer b=ByteBuffer.wrap(packet).order(ByteOrder.LITTLE_ENDIAN);b.position(8);int count=b.getInt();
        if(count<0 || count>16 || b.remaining()!=count*8)throw new IOException("Invalid ambient count");
        Map<Integer,Voice> next=new LinkedHashMap<>();
        for(int i=0;i<count;i++) {
            int sound=b.getInt(),owner=b.getInt();
            if(sound<0 || sound>=bankCount || owner<1 || next.containsKey(owner))throw new IOException("Invalid ambient identity");
            Voice old=desired.get(owner);next.put(owner,new Voice(sound,old!=null && old.sound==sound?old.stream:0));
        }
        List<Integer> stopped=new ArrayList<>();
        for(Map.Entry<Integer,Voice> e:desired.entrySet()) {
            Voice n=next.get(e.getKey()),old=e.getValue();
            if(old.stream!=0 && (n==null || n.sound!=old.sound))stopped.add(old.stream);
        }
        desired=next;return ints(stopped);
    }
    int[] pendingOwners(){List<Integer> p=new ArrayList<>();for(Map.Entry<Integer,Voice> e:desired.entrySet())if(e.getValue().stream==0)p.add(e.getKey());return ints(p);}
    int sound(int owner){return desired.get(owner).sound;}
    void played(int owner,int stream){Voice v=desired.get(owner);if(v!=null && stream!=0)v.stream=stream;}
    int[] clear(){List<Integer> p=new ArrayList<>();for(Voice v:desired.values())if(v.stream!=0)p.add(v.stream);desired.clear();return ints(p);}
    private static int[] ints(List<Integer> a){int[] b=new int[a.size()];for(int i=0;i<b.length;i++)b[i]=a.get(i);return b;}
}
