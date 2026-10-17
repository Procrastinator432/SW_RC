package org.rcport.diagnostic;
public final class FramePassCheck {
    static void check(boolean condition) {if(!condition) throw new AssertionError();}
    static void invalid(int pass,int blend,int texture,int previous,boolean extended) {
        try {FramePass.validate(pass,blend,texture,previous,extended);}
        catch(IllegalArgumentException expected) {return;}
        throw new AssertionError("Accepted invalid pass");
    }
    public static void main(String[] args) {
        check(FramePass.validate(0,0,-1,0,false)==0);
        check(FramePass.validate(1,0,0,0,false)==2);
        check(FramePass.validate(0,0,0,0,true)==0);
        check(FramePass.validate(1,1,0,0,true)==1);
        check(FramePass.validate(1,2,1023,1,true)==1);
        check(FramePass.validate(2,0,-1,1,true)==2);
        invalid(0,1,0,0,true);invalid(1,0,0,0,true);
        invalid(1,3,0,0,true);invalid(1,1,-1,0,true);
        invalid(2,1,0,1,true);invalid(1,1,0,2,true);
        invalid(3,0,0,0,true);invalid(2,0,0,0,false);
        invalid(0,0,1024,0,true);invalid(0,0,-2,0,true);
        System.out.println("FramePass checks passed");
    }
}
