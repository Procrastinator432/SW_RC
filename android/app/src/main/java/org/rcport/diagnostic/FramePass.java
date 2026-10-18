package org.rcport.diagnostic;

/** Frame 003: opaque actors, ordered world effects, then a depth-cleared weapon. */
final class FramePass {
    static int validate(int pass,int blend,int texture,int previous,boolean extended) {
        if(pass<previous || pass<0 || pass>(extended ? 2:1) || texture < -1 || texture>=1024)
            throw new IllegalArgumentException("Invalid frame pass");
        int mode=blend&255;
        if(extended && ((pass==1 && (mode<1 || mode>5 || texture<0 || (blend&~0xffff070f)!=0 || (blend&0x200)==0 && (blend&0x00ff0000)!=0 || (blend&0x400)==0 && (blend&0xff000000)!=0 || (blend&0x400)!=0 && mode<3)) || (pass!=1 && blend!=0)))
            throw new IllegalArgumentException("Invalid effect blend");
        return !extended && pass==1 ? 2:pass;
    }
}
