package org.rcport.diagnostic;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.graphics.*;
import android.media.AudioManager;
import android.media.ToneGenerator;
import android.os.*;
import android.util.Log;
import android.view.*;
import android.widget.*;
import java.io.*;
import java.util.*;
import java.util.concurrent.*;

/** Interactive host for the shared Rust vertical slice; native Rust owns all game state. */
public final class PlayActivity extends Activity {
    static { System.loadLibrary("rc_bridge"); }
    private static native long createGame(byte[] data) throws IOException;
    private static native int[] gameFrame(long id,double elapsed,float forward,float strafe,
        float lookX,float lookY,int buttons,int width,int height,long[] metrics) throws IOException;
    private static native byte[] gameAudio(long id,int kind) throws IOException;
    private static native byte[] gpuWorld(long id) throws IOException;
    private static native byte[] gpuFrame(long id,double elapsed,float forward,float strafe,float lookX,float lookY,int buttons,long[] metrics) throws IOException;
    private static native void gameCommand(long id,int command) throws IOException;
    private static native int[] jumpStatus(long id) throws IOException;
    private static native String actorStatus(long id) throws IOException;
    private static native String missionStatus(long id) throws IOException;
    private String lastJumpState=""; // worker-owned observation only
    private static native void releaseGame(long id);
    private final ExecutorService worker=Executors.newSingleThreadExecutor();
    private final Handler handler=new Handler(Looper.getMainLooper());
    private final Runnable pump=() -> queueFrame(false);
    private long game; // worker only
    private boolean loaded,paused,resumed,drawing,finished;
    private int generation,lastShots,lastHP=100,lastPhase,lastEnemies=3,lastAmmo=20;
    private long lastFrame;
    private ImageView image;
    private GpuSceneView gpu;
    private boolean useGpu;
    private Button backend;
    private Controls controls;
    private TextView status;
    private Button open,pause;
    private Bitmap bitmap;
    private ToneGenerator audio;
    private SceneAudio sceneAudio;
    private boolean profiling;
    private long profileSequence;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        profiling=getIntent().getBooleanExtra("rcProfile",false);
        android.app.ActivityManager manager=(android.app.ActivityManager)getSystemService(ACTIVITY_SERVICE);
        useGpu=manager.getDeviceConfigurationInfo().reqGlEsVersion>=0x20000 && !getIntent().getBooleanExtra("rcSoftware",false);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_FULLSCREEN
            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY);
        try { audio=new ToneGenerator(AudioManager.STREAM_MUSIC,22); } catch(RuntimeException ignored) {}
        LinearLayout root=new LinearLayout(this); root.setOrientation(LinearLayout.VERTICAL); root.setBackgroundColor(Color.rgb(16,25,35));
        LinearLayout bar=new LinearLayout(this);
        open=button(bar,"Szene öffnen",() -> {
            Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT); intent.setType("*/*");
            intent.addCategory(Intent.CATEGORY_OPENABLE); startActivityForResult(intent,1);
        });
        pause=button(bar,"Pause",this::togglePause);
        button(bar,"Neustart",this::restart);
        backend=button(bar,useGpu ? "GPU" : "CPU",() -> switchBackend(!useGpu));
        button(bar,"Zurück",this::finish);
        status=new TextView(this); status.setTextColor(Color.rgb(190,230,224)); status.setTextSize(12);
        status.setText("geo_01a.rcplay öffnen · links bewegen, rechts zielen");
        bar.addView(status,new LinearLayout.LayoutParams(0,-1,1)); root.addView(bar);
        FrameLayout frame=new FrameLayout(this);
        image=new ImageView(this); image.setScaleType(ImageView.ScaleType.FIT_CENTER);
        image.setContentDescription("Spielbare Szene geo_01a"); frame.addView(image,new FrameLayout.LayoutParams(-1,-1));
        gpu=new GpuSceneView(this,profiling,getIntent().getBooleanExtra("rcGpuCapture",false),message -> {
            if(isDestroyed()) return;
            switchBackend(false); Log.e("RCPlay","GPU fallback: "+message);
        });
        gpu.setVisibility(useGpu ? View.VISIBLE : View.GONE);image.setVisibility(useGpu ? View.GONE : View.VISIBLE);
        frame.addView(gpu,new FrameLayout.LayoutParams(-1,-1));
        controls=new Controls(); frame.addView(controls,new FrameLayout.LayoutParams(-1,-1));
        root.addView(frame,new LinearLayout.LayoutParams(-1,0,1)); setContentView(root);
        if(getIntent().getData()!=null) loadScene(getIntent().getData());
    }
    private Button button(LinearLayout row,String title,Runnable action) {
        Button b=new Button(this); b.setText(title); b.setTextSize(12); b.setOnClickListener(v -> action.run()); row.addView(b); return b;
    }
    private void switchBackend(boolean enabled) {
        useGpu=enabled;generation++;controls.clear();lastFrame=0;
        if(sceneAudio!=null) sceneAudio.stop();
        gpu.setVisibility(useGpu ? View.VISIBLE : View.GONE);image.setVisibility(useGpu ? View.GONE : View.VISIBLE);
        backend.setText(useGpu ? "GPU" : "CPU");
        Log.i("RCPlay","backend="+(useGpu ? "gpu" : "cpu"));
        if(loaded) {worker.execute(() -> {try {gameCommand(game,0);} catch(Exception e) {failure(e.getMessage());}});queueFrame(true);}
    }
    private void togglePause() {
        if(!loaded || finished) return;
        paused=!paused; generation++; controls.clear(); lastFrame=0; handler.removeCallbacks(pump);
        pause.setText(paused ? "Fortsetzen" : "Pause");
        if(paused) {
            if(sceneAudio!=null) sceneAudio.stop();
            worker.execute(() -> {try {gameCommand(game,0);} catch(Exception e) {Log.e("RCPlay","pause",e);} });
            status.setText("Pausiert");
        } else {status.setText("Drei Gegner ausschalten, dann Exfil erreichen"); queueFrame(false);}
    }
    private void restart() {
        if(!loaded) return;
        generation++; controls.clear(); handler.removeCallbacks(pump); lastFrame=0; finished=false; paused=false;
        pause.setText("Pause"); status.setText("Neustart …");
        if(sceneAudio!=null) sceneAudio.reset();
        worker.execute(() -> {
            try { gameCommand(game,1); runOnUiThread(() -> {
                if(isDestroyed()) return; lastShots=0; lastHP=100; lastPhase=0; lastEnemies=3; lastAmmo=20;
                status.setText("Drei Gegner ausschalten, dann Exfil erreichen"); queueFrame(true);
            }); } catch(Exception e) {failure(e.getMessage());}
        });
    }
    @Override protected void onActivityResult(int request,int result,Intent data) {
        super.onActivityResult(request,result,data);
        if(request!=1 || result!=RESULT_OK || data==null || data.getData()==null) return;
        loadScene(data.getData());
    }
    private void loadScene(Uri uri) {
        loaded=false; generation++; controls.clear(); handler.removeCallbacks(pump); open.setEnabled(false);
        if(sceneAudio!=null) {sceneAudio.close();sceneAudio=null;}
        status.setText("Szene und Kollision werden geladen …"); image.setImageDrawable(null); gpu.world(null);
        worker.execute(() -> {
            long loadStart=System.nanoTime();
            releaseGame(game); game=0;
            try(InputStream input=getContentResolver().openInputStream(uri); ByteArrayOutputStream bytes=new ByteArrayOutputStream()) {
                if(input==null) throw new IOException("Datei konnte nicht geöffnet werden");
                byte[] block=new byte[32768]; int n;
                while((n=input.read(block))!=-1) {
                    if(bytes.size()+n>96*1024*1024) throw new IOException("Szene überschreitet 96 MiB");
                    bytes.write(block,0,n);
                }
                long decodeStart=System.nanoTime();
                game=createGame(bytes.toByteArray());
                final GpuSceneView.World world=new GpuSceneView.World(gpuWorld(game));
                final byte[] soundBank=gameAudio(game,0);
                Log.i("RCPlay","loaded geo_01a game="+game);
                if(profiling) Log.i("RCPerf","{\"type\":\"load\",\"read_ns\":"+(decodeStart-loadStart)+",\"create_ns\":"+(System.nanoTime()-decodeStart)+",\"bytes\":"+bytes.size()+"}");
                runOnUiThread(() -> {
                    if(isDestroyed()) return;
                    try {sceneAudio=new SceneAudio(this,soundBank);} catch(IOException ex) {failure(ex.getMessage());open.setEnabled(true);return;}
                    gpu.world(world); loaded=true; paused=false; finished=false; lastFrame=0;
                    lastShots=0; lastHP=100; lastPhase=0; lastEnemies=3; lastAmmo=20; pause.setText("Pause"); open.setEnabled(true);
                    status.setText("Drei Gegner ausschalten, dann Exfil erreichen"); controls.invalidate(); queueFrame(true);
                });
            } catch(Exception ex) {runOnUiThread(() -> {if(!isDestroyed()) {open.setEnabled(true); status.setText("Ladefehler: "+ex.getMessage());}});}
        });
    }
    private void failure(String message) {
        runOnUiThread(() -> {if(!isDestroyed()) {paused=true; controls.clear(); lastFrame=0;
            if(sceneAudio!=null) sceneAudio.stop();
            status.setText("Spielfehler: "+message); pause.setText("Fortsetzen");}});
    }
    private void queueFrame(boolean still) {
        if(!loaded || !resumed || isDestroyed() || drawing || (!still && (paused || finished))) return;
        drawing=true;
        long now=System.nanoTime(); final long interval=lastFrame==0 ? 0 : now-lastFrame; double elapsed=still || lastFrame==0 ? 0 : Math.min(0.25,(now-lastFrame)/1e9); lastFrame=now;
        final int requestedGeneration=generation; final boolean renderGpu=useGpu;
        final float forward=controls.forward(), strafe=controls.strafe(), lookX=controls.lookX, lookY=controls.lookY;
        final int buttons=controls.buttons(); controls.lookX=0; controls.lookY=0; controls.reload=false; controls.fireQueued=false; controls.jumpQueued=false;
        worker.execute(() -> {
            int[] pixels=null; GpuSceneView.Frame graphics=null; String error=null; byte[] soundEvents=null; String missionText=""; String actorText="[]";
            final long workStart=System.nanoTime();
            final long[] metrics=profiling ? new long[26] : null;
            try {
                if(renderGpu) {graphics=new GpuSceneView.Frame(gpuFrame(game,elapsed,forward,strafe,lookX,lookY,buttons,metrics),still);pixels=graphics.status;}
                else pixels=gameFrame(game,elapsed,forward,strafe,lookX,lookY,buttons,640,400,metrics);
                soundEvents=gameAudio(game,1);
                missionText=missionStatus(game);
                if(profiling) actorText=actorStatus(game);
                String jumpState=Arrays.toString(jumpStatus(game));
                if(!jumpState.equals(lastJumpState)) {Log.i("RCJump","ticks="+pixels[5]+" "+jumpState);lastJumpState=jumpState;}
            } catch(Exception ex) {error=ex.getMessage();}
            final long workEnd=System.nanoTime();
            final byte[] events=soundEvents;
            final String guidance=missionText;
            final String actorObservation=actorText;
            final int[] result=pixels; final String problem=error; final GpuSceneView.Frame graphicsFrame=graphics;
            runOnUiThread(() -> {
                drawing=false; if(isDestroyed()) return;
                if(generation!=requestedGeneration) {if(loaded) queueFrame(true); return;}
                if(problem!=null) {failure(problem); return;}
                long bitmapStart=System.nanoTime();
                if(renderGpu) gpu.submit(graphicsFrame);
                else {
                    if(bitmap==null) bitmap=Bitmap.createBitmap(640,400,Bitmap.Config.ARGB_8888);
                    bitmap.setPixels(result,10,640,0,0,640,400); image.setImageBitmap(bitmap);
                }
                long bitmapEnd=System.nanoTime();
                if(profiling) recordPerformance(result,metrics,now,workStart,workEnd,bitmapStart,bitmapEnd,interval,still,forward,strafe,lookX,lookY,buttons,actorObservation);
                if(sceneAudio!=null && !paused && resumed) {try {sceneAudio.events(events);} catch(IOException ex) {failure(ex.getMessage());return;}}
                if(audio!=null && (sceneAudio==null || !sceneAudio.hasOriginalSounds()) && result[6]>lastShots) audio.startTone(ToneGenerator.TONE_PROP_BEEP,35);
                else if(audio!=null && (sceneAudio==null || !sceneAudio.hasOriginalSounds()) && result[0]<lastHP) audio.startTone(ToneGenerator.TONE_PROP_NACK,50);
                if(result[0]!=lastHP || result[1]!=lastAmmo || result[3]!=lastEnemies || result[4]!=lastPhase || result[6]!=lastShots)
                    Log.i("RCPlay","state hp="+result[0]+" ammo="+result[1]+" alive="+result[3]+" phase="+result[4]+" ticks="+result[5]+" shots="+result[6]+" kills="+result[7]+" jumps="+result[8]);
                lastShots=result[6]; lastHP=result[0]; lastPhase=result[4]; lastEnemies=result[3]; lastAmmo=result[1];
                if(!guidance.isEmpty() && !paused) status.setText(guidance);
                if(result[4]!=0) {finished=true; controls.clear(); status.setText(result[4]==1 ? (guidance.isEmpty() ? "Abschnitt geschafft · Neustart möglich" : guidance) : "Gefallen · Neustart möglich");}
                if(loaded && resumed && !paused && !finished) handler.postDelayed(pump,renderGpu ? Math.max(1,16-(System.nanoTime()-now)/1000000) : 1);
            });
        });
    }
    private void recordPerformance(int[] state,long[] metrics,long queued,long started,long ended,
        long bitmapStart,long bitmapEnd,long interval,boolean still,float forward,float strafe,float lookX,float lookY,int buttons,String actorObservation) {
        try {
            org.json.JSONObject record=new org.json.JSONObject();
            record.put("backend",useGpu ? "gpu" : "cpu"); record.put("type","frame"); record.put("sequence",++profileSequence);
            record.put("interval_ns",interval); record.put("still",still);
            record.put("worker_wait_ns",started-queued); record.put("jni_wall_ns",ended-started);
            record.put("ui_wait_ns",bitmapStart-ended); record.put("bitmap_submit_ns",bitmapEnd-bitmapStart);
            record.put("submit_latency_ns",bitmapEnd-queued);
            org.json.JSONArray ns=new org.json.JSONArray(), calls=new org.json.JSONArray(), pose=new org.json.JSONArray();
            for(int i=0;i<10;i++) {ns.put(metrics[i]); calls.put(metrics[i+10]);}
            for(int i=20;i<25;i++) pose.put(Double.longBitsToDouble(metrics[i]));
            record.put("ns",ns); record.put("calls",calls); record.put("pose",pose);
            record.put("actors",new org.json.JSONArray(actorObservation));
            record.put("discarded_seconds",Double.longBitsToDouble(metrics[25]));
            record.put("forward",forward); record.put("strafe",strafe); record.put("lookX",lookX); record.put("lookY",lookY); record.put("buttons",buttons);
            record.put("ticks",state[5]); record.put("shots",state[6]); record.put("jumps",state[8]); record.put("phase",state[4]);
            record.put("java_heap_bytes",Runtime.getRuntime().totalMemory()-Runtime.getRuntime().freeMemory());
            record.put("native_heap_bytes",Debug.getNativeHeapAllocatedSize());
            Log.i("RCPerf",record.toString());
        } catch(org.json.JSONException error) {Log.e("RCPerf","Unable to serialize timing",error);}
    }
    @Override protected void onResume() {super.onResume(); gpu.onResume(); resumed=true; lastFrame=0; if(loaded) queueFrame(true);}
    @Override protected void onPause() {
        resumed=false; if(sceneAudio!=null) sceneAudio.stop(); handler.removeCallbacks(pump); lastFrame=0; if(controls!=null) controls.clear();
        worker.execute(() -> {if(game!=0) {try {gameCommand(game,0);} catch(Exception e) {Log.e("RCPlay","suspend",e);} }});
        gpu.onPause();super.onPause();
    }
    @Override protected void onDestroy() {
        loaded=false; handler.removeCallbacks(pump); worker.execute(() -> {releaseGame(game); game=0;}); worker.shutdown();
        if(sceneAudio!=null) sceneAudio.close();
        if(audio!=null) audio.release(); super.onDestroy();
    }
    @Override public boolean onKeyDown(int key,KeyEvent event) {
        if(controls.key(key,true)) return true; return super.onKeyDown(key,event);
    }
    @Override public boolean onKeyUp(int key,KeyEvent event) {
        if(controls.key(key,false)) return true; return super.onKeyUp(key,event);
    }
    private final class Controls extends View {
        private final Paint paint=new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Map<Integer,Integer> roles=new HashMap<>();
        private float dx,dy,lookX,lookY,lastX,lastY;
        private boolean fire,jump,reload,fireQueued,jumpQueued,w,s,a,d,walk,use;
        Controls() {super(PlayActivity.this); setContentDescription("Links Joystick; rechts Blick drehen; Feuer, Sprung und Laden"); setFocusable(true);}
        float radius() {return Math.min(getWidth(),getHeight())*0.145f;}
        float forward() {return Math.max(-1,Math.min(1,-dy+(w?1:0)-(s?1:0)));}
        float strafe() {return Math.max(-1,Math.min(1,dx+(d?1:0)-(a?1:0)));}
        int buttons() {return (jump||jumpQueued?1:0)|(fire||fireQueued?2:0)|(reload?4:0)|(walk?8:0)|(use?16:0);}
        void clear() {roles.clear(); dx=dy=lookX=lookY=0; fire=jump=reload=fireQueued=jumpQueued=w=s=a=d=walk=use=false; invalidate();}
        boolean key(int key,boolean down) {
            switch(key) {
                case KeyEvent.KEYCODE_W: case KeyEvent.KEYCODE_DPAD_UP:w=down;return true;
                case KeyEvent.KEYCODE_S: case KeyEvent.KEYCODE_DPAD_DOWN:s=down;return true;
                case KeyEvent.KEYCODE_A: case KeyEvent.KEYCODE_DPAD_LEFT:a=down;return true;
                case KeyEvent.KEYCODE_D: case KeyEvent.KEYCODE_DPAD_RIGHT:d=down;return true;
                case KeyEvent.KEYCODE_SPACE:jump=down;if(down) jumpQueued=true;return true;
                case KeyEvent.KEYCODE_CTRL_LEFT:fire=down;if(down) fireQueued=true;return true;
                case KeyEvent.KEYCODE_SHIFT_LEFT:walk=down;return true;
                case KeyEvent.KEYCODE_R:if(down) reload=true;return true;
                case KeyEvent.KEYCODE_E:use=down;return true;
                case KeyEvent.KEYCODE_ENTER:if(down) restart();return true;
                default:return false;
            }
        }
        private boolean inside(float x,float y,float cx,float cy,float r) {return Math.hypot(x-cx,y-cy)<r;}
        @Override public boolean onTouchEvent(MotionEvent e) {
            if(!loaded || paused || finished) return true;
            int action=e.getActionMasked(), index=e.getActionIndex(); float r=radius(), width=getWidth(), height=getHeight();
            if(action==MotionEvent.ACTION_CANCEL) {clear();return true;}
            if(action==MotionEvent.ACTION_DOWN || action==MotionEvent.ACTION_POINTER_DOWN) {
                int id=e.getPointerId(index); float x=e.getX(index),y=e.getY(index); int role;
                if(inside(x,y,width-r*1.3f,height-r*1.7f,r*0.8f)) {role=2;fire=true;fireQueued=true;}
                else if(inside(x,y,width-r*3.1f,height-r*1.3f,r*0.65f)) {role=3;jump=true;jumpQueued=true;}
                else if(inside(x,y,width-r*1.3f,height-r*3.5f,r*0.65f)) {role=4;reload=true;}
                else if(inside(x,y,width-r*3.1f,height-r*3.5f,r*0.65f)) {role=6;use=true;}
                else if(x<width*0.4f && !roles.containsValue(1)) {role=1;}
                else if(!roles.containsValue(5)) {role=5;lastX=x;lastY=y;}
                else {role=0;}
                roles.put(id,role);
            }
            // Release events can carry the final look position even when Android coalesces MOVE events.
            if(action==MotionEvent.ACTION_MOVE || action==MotionEvent.ACTION_DOWN || action==MotionEvent.ACTION_POINTER_DOWN || action==MotionEvent.ACTION_UP || action==MotionEvent.ACTION_POINTER_UP) {
                for(int i=0;i<e.getPointerCount();i++) {
                    int role=roles.getOrDefault(e.getPointerId(i),0); float x=e.getX(i),y=e.getY(i);
                    if(role==1) {dx=(x-r*1.6f)/r;dy=(y-(height-r*1.7f))/r;float norm=Math.max(1,(float)Math.hypot(dx,dy));dx/=norm;dy/=norm;}
                    if(role==5) {lookX=Math.max(-6,Math.min(6,lookX+(x-lastX)/width*3.0f));lookY=Math.max(-6,Math.min(6,lookY+(y-lastY)/height*2.0f));lastX=x;lastY=y;}
                }
            }
            if(action==MotionEvent.ACTION_UP || action==MotionEvent.ACTION_POINTER_UP) {
                Integer role=roles.remove(e.getPointerId(index));
                if(role!=null && role==1) {dx=dy=0;}
                if(role!=null && role==2) fire=roles.containsValue(2);
                if(role!=null && role==3) jump=roles.containsValue(3);
                if(role!=null && role==6) use=roles.containsValue(6);
                if(action==MotionEvent.ACTION_UP) performClick();
            }
            invalidate(); return true;
        }
        @Override public boolean performClick() {super.performClick();return true;}
        private void circle(Canvas canvas,float x,float y,float r,String text,boolean active) {
            paint.setColor(active ? 0xb0e6a658 : 0x804d7278); canvas.drawCircle(x,y,r,paint);
            paint.setColor(0xffe4f4ed); paint.setTextAlign(Paint.Align.CENTER); paint.setTextSize(r*0.38f);canvas.drawText(text,x,y+r*0.12f,paint);
        }
        @Override protected void onDraw(Canvas canvas) {
            if(!loaded) return; float r=radius(),width=getWidth(),height=getHeight();
            circle(canvas,r*1.6f,height-r*1.7f,r,"",false);
            circle(canvas,r*1.6f+dx*r*0.6f,height-r*1.7f+dy*r*0.6f,r*0.38f,"",true);
            circle(canvas,width-r*1.3f,height-r*1.7f,r*0.8f,"FEUER",fire);
            circle(canvas,width-r*3.1f,height-r*1.3f,r*0.65f,"SPRUNG",jump);
            circle(canvas,width-r*1.3f,height-r*3.5f,r*0.65f,"LADEN",reload);
            circle(canvas,width-r*3.1f,height-r*3.5f,r*0.65f,"BENUTZEN",use);
        }
    }
}
