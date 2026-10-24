package org.rcport.diagnostic;

import android.content.Context;
import android.media.*;
import android.os.*;

/** Main-thread focus owner; AudioTrack itself belongs exclusively to PcmPump's worker. */
final class PcmTrackOutput implements AutoCloseable {
    final PcmPump pump;
    private final AudioManager manager;
    private final AudioFocusRequest focus;
    private boolean closed;
    PcmTrackOutput(Context context,int rate,Runnable onLoss){
        this(context,rate,onLoss,false);
    }
    PcmTrackOutput(Context context,int rate,Runnable onLoss,boolean lowLatency){
        manager=(AudioManager)context.getSystemService(Context.AUDIO_SERVICE);
        AudioAttributes attributes=new AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_GAME)
            .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build();
        pump=new PcmPump(rate,frequency->new Track(frequency,attributes,lowLatency),lowLatency?rate/50:rate/4);
        focus=new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT)
            .setAudioAttributes(attributes).setWillPauseWhenDucked(true)
            .setOnAudioFocusChangeListener(change->{
                if(!closed && change!=AudioManager.AUDIOFOCUS_GAIN){pump.pause();manager.abandonAudioFocusRequest(focusRequest());onLoss.run();}
            },new Handler(Looper.getMainLooper())).build();
    }
    private AudioFocusRequest focusRequest(){return focus;}
    boolean start(){
        if(closed || pump.failure()!=null)return false;
        if(manager.requestAudioFocus(focus)!=AudioManager.AUDIOFOCUS_REQUEST_GRANTED)return false;
        pump.start();return true;
    }
    void pause(){pump.pause();manager.abandonAudioFocusRequest(focus);}
    @Override public void close(){if(!closed){closed=true;pump.close();manager.abandonAudioFocusRequest(focus);}}
    private static final class Track implements PcmPump.Sink {
        private final AudioTrack track;
        private final short[] drainSilence;
        private long writtenFrames;
        private int paddingOffset;
        Track(int rate,AudioAttributes attributes,boolean lowLatency){
            int minimum=AudioTrack.getMinBufferSize(rate,AudioFormat.CHANNEL_OUT_STEREO,AudioFormat.ENCODING_PCM_16BIT);
            if(minimum<=0)throw new IllegalStateException("AudioTrack-Format nicht unterstützt: "+minimum);
            track=new AudioTrack.Builder().setAudioAttributes(attributes)
                .setAudioFormat(new AudioFormat.Builder().setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                    .setSampleRate(rate).setChannelMask(AudioFormat.CHANNEL_OUT_STEREO).build())
                .setTransferMode(AudioTrack.MODE_STREAM).setPerformanceMode(lowLatency?AudioTrack.PERFORMANCE_MODE_LOW_LATENCY:AudioTrack.PERFORMANCE_MODE_NONE)
                .setBufferSizeInBytes(Math.max(minimum,rate/(lowLatency?50:10)*4)).build();
            if(track.getState()!=AudioTrack.STATE_INITIALIZED){track.release();throw new IllegalStateException("AudioTrack nicht bereit");}
            drainSilence=new short[track.getBufferSizeInFrames()*2];
        }
        public void play(){track.play();}
        public void pauseAndFlush(){track.pause();track.flush();writtenFrames=0;paddingOffset=0;}
        public int write(short[] data,int offset,int count){int result=track.write(data,offset,count,AudioTrack.WRITE_NON_BLOCKING);if(result>0)writtenFrames+=result/2;return result;}
        public boolean drained(){
            // A short final clip may not fill the platform's start threshold (also on API 26).
            // Pad with at most one hardware buffer of silence, outside the software queue budget.
            if(paddingOffset<drainSilence.length){
                int remaining=drainSilence.length-paddingOffset;
                int n=track.write(drainSilence,paddingOffset,remaining,AudioTrack.WRITE_NON_BLOCKING);
                if(n<0 || n>remaining || n%2!=0)throw new IllegalStateException("PCM-Endpuffer: "+n);
                paddingOffset+=n;
            }
            return Integer.toUnsignedLong(track.getPlaybackHeadPosition())>=writtenFrames;
        }
        public void close(){track.release();}
    }
}
