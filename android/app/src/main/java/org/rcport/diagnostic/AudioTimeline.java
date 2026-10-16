package org.rcport.diagnostic;

/** Preserve simulation-tick spacing when asynchronous sample loading queues events. */
final class AudioTimeline {
    private long lastTick=-1,lastMillis;
    long delay(long tick,long now) {
        if(lastTick<0)return 0;
        long spacing=Math.round(Math.max(0,tick-lastTick)*1000.0/60.0);
        return Math.max(0,lastMillis+spacing-now);
    }
    void played(long tick,long now){lastTick=tick;lastMillis=now;}
    void reset(){lastTick=-1;}
}
