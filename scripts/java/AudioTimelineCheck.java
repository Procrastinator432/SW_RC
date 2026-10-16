package org.rcport.diagnostic;
public final class AudioTimelineCheck {
    static void eq(long a,long b){if(a!=b)throw new AssertionError(a+" != "+b);}
    public static void main(String[] args){
        AudioTimeline t=new AudioTimeline();
        // Three events waited for a delayed bank. Readiness must not bunch them.
        eq(t.delay(60,8000),0);t.played(60,8000);
        eq(t.delay(120,8000),1000);eq(t.delay(120,8999),1);eq(t.delay(120,9000),0);t.played(120,9000);
        eq(t.delay(180,9000),1000);eq(t.delay(180,10000),0);t.played(180,15000);
        eq(t.delay(180,15000),0);eq(t.delay(240,18000),0);
        t.reset();eq(t.delay(0,18000),0);
        System.out.println("{\"passed\":true,\"delayed_bank_preserves_event_tick_gaps\":true,\"pause_restart_clears_old_deadline\":true}");
    }
}
