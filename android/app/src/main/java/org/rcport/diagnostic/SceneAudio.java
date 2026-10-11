package org.rcport.diagnostic;

import android.content.Context;
import android.media.AudioAttributes;
import android.media.SoundPool;
import android.util.Log;
import java.io.*;
import java.nio.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Original local PCM, triggered only by consumable Rust simulation events. UI thread owns this host. */
final class SceneAudio implements AutoCloseable {
    private final SoundPool pool;
    private final int[] sounds;
    private final Set<Integer> ready=new HashSet<>();
    private final Map<Integer,File> files=new HashMap<>();
    private final ArrayDeque<Event> pending=new ArrayDeque<>();
    private final ArrayDeque<Integer> streams=new ArrayDeque<>();
    private boolean closed;
    private long lastSequence;
    private static final class Event {
        final long sequence,tick; final int sound,owner;
        Event(long s,long t,int i,int o) {sequence=s;tick=t;sound=i;owner=o;}
    }
    private static ByteBuffer packet(byte[] bytes,String magic) throws IOException {
        if(bytes.length<12 || !new String(bytes,0,8,StandardCharsets.US_ASCII).equals(magic)) throw new IOException("Invalid sound packet");
        ByteBuffer b=ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN); b.position(8);return b;
    }
    SceneAudio(Context context,byte[] bytes) throws IOException {
        ByteBuffer b=packet(bytes,"RCSND001"); int count=b.getInt();
        if(count<0 || count>64) throw new IOException("Invalid sound bank count");
        sounds=new int[count];
        pool=new SoundPool.Builder().setMaxStreams(16).setAudioAttributes(new AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_GAME).setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build()).build();
        pool.setOnLoadCompleteListener((p,id,status) -> {
            File file=files.remove(id);if(file!=null) file.delete();
            if(closed) return;
            if(status==0) {ready.add(id);Log.i("RCAudio","loaded sample="+id);flush();}
            else {Log.e("RCAudio","sample load failed="+status);pending.clear();}
        });
        try {
            for(int i=0;i<count;i++) {
                if(b.remaining()<4) throw new IOException("Truncated sound bank");
                int size=b.getInt(); if(size<44 || size>2*1024*1024 || size>b.remaining()) throw new IOException("Invalid WAV length");
                File file=File.createTempFile("rc-sound-",".wav",context.getCacheDir());
                try(FileOutputStream out=new FileOutputStream(file)) {out.write(bytes,b.position(),size);}
                b.position(b.position()+size);
                int id=pool.load(file.getAbsolutePath(),1);
                if(id==0) {file.delete();throw new IOException("Unable to load original WAV");}
                files.put(id,file);sounds[i]=id;
            }
            if(b.hasRemaining()) throw new IOException("Trailing sound bank bytes");
            Log.i("RCAudio","bank sounds="+count);
        } catch(IOException|RuntimeException e) {close();throw e;}
    }
    boolean hasOriginalSounds() {return sounds.length>0;}
    void events(byte[] bytes) throws IOException {
        ByteBuffer b=packet(bytes,"RCAUD001");int count=b.getInt();
        if(count<0 || count>256 || b.remaining()!=count*24) throw new IOException("Invalid sound events");
        for(int i=0;i<count;i++) {
            Event e=new Event(b.getLong(),b.getLong(),b.getInt(),b.getInt());
            if(e.sequence<=lastSequence || e.sound<0 || e.sound>=sounds.length) throw new IOException("Invalid sound event identity");
            lastSequence=e.sequence;
            if(pending.size()==256) pending.removeFirst();pending.add(e);
        }
        flush();
    }
    private void flush() {
        if(closed || ready.size()!=sounds.length) return;
        while(!pending.isEmpty()) {
            Event e=pending.removeFirst();
            int stream=pool.play(sounds[e.sound],0.35f,0.35f,1,0,1f);
            if(stream==0) {Log.e("RCAudio","play failed sequence="+e.sequence);continue;}
            if(streams.size()==64) streams.removeFirst();streams.add(stream);
            Log.i("RCAudio","event sequence="+e.sequence+" tick="+e.tick+" sound="+e.sound+" owner="+e.owner);
        }
    }
    void stop() {pending.clear();for(int id:streams) pool.stop(id);streams.clear();}
    void reset() {stop();lastSequence=0;}
    @Override public void close() {
        if(closed) return;closed=true;stop();pool.release();
        for(File file:files.values()) file.delete();files.clear();ready.clear();
    }
}
