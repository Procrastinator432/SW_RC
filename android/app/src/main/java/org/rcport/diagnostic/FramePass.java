package org.rcport.diagnostic;

/** Frame 003: opaque actors, ordered world effects, then a depth-cleared weapon. */
final class FramePass {
    static int validate(int pass,int blend,int texture,int previous,boolean extended) {
        if(pass<previous || pass<0 || pass>(extended ? 2:1) || texture < -1 || texture>=1024)
            throw new IllegalArgumentException("Invalid frame pass");
        if(extended && ((pass==1 && (blend<1 || blend>2 || texture<0)) || (pass!=1 && blend!=0)))
            throw new IllegalArgumentException("Invalid effect blend");
        return !extended && pass==1 ? 2:pass;
    }
}
