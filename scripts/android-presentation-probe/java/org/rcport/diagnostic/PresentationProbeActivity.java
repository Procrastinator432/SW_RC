package org.rcport.diagnostic;

import android.app.Activity;
import android.os.Bundle;
import android.os.SystemClock;
import android.util.Log;
import android.view.WindowManager;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.io.*;
import java.lang.reflect.*;
import java.nio.*;
import java.util.concurrent.CountDownLatch;
import org.json.JSONObject;

/** Optional local acceptance APK only. Reuses the production JNI, GL and audio hosts.
 * Inputs and checkpoints come from a separate contiguous Android-native replay.
 * It does not install test hooks in the ordinary delivery APK. */
public final class PresentationProbeActivity extends Activity {
    private volatile boolean failed,stopped;
    private GpuSceneView gpu;
    private SceneAudio audio;
    private TextView label;
    private long game;
    private Class<?> host;
    private File directory;
    private String name;
    private Object call(String method,Class<?>[] types,Object... args) throws Exception {
        Method m=host.getDeclaredMethod(method,types);m.setAccessible(true);
        try {return m.invoke(null,args);} catch(InvocationTargetException e) {throw new IOException(method,e.getCause());}
    }
    private byte[] bytes(String filename) throws IOException {
        if(!filename.matches("[a-zA-Z0-9._-]+"))throw new IOException("Invalid probe filename");
        File file=new File(directory,filename);
        if(file.length()>112*1024*1024 || file.length()<1)throw new IOException("Probe file size");
        byte[] bytes=new byte[(int)file.length()];
        try(DataInputStream in=new DataInputStream(new FileInputStream(file))){in.readFully(bytes);}
        return bytes;
    }
    private void ui(Runnable task) throws Exception {
        CountDownLatch done=new CountDownLatch(1);
        runOnUiThread(()->{try{task.run();}catch(Exception e){failed=true;Log.e("RCProbe","UI failure",e);}finally{done.countDown();}});
        if(!done.await(30,java.util.concurrent.TimeUnit.SECONDS))throw new IOException("Probe UI timeout");
        if(failed || stopped)throw new IOException("Probe stopped or failed");
    }
    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        getWindow().getDecorView().setSystemUiVisibility(5894);
        directory=getExternalFilesDir("acceptance");
        name=getIntent().getStringExtra("probe");if(name==null)name="injury";
        LinearLayout layout=new LinearLayout(this);layout.setOrientation(LinearLayout.VERTICAL);
        label=new TextView(this);label.setText("Android presentation acceptance · "+name);layout.addView(label);
        gpu=new GpuSceneView(this,true,true,error->{failed=true;label.setText(error);});
        layout.addView(gpu,new LinearLayout.LayoutParams(-1,0,1));setContentView(layout);
        new Thread(this::runProbe,"rc-presentation-probe").start();
    }
    private void runProbe() {
        JSONObject report=new JSONObject();
        try {
            host=Class.forName("org.rcport.diagnostic.PlayActivity");
            byte[] scene=bytes("geo_01a.rcplay");
            game=(Long)call("createGame",new Class<?>[]{byte[].class},scene);scene=null;
            if(game==0)throw new IOException("Native scene creation failed");
            byte[] checkpoint=bytes(name+".save");
            JSONObject state=new JSONObject(new String(checkpoint,20,checkpoint.length-20,java.nio.charset.StandardCharsets.UTF_8)).getJSONObject("state");
            int tick=state.getJSONObject("statistics").getInt("ticks");
            call("restoreCheckpoint",new Class<?>[]{long.class,byte[].class},game,checkpoint);
            byte[] world=(byte[])call("gpuWorld",new Class<?>[]{long.class},game);
            GpuSceneView.World parsed=new GpuSceneView.World(world);world=null;
            byte[] bank=(byte[])call("gameAudio",new Class<?>[]{long.class,int.class},game,0);
            ui(()->{gpu.world(parsed);try{audio=new SceneAudio(this,bank);}catch(IOException e){throw new IllegalStateException(e);}});
            call("gameAudio",new Class<?>[]{long.class,int.class},game,2); // discard saved historic events
            long deadline=SystemClock.uptimeMillis()+30000;
            boolean[] ready={false};
            while(!ready[0]) {
                ui(()->{try{ready[0]=audio.ready();}catch(IOException e){throw new IllegalStateException(e);}});
                if(SystemClock.uptimeMillis()>deadline)throw new IOException("SoundPool bank not ready");
                if(!ready[0])SystemClock.sleep(25);
            }
            ByteBuffer replay=ByteBuffer.wrap(bytes("map_a_replay.bin")).order(ByteOrder.LITTLE_ENDIAN);
            if(replay.getInt(8)!=(replay.capacity()-12)/20)throw new IOException("Invalid input recording");
            int count=Math.min(getIntent().getIntExtra("ticks",150),(replay.capacity()-12)/20-tick);
            replay.position(12+tick*20);
            int[] status=null;
            for(int i=0;i<count;i++) {
                long started=SystemClock.uptimeMillis();
                float forward=replay.getFloat(),strafe=replay.getFloat(),lookX=replay.getFloat(),lookY=replay.getFloat();int buttons=replay.getInt();
                byte[] packet=(byte[])call("gpuFrame",new Class<?>[]{long.class,double.class,float.class,float.class,float.class,float.class,int.class,long[].class},game,1./60.,forward,strafe,lookX,lookY,buttons,null);
                GpuSceneView.Frame frame=new GpuSceneView.Frame(packet,i==count/2 || i==count-1);status=frame.status;
                byte[] events=(byte[])call("gameAudio",new Class<?>[]{long.class,int.class},game,2);
                byte[] ambient=(byte[])call("gameAudio",new Class<?>[]{long.class,int.class},game,3);
                int current=tick+i+1;
                File reference=new File(getFilesDir(),"gpu-reference.png");
                if(frame.capture && reference.exists() && !reference.delete())throw new IOException("Cannot clear previous probe image");
                ui(()->{gpu.submit(frame);label.setText(name+" · tick "+current);try{audio.events(events);audio.ambientLoops(ambient);}catch(IOException e){throw new IllegalStateException(e);}});
                if(frame.capture) {
                    long until=SystemClock.uptimeMillis()+10000;
                    while(!reference.exists() && !failed && SystemClock.uptimeMillis()<until)SystemClock.sleep(25);
                    if(!reference.exists())throw new IOException("GLES screenshot was not produced");
                    SystemClock.sleep(100);
                    java.nio.file.Files.copy(reference.toPath(),new File(getFilesDir(),name+"-"+current+".png").toPath(),java.nio.file.StandardCopyOption.REPLACE_EXISTING);
                }
                long remaining=17-(SystemClock.uptimeMillis()-started);if(remaining>0)SystemClock.sleep(remaining);
            }
            SystemClock.sleep(500);
            byte[] saved=(byte[])call("gameCheckpoint",new Class<?>[]{long.class},game);
            try(FileOutputStream out=new FileOutputStream(new File(directory,name+"-after.save"))){out.write(saved);}
            report.put("passed",!failed).put("probe",name).put("start_tick",tick).put("frames",count).put("sounds_ready",true).put("status",new org.json.JSONArray(status));
            report.put("scope","Actual Android native input continuation, production GLES and SoundPool. No claim of physical-phone hearing/touch acceptance.");
        } catch(Exception e) {failed=true;Log.e("RCProbe","Probe failure",e);try{report.put("passed",false).put("error",e.toString());}catch(Exception ignored){}}
        finally {
            try {if(game!=0)call("releaseGame",new Class<?>[]{long.class},game);}catch(Exception ignored){}
            try(FileOutputStream out=new FileOutputStream(new File(directory,name+"-result.json"))){out.write(report.toString(2).getBytes(java.nio.charset.StandardCharsets.UTF_8));}catch(Exception e){Log.e("RCProbe","Report failure",e);}
            Log.i("RCProbe",report.toString());
        }
    }
    @Override protected void onResume(){super.onResume();gpu.onResume();}
    @Override protected void onPause(){super.onPause();gpu.onPause();if(audio!=null)audio.suspend();}
    @Override protected void onDestroy(){stopped=true;if(audio!=null)audio.close();super.onDestroy();}
}
