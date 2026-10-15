package org.rcport.diagnostic;
public final class TouchLayoutCheck {
    public static void main(String[] args){
        int checked=0;
        for(float density:new float[]{1,1.5f,2,3})for(int[] size:new int[][]{{480,192},{640,312},{640,336},{800,400}}){
            float w=size[0]*density,h=size[1]*density;TouchLayout l=new TouchLayout(w,h,density);
            for(TouchLayout.Key k:l.keys){
                if(k.r*2<48*density || k.x-k.r<0 || k.y-k.r<0 || k.x+k.r>w || k.y+k.r>h || !k.contains(k.x,k.y))throw new AssertionError("Clipped/small touch action");
                for(TouchLayout.Key other:l.keys)if(k!=other && Math.hypot(k.x-other.x,k.y-other.y)<k.r+other.r)throw new AssertionError("Overlapping actions");
                if(Math.hypot(k.x-l.joystickX,k.y-l.joystickY)<k.r+l.joystickRadius)throw new AssertionError("Action overlaps joystick");
            }checked++;
        }
        float base=TouchLayout.look(80,1,1);
        if(Math.abs(base-TouchLayout.look(160,2,1))>.00001 || Math.abs(base*2-TouchLayout.look(80,1,2))>.00001 || Math.abs(base*.5-TouchLayout.look(80,1,.5f))>.00001)throw new AssertionError("Density/sensitivity scaling drift");
        System.out.println("{\"passed\":true,\"layout_variants\":"+checked+",\"minimum_48dp\":true,\"no_clipping_or_overlap\":true,\"density_independent_look\":true,\"sensitivity_linear\":true}");
    }
}
