package org.rcport.diagnostic;

import java.io.IOException;
import java.util.function.BooleanSupplier;

/** Production mission producer, independent of Android so its lifecycle can be exercised on the host. */
final class SceneAudioProducer {
    interface Source extends AutoCloseable {
        void restore(byte[] bytes)throws Exception;
        void mix(byte[] bytes)throws Exception;
        void consume(long cursor)throws Exception;
        void update(byte[] events,byte[] loops)throws Exception;
        byte[] render(int frames,long epoch,long sequence)throws Exception;
        byte[] snapshot()throws Exception;
        void close()throws Exception;
    }
    interface Factory {Source open(long epoch)throws Exception;}
    interface Prepared {void ready()throws Exception;}
    static final class Startup {
        static final long LIMIT_NANOS=20_000_000_000L;
        private final long started;
        private boolean prepared,expired;
        Startup(long now){started=now;}
        synchronized boolean prepared(){if(expired)return false;prepared=true;return true;}
        synchronized void check(long now)throws IOException{
            if(!prepared && now-started>=LIMIT_NANOS)expired=true;
            if(expired)throw new IOException("Der Missions-Audiostart dauert zu lange. Bitte die Mission erneut laden.");
        }
    }
    private SceneAudioProducer(){}
    static void produce(PcmPump pump,SceneAudioMailbox mailbox,SceneResumeCache cache,long version,
                        Factory factory,BooleanSupplier cancelled,Prepared prepared)throws Exception{
        produce(pump,mailbox,cache,version,factory,cancelled,prepared,new SceneAudioCapture());
    }
    static void produce(PcmPump pump,SceneAudioMailbox mailbox,SceneResumeCache cache,long version,
                        Factory factory,BooleanSupplier cancelled,Prepared prepared,SceneAudioCapture capture)throws Exception{
        Source source=null;boolean healthy=false;
        try{
            if(cancelled.getAsBoolean())return;
            source=factory.open(pump.epoch());
            if(cancelled.getAsBoolean())return;
            boolean restored=false;
            byte[] saved=cache.read(version);
            if(saved!=null){
                try{source.restore(saved);restored=true;}
                catch(Exception invalid){
                    cache.keep(version,null);Source rejected=source;source=null;rejected.close();
                    if(cancelled.getAsBoolean())return;
                    source=factory.open(pump.epoch());
                }
            }
            if(cancelled.getAsBoolean())return;
            // A gameplay-only cold load already consumed earlier event IDs.
            // Seed that cursor before new events; a valid sidecar owns its own cursor.
            if(!restored && mailbox.initialCursor()>0)source.consume(mailbox.initialCursor());
            byte[] initialMix=mailbox.takeMix();if(initialMix!=null)source.mix(initialMix);
            prepared.ready();healthy=true;
            while(!cancelled.getAsBoolean() && pump.isPlaying()){
                byte[] mix=mailbox.takeMix();if(mix!=null)source.mix(mix);
                // Preparing native resources must not consume a restored sentence
                // or start the PCM clock before the first current gameplay pose.
                if(!mailbox.hasLoopState()){Thread.sleep(5);continue;}
                PcmPump.Ticket ticket=pump.reserve(480);
                if(ticket==null){Thread.sleep(5);continue;}
                if(cancelled.getAsBoolean()){pump.cancel(ticket);return;}
                SceneAudioMailbox.Batch batch=mailbox.poll();
                if(batch!=null)source.update(batch.events,batch.loops);
                if(cancelled.getAsBoolean()){pump.cancel(ticket);return;}
                PcmPacket packet=PcmPacket.decode(source.render(480,ticket.epoch,ticket.sequence));
                if(!pump.commit(ticket,packet)){
                    if(pump.failure()!=null)throw new IOException(pump.failure());
                    return;
                }
            }
            if(pump.failure()!=null)throw new IOException(pump.failure());
        }catch(Throwable failure){
            healthy=false;cache.keep(version,null);
            if(!cancelled.getAsBoolean())pump.producerFailed(failure);
            throw failure;
        }finally{
            byte[] paused=null;boolean clean=false;
            try {
            if(source!=null){
                try{
                    if(healthy && cancelled.getAsBoolean() && pump.failure()==null && (version==cache.version() || capture.needed())){
                        // Updates accepted before pause and packets still in the
                        // mailbox join the same native cursor before its snapshot.
                        SceneAudioMailbox.Pause pause=mailbox.pause();
                        for(SceneAudioMailbox.Batch batch:pause.dialogs)source.update(batch.events,batch.loops);
                        if(pause.receipt>=0)source.consume(pause.receipt);
                        paused=source.snapshot();cache.keep(version,paused);
                    }
                }catch(Throwable failure){cache.keep(version,null);throw failure;}
                finally{
                    try{source.close();}
                    catch(Throwable failure){cache.keep(version,null);throw failure;}
                }
            }
            clean=true;
            } finally {capture.finish(clean?paused:null);}
        }
    }
}
