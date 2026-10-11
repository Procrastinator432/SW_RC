package org.rcport.inputprobe;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import android.os.SystemClock;
import android.util.Base64;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.KeyEvent;
import org.json.JSONObject;

/** Separate test APK: injects real Android touch events, never calls game APIs. */
public final class InputProbe extends Instrumentation {
    private Bundle arguments;
    private long downTime;
    @Override public void onCreate(Bundle args) {super.onCreate(args); arguments=args; start();}
    private void emit(int action, float[][] points) {
        int[] ids={2,7,19,23};
        MotionEvent.PointerProperties[] props=new MotionEvent.PointerProperties[points.length];
        MotionEvent.PointerCoords[] coords=new MotionEvent.PointerCoords[points.length];
        for(int i=0;i<points.length;i++) {
            props[i]=new MotionEvent.PointerProperties(); props[i].id=ids[i]; props[i].toolType=MotionEvent.TOOL_TYPE_FINGER;
            coords[i]=new MotionEvent.PointerCoords(); coords[i].x=points[i][0]; coords[i].y=points[i][1]; coords[i].pressure=1; coords[i].size=0.1f;
        }
        MotionEvent event=MotionEvent.obtain(downTime,SystemClock.uptimeMillis(),action,points.length,props,coords,0,0,1,1,0,0,InputDevice.SOURCE_TOUCHSCREEN,0);
        boolean accepted=getUiAutomation().injectInputEvent(event,true); event.recycle();
        if(!accepted) throw new IllegalStateException("Touch injection rejected");
    }
    @Override public void onStart() {
        Bundle result=new Bundle();
        try {
            JSONObject g=new JSONObject(new String(Base64.decode(arguments.getString("geometry"),Base64.DEFAULT),"UTF-8"));
            float left=(float)g.getDouble("left"),top=(float)g.getDouble("top"),w=(float)g.getDouble("width"),h=(float)g.getDouble("height");
            float r=Math.min(w,h)*0.145f;
            float[] move={left+r*1.6f,top+h-r*1.7f};
            float[] look={left+w*0.60f,top+h*0.35f};
            float[] fire={left+w-r*1.3f,top+h-r*1.7f};
            float[] jump={left+w-r*3.1f,top+h-r*1.3f};
            downTime=SystemClock.uptimeMillis();
            if("release".equals(arguments.getString("mode"))) {
                emit(MotionEvent.ACTION_DOWN,new float[][]{look});
                emit(MotionEvent.ACTION_UP,new float[][]{{left+w*.70f,top+h*.40f}});
                emit(MotionEvent.ACTION_DOWN,new float[][]{move});
                emit(MotionEvent.ACTION_POINTER_DOWN|(1<<MotionEvent.ACTION_POINTER_INDEX_SHIFT),new float[][]{move,look});
                emit(MotionEvent.ACTION_POINTER_UP|(1<<MotionEvent.ACTION_POINTER_INDEX_SHIFT),new float[][]{move,{left+w*.70f,top+h*.40f}});
                emit(MotionEvent.ACTION_UP,new float[][]{move});
                result.putString("result","FINAL_LOOK_RELEASE_INJECTED");finish(Activity.RESULT_OK,result);return;
            }
            emit(MotionEvent.ACTION_DOWN,new float[][]{move});
            emit(MotionEvent.ACTION_POINTER_DOWN|(1<<MotionEvent.ACTION_POINTER_INDEX_SHIFT),new float[][]{move,look});
            emit(MotionEvent.ACTION_POINTER_DOWN|(2<<MotionEvent.ACTION_POINTER_INDEX_SHIFT),new float[][]{move,look,fire});
            for(int i=0;i<24;i++) {
                move[1]=top+h-r*2.3f;
                look[0]=left+w*(0.60f+i*0.0015f);
                emit(MotionEvent.ACTION_MOVE,new float[][]{move,look,fire});
                if(i==8) {
                    emit(MotionEvent.ACTION_POINTER_DOWN|(3<<MotionEvent.ACTION_POINTER_INDEX_SHIFT),new float[][]{move,look,fire,jump});
                    SystemClock.sleep(80);
                    emit(MotionEvent.ACTION_POINTER_UP|(3<<MotionEvent.ACTION_POINTER_INDEX_SHIFT),new float[][]{move,look,fire,jump});
                }
                SystemClock.sleep(20);
            }
            if("focus".equals(arguments.getString("mode"))) {
                getUiAutomation().injectInputEvent(new KeyEvent(KeyEvent.ACTION_DOWN,KeyEvent.KEYCODE_HOME),true);
                getUiAutomation().injectInputEvent(new KeyEvent(KeyEvent.ACTION_UP,KeyEvent.KEYCODE_HOME),true);
                result.putString("result","HOME_DURING_HELD_INPUT");
            } else {
                emit(MotionEvent.ACTION_CANCEL,new float[][]{move,look,fire});
                result.putString("result","MULTITOUCH_AND_CANCEL_INJECTED");
            }
            result.putInt("maximum_pointers",4);
            finish(Activity.RESULT_OK,result);
        } catch(Exception error) {
            result.putString("error",error.toString()); finish(Activity.RESULT_CANCELED,result);
        }
    }
}
