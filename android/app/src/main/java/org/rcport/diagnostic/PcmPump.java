package org.rcport.diagnostic;

import java.util.ArrayDeque;

/** One owner thread for every sink call. Admission never blocks the UI or invokes the sink. */
final class PcmPump implements AutoCloseable {
    interface Sink extends AutoCloseable {
        void play(); void pauseAndFlush(); int write(short[] data,int offset,int count);
        default boolean drained(){return true;}
        @Override void close();
    }
    interface Factory { Sink open(int rate); }
    static final class Ticket {
        private final PcmPump owner;
        final long epoch,sequence;
        final int frames;
        private Ticket(PcmPump owner,long epoch,long sequence,int frames){this.owner=owner;this.epoch=epoch;this.sequence=sequence;this.frames=frames;}
    }
    private static final class Entry {
        final PcmPacket packet;final short[] samples;int offset;
        Entry(PcmPacket p){packet=p;samples=p.copySamples();}
    }
    private final ArrayDeque<Entry> queue=new ArrayDeque<>();
    private final Thread worker;
    private final int rate;
    private final int capacityFrames;
    private long epoch=1,nextSequence;
    private int queuedFrames;
    private boolean playing,closed,finishing;
    private String failure;
    private Ticket reservation;
    PcmPump(int rate,Factory factory){
        this(rate,factory,rate/4);
    }
    PcmPump(int rate,Factory factory,int capacityFrames){
        if(rate<8000 || rate>192000)throw new IllegalArgumentException("PCM-Rate");
        if(capacityFrames<1 || capacityFrames>rate/4)throw new IllegalArgumentException("PCM-Queuebudget");
        this.rate=rate;this.capacityFrames=capacityFrames;worker=new Thread(()->run(factory),"rc-pcm-output");worker.start();
    }
    synchronized long epoch(){return epoch;}
    synchronized String failure(){return failure;}
    synchronized int queuedFrames(){return queuedFrames;}
    synchronized boolean isPlaying(){return playing && !closed && failure==null;}
    synchronized boolean offer(PcmPacket packet){
        if(packet.frames()>capacityFrames)throw new IllegalArgumentException("PCM-Paket größer als Ausgabebudget");
        if(closed || failure!=null || !playing || finishing || reservation!=null || packet.rate!=rate || packet.epoch!=epoch || packet.sequence!=nextSequence)
            return false;
        if(queue.size()>=16 || packet.frames()>capacityFrames-queuedFrames)return false;
        queue.addLast(new Entry(packet));queuedFrames+=packet.frames();nextSequence++;notifyAll();return true;
    }
    synchronized Ticket reserve(int frames){
        if(frames<1 || frames>PcmPacket.MAX_FRAMES || frames>capacityFrames)throw new IllegalArgumentException("PCM-Rendergröße");
        if(closed || failure!=null || !playing || finishing || reservation!=null || nextSequence==Long.MAX_VALUE
            || queue.size()>=16 || frames>capacityFrames-queuedFrames)return null;
        reservation=new Ticket(this,epoch,nextSequence,frames);return reservation;
    }
    synchronized boolean commit(Ticket ticket,PcmPacket packet){
        if(ticket==null || ticket.owner!=this || reservation!=ticket)return false;
        reservation=null;
        if(packet.rate!=rate || packet.epoch!=ticket.epoch || packet.sequence!=ticket.sequence || packet.frames()!=ticket.frames){
            fail(new IllegalArgumentException("PCM-Renderergebnis passt nicht zur Reservierung"));return false;
        }
        return offer(packet);
    }
    synchronized int reservedFrames(){return reservation==null?0:reservation.frames;}
    synchronized void cancel(Ticket ticket){if(reservation==ticket){reservation=null;notifyAll();}}
    synchronized void producerFailed(Throwable error){fail(error);}
    synchronized void start(){if(!closed && failure==null){playing=true;notifyAll();}}
    synchronized void finish(){finishing=true;reservation=null;notifyAll();}
    synchronized void pause(){
        if(closed || failure!=null)return;
        if(epoch==Long.MAX_VALUE){failure="PCM-Sitzungszähler erschöpft";closed=true;}
        else epoch++;
        playing=false;finishing=false;reservation=null;queue.clear();queuedFrames=0;nextSequence=0;notifyAll();
    }
    @Override public synchronized void close(){closed=true;playing=false;reservation=null;queue.clear();queuedFrames=0;notifyAll();}
    boolean awaitStopped(long millis)throws InterruptedException{worker.join(millis);return !worker.isAlive();}
    private synchronized void fail(Throwable error){failure=error.toString();closed=true;playing=false;reservation=null;queue.clear();queuedFrames=0;notifyAll();}
    private void run(Factory factory){
        Sink sink=null;long appliedEpoch=0;boolean active=false;
        try {
            sink=factory.open(rate);
            while(true){
                Entry entry;long generation;boolean desired,ending;
                synchronized(this){
                    if(closed)return;
                    generation=epoch;desired=playing;entry=queue.peekFirst();ending=finishing;
                    if(generation==appliedEpoch && desired==active && (!desired || (entry==null && !ending))){wait();continue;}
                }
                // Epoch changes force a flush even if pause/start both occurred before this snapshot.
                if(generation!=appliedEpoch || (!desired && active)){sink.pauseAndFlush();active=false;appliedEpoch=generation;}
                if(desired && !active){sink.play();active=true;}
                if(!desired)continue;
                if(entry==null){
                    if(!ending)continue;
                    boolean drained=sink.drained();
                    synchronized(this){if(epoch!=generation)continue;if(drained){close();return;}wait(10);}
                    continue;
                }
                // A pause may race this nonblocking write; the next epoch barrier flushes it before new PCM.
                int requested=entry.samples.length-entry.offset;
                int written=sink.write(entry.samples,entry.offset,requested);
                if(written<0 || written>requested || written%2!=0)throw new IllegalStateException("PCM-Schreibfehler: "+written);
                synchronized(this){
                    if(closed)return;
                    if(epoch!=generation || queue.peekFirst()!=entry)continue;
                    entry.offset+=written;queuedFrames-=written/2;
                    if(entry.offset==entry.samples.length)queue.removeFirst();
                    notifyAll();
                    if(written==0)wait(10);
                }
            }
        }catch(Throwable error){fail(error);}
        finally{if(sink!=null)try{sink.close();}catch(Throwable error){fail(error);}}
    }
}
