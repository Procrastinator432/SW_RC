package org.rcport.diagnostic;

import android.content.Context;
import android.util.Log;
import java.io.IOException;
import java.util.concurrent.*;

/** Mission WAV events and desired loops -> shared Rust/OpenAL worker -> reserved PCM -> AudioTrack. */
final class NativeSceneAudio implements MissionAudio {
    private final Context context;
    private final byte[] bank;
    private final byte[] bankHash;
    private final int count;
    private final String library;
    private final Runnable focusLoss;
    private final ExecutorService producer=Executors.newSingleThreadExecutor(r->new Thread(r,"rc-mission-audio"));
    private volatile long generation;
    private volatile long loadedGeneration=-1;
    private volatile String failure;
    private volatile boolean closed;
    private boolean suspended=true;
    private SceneAudioProducer.Startup startup;
    private long lastSequence;
    private SceneAudioMailbox mailbox;
    private PcmTrackOutput output;
    private MissionMix mix;
    private SceneAudioCapture capture;
    private volatile Long expectedRestoreCursor;
    private final SceneResumeCache resumeCache=new SceneResumeCache();
    NativeSceneAudio(Context context,byte[] bank,long initialCursor,Runnable focusLoss)throws IOException{
        if(initialCursor<0)throw new IOException("Initial mission audio cursor");lastSequence=initialCursor;
        count=SceneAudioMailbox.bankCount(bank);this.context=context;this.bank=bank.clone();bankHash=AudioCheckpointStore.bankHash(bank);this.focusLoss=focusLoss;mix=MissionMixSettings.load(context);library=context.getApplicationInfo().nativeLibraryDir+"/libopenal.so";
    }
    public boolean hasOriginalSounds(){return count>0;}
    void setMix(MissionMix value){mix=value;if(mailbox!=null)try{mailbox.mix(value.packet());}catch(IOException invalid){throw new IllegalArgumentException(invalid);}}
    public boolean ready()throws IOException{if(!closed && !suspended && startup!=null)startup.check(System.nanoTime());if(failure!=null)throw new IOException(failure);if(output!=null && output.pump.failure()!=null)throw new IOException(output.pump.failure());return loadedGeneration==generation && !closed && !suspended;}
    public void events(byte[] bytes)throws IOException{
        long next=SceneAudioMailbox.validate(bytes,count,lastSequence);
        if(!closed && !suspended && mailbox!=null)mailbox.offer(bytes);
        lastSequence=next;
    }
    public void ambientLoops(byte[] bytes)throws IOException{if(!closed && !suspended && mailbox!=null)mailbox.loops(bytes);}
    public void resume(){
        if(closed || !suspended)return;suspended=false;loadedGeneration=-1;failure=null;final long token=++generation;
        startup=new SceneAudioProducer.Startup(System.nanoTime());final SceneAudioProducer.Startup progress=startup;
        final SceneAudioMailbox current=new SceneAudioMailbox(count,lastSequence);mailbox=current;
        setMix(mix);
        final SceneAudioCapture currentCapture=new SceneAudioCapture();capture=currentCapture;
        output=new PcmTrackOutput(context,48000,()->{suspend();focusLoss.run();},true);final PcmTrackOutput target=output;
        if(!target.start()){currentCapture.finish(null);failure="Kein Audiofokus für den Missionsmixer";suspend();return;}
        final long resumeVersion=resumeCache.version();
        producer.execute(()->{
            try{
                SceneAudioProducer.produce(target.pump,current,resumeCache,resumeVersion,epoch->{
                    final long id=NativeMixer.sceneOpen(library,bank,epoch);
                    return new SceneAudioProducer.Source(){
                        public void restore(byte[] bytes)throws IOException{try{NativeMixer.sceneRestore(id,bytes);Long expected=expectedRestoreCursor;if(expected!=null && ScenePoolStatus.decode(NativeMixer.sceneStatus(id)).last!=expected)throw new IOException("Audiostand passt nicht zum Missionsereigniszeiger");Log.i("RCNativeAudio","dialog resume session="+token);}finally{expectedRestoreCursor=null;}}
                        public void mix(byte[] bytes)throws IOException{NativeMixer.sceneMix(id,bytes);}
                        public void consume(long cursor)throws IOException{NativeMixer.sceneConsume(id,cursor);}
                        public void update(byte[] events,byte[] loops)throws IOException{NativeMixer.sceneUpdate(id,events,loops);}
                        public byte[] render(int frames,long epoch,long sequence)throws IOException{return NativeMixer.render(id,frames,epoch,sequence);}
                        public byte[] snapshot()throws IOException{byte[] bytes=NativeMixer.sceneSnapshot(id);currentCapture.position(ScenePoolStatus.decode(NativeMixer.sceneStatus(id)).last);return bytes;}
                        public void close()throws IOException{try{Log.i("RCNativeAudio","pool session="+token+" "+ScenePoolStatus.decode(NativeMixer.sceneStatus(id)));}finally{NativeMixer.release(id);}}
                    };
                },()->token!=generation,()->{
                    if(!progress.prepared())throw new IOException("Audiostart bereits abgebrochen");
                    if(token==generation)loadedGeneration=token;
                },currentCapture);
            }catch(Throwable error){
                if(token==generation){failure="Missionsaudio: "+error.getMessage();target.pump.producerFailed(error);Log.e("RCNativeAudio",failure,error);}
                else Log.w("RCNativeAudio","Audiofortsetzung verworfen",error);
            }finally{if(token==generation)loadedGeneration=-1;}

        });
        Log.i("RCNativeAudio","session="+token+" bank="+count);
    }
    public void suspend(){generation++;suspended=true;loadedGeneration=-1;if(mailbox!=null){mailbox.seal();mailbox=null;}if(output!=null){output.close();output=null;}}
    byte[] bankHash(){return bankHash.clone();}
    CompletableFuture<SceneAudioCapture.Result> suspendForCheckpoint(){CompletableFuture<SceneAudioCapture.Result> result=closed || capture==null?CompletableFuture.completedFuture(null):capture.request();suspend();return result;}
    void restoreAudio(byte[] bytes,long cursor){if(!suspended || closed || cursor<0)throw new IllegalStateException("Audio restore target");resumeCache.clear();resumeCache.keep(resumeCache.version(),bytes);lastSequence=cursor;expectedRestoreCursor=cursor;capture=new SceneAudioCapture();capture.position(cursor);capture.finish(bytes);}
    public void stop(){capture=null;expectedRestoreCursor=null;resumeCache.clear();suspend();}
    public void reset(){boolean active=!suspended;capture=null;expectedRestoreCursor=null;resumeCache.clear();suspend();lastSequence=0;failure=null;if(active)resume();}
    public void close(){if(closed)return;closed=true;resumeCache.clear();suspend();producer.shutdown();}
}
