package org.rcport.diagnostic;
import java.io.IOException;
import java.util.function.BooleanSupplier;
/** The finite diagnostic producer reserves output capacity before advancing any native clock. */
final class PcmLiveProducer {
    interface Source extends AutoCloseable {
        byte[] render(int frames,long epoch,long sequence)throws Exception;
        default int remainingFrames(){return -1;}
        @Override void close()throws Exception;
    }
    interface Factory { Source open(long epoch)throws Exception; }
    private PcmLiveProducer(){}
    static void produce(PcmPump pump,Factory factory,int totalFrames,int chunkFrames,BooleanSupplier cancelled)throws Exception {
        if(totalFrames<1 || chunkFrames<1 || chunkFrames>PcmPacket.MAX_FRAMES)throw new IllegalArgumentException("Live PCM frame limit");
        final long epoch=pump.epoch();
        if(cancelled.getAsBoolean() || !pump.isPlaying())return;
        try(Source source=factory.open(epoch)){
            int available=source.remainingFrames();
            int remaining=available<0?totalFrames:Math.min(totalFrames,available);
            while(remaining>0){
                if(cancelled.getAsBoolean() || !pump.isPlaying() || pump.epoch()!=epoch)return;
                int frames=Math.min(remaining,chunkFrames);
                PcmPump.Ticket ticket=pump.reserve(frames);
                if(ticket==null){Thread.sleep(10);continue;}
                if(cancelled.getAsBoolean() || !pump.isPlaying() || ticket.epoch!=epoch){pump.cancel(ticket);return;}
                PcmPacket packet;
                try{packet=PcmPacket.decode(source.render(frames,ticket.epoch,ticket.sequence));}
                catch(Exception error){pump.producerFailed(error);throw error;}
                if(!pump.commit(ticket,packet)){
                    if(pump.failure()!=null)throw new IOException(pump.failure());return;
                }
                remaining-=frames;
            }
            pump.finish();
        }
    }
}
