package org.rcport.diagnostic;
import java.util.concurrent.*;

/** One output session's completed pause result, independent of RAM-cache invalidation. */
final class SceneAudioCapture {
    private static final ScheduledExecutorService TIMER=Executors.newSingleThreadScheduledExecutor(r->{Thread t=new Thread(r,"rc-audio-capture-timeout");t.setDaemon(true);return t;});
    static final class Result {
        final long cursor;private final byte[] bytes;
        Result(long cursor,byte[] bytes){if(cursor<0 || bytes==null || bytes.length>256*1024)throw new IllegalArgumentException("Audio capture");this.cursor=cursor;this.bytes=bytes.clone();}
        byte[] bytes(){return bytes.clone();}
    }
    private final CompletableFuture<Result> result=new CompletableFuture<>();
    private final long timeoutMillis;private boolean requested;private long cursor;
    SceneAudioCapture(){this(3000);}
    SceneAudioCapture(long timeoutMillis){if(timeoutMillis<0)throw new IllegalArgumentException();this.timeoutMillis=timeoutMillis;}
    synchronized CompletableFuture<Result> request(){
        if(!requested){requested=true;if(!result.isDone())TIMER.schedule(()->result.complete(null),timeoutMillis,TimeUnit.MILLISECONDS);}
        return result;
    }
    synchronized boolean needed(){return requested && !result.isDone();}
    synchronized void position(long cursor){if(cursor<0)throw new IllegalArgumentException();this.cursor=cursor;}
    synchronized void finish(byte[] bytes){result.complete(bytes==null?null:new Result(cursor,bytes));}
}
