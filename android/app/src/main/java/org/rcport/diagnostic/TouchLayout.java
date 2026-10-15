package org.rcport.diagnostic;

/** Density-independent touch geometry shared by drawing, hit testing and checks. */
final class TouchLayout {
    static final class Key {
        final int role;final String label;final float x,y,r;
        Key(int role,String label,float x,float y,float r){this.role=role;this.label=label;this.x=x;this.y=y;this.r=r;}
        boolean contains(float px,float py){return Math.hypot(px-x,py-y)<=r;}
    }
    final float joystickX,joystickY,joystickRadius;
    final Key[] keys;
    TouchLayout(float width,float height,float density){
        float r=Math.max(24*density,Math.min(32*density,Math.min(height*.105f,width*.045f)));
        float stride=r*2+8*density,right=width-r*1.12f-12*density,bottom=height-r*1.12f-12*density;
        keys=new Key[]{new Key(2,"FEUER",right,bottom,r*1.12f),new Key(3,"SPRUNG",right-stride,bottom,r),new Key(4,"LADEN",right,bottom-stride,r),new Key(6,"NUTZEN",right-stride,bottom-stride,r),new Key(7,"GRANATE",right,bottom-2*stride,r),new Key(8,"DUCKEN",right-stride,bottom-2*stride,r)};
        joystickRadius=Math.max(42*density,Math.min(64*density,Math.min(height*.21f,width*.12f)));
        joystickX=12*density+joystickRadius;joystickY=height-12*density-joystickRadius;
    }
    static float look(float pixels,float density,float sensitivity){return pixels/density*(3f/640f)*Math.max(.5f,Math.min(2f,sensitivity));}
}
