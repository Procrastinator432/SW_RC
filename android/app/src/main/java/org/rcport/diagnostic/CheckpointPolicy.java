package org.rcport.diagnostic;

/** Simulation time, not wall time: pausing cannot manufacture a quiet period. */
final class CheckpointPolicy {
    private int tick=-1,health,shots,attacks,quietSince;
    private long safeMarker;
    private int safeTick=-1;
    void reset(){tick=-1;safeTick=-1;}
    boolean observe(int[] state,boolean eligible,long marker) {
        int now=state[5];
        if(tick<0 || now<tick || !eligible || state[0]<health || state[6]!=shots || state[9]!=attacks)quietSince=now;
        tick=now;health=state[0];shots=state[6];attacks=state[9];
        return eligible && now-quietSince>=300 && (safeTick<0 || marker!=safeMarker || now-safeTick>=1800);
    }
    void saved(int tick,long marker){safeTick=tick;safeMarker=marker;}
}
