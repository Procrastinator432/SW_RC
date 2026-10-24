package org.rcport.diagnostic;
import java.io.*;
import java.nio.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
/** Pure mission packet/lifecycle gate. A new mailbox binds each output session. */
final class SceneAudioMailbox {
    static final byte[] EMPTY="RCAUD003\0\0\0\0".getBytes(StandardCharsets.US_ASCII);
    static final class Batch {final byte[] events,loops;Batch(byte[]e,byte[]l){events=e;loops=l;}}
    static final class Pause {final List<Batch> dialogs;final long receipt;Pause(List<Batch> dialogs,long receipt){this.dialogs=Collections.unmodifiableList(dialogs);this.receipt=receipt;}}
    private final int count;
    private final ArrayDeque<byte[]> events=new ArrayDeque<>();
    private byte[] loops;
    private boolean dirty,closed,loopReady;
    private long last;
    private final long initialCursor;
    private long received=-1;
    private byte[] mix=MissionMix.defaults().packet();
    private boolean mixDirty=true;
    SceneAudioMailbox(int count,long last){if(last<0)throw new IllegalArgumentException("Initial audio cursor");this.count=count;this.last=last;this.initialCursor=last;loops=ByteBuffer.allocate(32).order(ByteOrder.LITTLE_ENDIAN).put("RCLOOP04".getBytes(StandardCharsets.US_ASCII)).array();}
    static int bankCount(byte[] bytes)throws IOException{
        bytes=MissionSoundtrack.waveBank(bytes);
        if(bytes.length<12 || bytes.length>8*1024*1024+1024)throw new IOException("Mission sound bank length");ByteBuffer b=ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);byte[]magic=new byte[8];b.get(magic);
        if(!Arrays.equals(magic,"RCSND001".getBytes(StandardCharsets.US_ASCII)))throw new IOException("Mission sound bank magic");int count=b.getInt();if(count<0 || count>128)throw new IOException("Mission sound count");
        for(int i=0;i<count;i++){if(b.remaining()<4)throw new IOException("Truncated mission bank");int n=b.getInt();if(n<44 || n>2*1024*1024 || n>b.remaining())throw new IOException("Mission WAV length");b.position(b.position()+n);}if(b.hasRemaining())throw new IOException("Mission bank trailing bytes");return count;
    }
    static long validate(byte[] bytes,int bank,long last)throws IOException{
        if(bytes.length<12 || bytes.length>12+256*48 || !new String(bytes,0,8,StandardCharsets.US_ASCII).equals("RCAUD003"))throw new IOException("Mission event packet");ByteBuffer b=ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);b.position(8);int count=b.getInt();if(count<0 || count>256 || b.remaining()!=count*48)throw new IOException("Mission event count");
        long next=last;
        for(int i=0;i<count;i++){long sequence=b.getLong(),tick=b.getLong();int sound=b.getInt();b.getInt();int category=b.getInt(),flags=b.getInt();AudioPlayback.read(b);
            if(sequence<=next || tick<0 || sound<0 || sound>=bank || category<0 || category>2 || flags<0 || flags>1)throw new IOException("Mission event identity");next=sequence;}
        return next;
    }
    synchronized void offer(byte[] packet)throws IOException{
        long next=validate(packet,count,last);if(closed)throw new IOException("Mission audio session closed");if(packet.length>12){if(events.size()>=8)throw new IOException("Mission audio command queue full");events.add(packet.clone());received=next;}last=next;
    }
    synchronized void loops(byte[] packet)throws IOException{if(closed)return;new AmbientLoops().select(MissionSoundtrack.pose(packet),count);if(!loopReady || !Arrays.equals(loops,packet)){loops=packet.clone();dirty=true;}loopReady=true;}
    synchronized Batch poll(){if(closed || !loopReady || events.isEmpty() && !dirty)return null;byte[] event=events.isEmpty()?EMPTY:events.removeFirst();dirty=false;return new Batch(event,loops);}
    long initialCursor(){return initialCursor;}
    synchronized long last(){return last;}
    synchronized boolean hasLoopState(){return loopReady;}
    synchronized void mix(byte[] packet)throws IOException{MissionMix.validate(packet);if(closed)return;if(!Arrays.equals(mix,packet)){mix=packet.clone();mixDirty=true;}}
    synchronized byte[] takeMix(){if(!mixDirty)return null;mixDirty=false;return mix.clone();}
    /** Seal UI ingress but leave unconsumed packets for the owning producer. */
    synchronized void seal(){closed=true;}
    synchronized List<Batch> pauseDialogs()throws IOException{return pause().dialogs;}
    synchronized Pause pause()throws IOException{
        closed=true;List<Batch> retained=new ArrayList<>();int total=0;
        while(!events.isEmpty()){
            byte[] packet=events.removeFirst();ByteBuffer input=ByteBuffer.wrap(packet).order(ByteOrder.LITTLE_ENDIAN);
            ByteArrayOutputStream records=new ByteArrayOutputStream();int count=0;
            for(int at=12;at<packet.length;at+=48){
                if(input.getInt(at+24)==1){records.write(packet,at,48);count++;}
            }
            if(count>0){
                total+=count;if(total>512 || !loopReady)throw new IOException("Dialogfortsetzung überschreitet das Übergabebudget");
                ByteBuffer filtered=ByteBuffer.allocate(12+count*48).order(ByteOrder.LITTLE_ENDIAN);
                filtered.put(EMPTY).putInt(8,count).position(12);filtered.put(records.toByteArray());
                retained.add(new Batch(filtered.array(),loops.clone()));
            }
        }
        dirty=false;long receipt=received;received=-1;return new Pause(retained,receipt);
    }
    synchronized void close(){closed=true;events.clear();dirty=false;received=-1;}
}
