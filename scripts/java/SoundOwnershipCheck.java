package org.rcport.diagnostic;
public final class SoundOwnershipCheck {
    static void eq(int a,int b){if(a!=b)throw new AssertionError(a+" != "+b);}
    public static void main(String[] args){
        SoundOwnership p=new SoundOwnership();
        p.played(523,1,17);p.played(530,1,19);
        eq(p.replaced(523,2),0); // SFX cannot stop the actor's voice.
        eq(p.replaced(523,1),17); // An admitted replacement must stop the old stream.
        eq(p.replaced(530,1),19); // Independent owner.
        p.played(523,1,20);eq(p.activeStreams().length,1);eq(p.activeStreams()[0],20);
        p.clear();eq(p.activeStreams().length,0);eq(p.replaced(523,1),0);
        p.played(523,1,0);eq(p.replaced(523,1),0);
        System.out.println("{\"passed\":true,\"voice_replacement_owner_scoped\":true,\"sfx_independent\":true,\"pause_clears_ownership\":true}");
    }
}
