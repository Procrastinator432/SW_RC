package org.rcport.diagnostic;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.graphics.Bitmap;
import android.view.MotionEvent;
import android.widget.*;
import java.io.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;

/** Diagnostic geometry viewer. Free flight has no collision or gameplay. */
public class MainActivity extends Activity {
    static { System.loadLibrary("rc_bridge"); }
    private static native String inspectPackage(byte[] data);
    private static native long createScene(byte[] data) throws IOException;
    private static native int[] drawScene(long scene, int width, int height, float yaw, float pitch,
        float zoom, float x, float y, float z, boolean free) throws IOException;
    private static native void releaseScene(long scene);
    private static native float[] startCamera(long scene, int index) throws IOException;
    private static native int checkBsp(long scene, float x, float y, float z, float yaw, float pitch);
    private final ExecutorService worker=Executors.newSingleThreadExecutor();
    private final AtomicBoolean drawing=new AtomicBoolean();
    private TextView status;
    private Button open, mode;
    private ImageView view;
    private LinearLayout movement;
    private long scene; // worker only
    private boolean loaded, free;
    private float yaw=-0.7f, pitch=0.65f, zoom=1f, eyeX, eyeY, eyeZ, touchX, touchY;
    private int revision, generation;
    private int startIndex=-1, startCount;
    private boolean locating;
    private String sceneMessage="";
    private boolean atAnchor;
    private void inspectBsp() {
        if(!loaded) return;
        final float requestedYaw=yaw, requestedPitch=pitch;
        final float x=free ? eyeX : (float)(Math.cos(yaw)*Math.cos(pitch))*2.8f/zoom;
        final float y=free ? eyeY : (float)(Math.sin(yaw)*Math.cos(pitch))*2.8f/zoom;
        final float z=free ? eyeZ : (float)Math.sin(pitch)*2.8f/zoom;
        final int requestedGeneration=generation, requestedRevision=revision;
        worker.execute(() -> {
            int result=checkBsp(scene,x,y,z,requestedYaw,requestedPitch);
            runOnUiThread(() -> {
                if(isDestroyed() || !loaded || generation!=requestedGeneration || revision!=requestedRevision) return;
                atAnchor=true; // clear this diagnostic status on the next movement/reset
                if(result==-2) status.setText("Diese Szenendatei enthält keine BSP-Prüfdaten. Für die Prüfung eine Originalkarte (.ctm) öffnen.");
                else if(result<0) status.setText("BSP-Prüfung fehlgeschlagen.");
                else status.setText("Original-BSP: Kamerapunkt "+((result&1)!=0 ? "frei" : "im Solid")+"; Blicklinie (1000 Einheiten) "+((result&2)!=0 ? "frei" : "blockiert")+". Ohne Mesh- oder Spieler-Kollision.");
            });
        });
    }
    private void leaveAnchor() { if(atAnchor) { atAnchor=false; status.setText(sceneMessage); } }

    private void goToStart() {
        if(!loaded || locating) return;
        locating=true;
        final int requestedGeneration=generation;
        final int index=startCount>0 ? (startIndex+1)%startCount : 0;
        worker.execute(() -> {
            float[] camera=null; String error=null;
            try { camera=startCamera(scene,index); } catch(Exception ex) { error=ex.getMessage(); }
            final float[] result=camera; final String failure=error;
            runOnUiThread(() -> {
                locating=false;
                if(isDestroyed() || !loaded || generation!=requestedGeneration) return;
                if(failure!=null) { status.setText(failure); return; }
                eyeX=result[0]; eyeY=result[1]; eyeZ=result[2]; yaw=result[3]; pitch=result[4];
                startCount=(int)result[5]; startIndex=index; free=true; atAnchor=true;
                mode.setText("Freie Kamera aktiv · ohne Kollision"); movement.setVisibility(android.view.View.VISIBLE);
                status.setText("Startpunkt "+(index+1)+" / "+startCount+" · Originalposition und Blickrichtung. Keine Augenhöhe oder Spawn-Logik; freie Kamera ohne Kollision.");
                changed();
            });
        });
    }

    private void changed() { revision++; queueFrame(); }
    private void resetCamera() {
        leaveAnchor();
        yaw=-0.7f; pitch=0.65f; zoom=1f; free=false;
        mode.setText("Freie Kamera"); movement.setVisibility(android.view.View.GONE); changed();
    }
    private void control(LinearLayout row, String text, Runnable action) {
        Button button=new Button(this); button.setText(text); button.setTextSize(12);
        button.setPadding(0,0,0,0);
        button.setOnClickListener(v -> { if(loaded) action.run(); });
        row.addView(button,new LinearLayout.LayoutParams(0,LinearLayout.LayoutParams.WRAP_CONTENT,1));
    }
    private void move(float forward, float right, float up) {
        leaveAnchor();
        float step=0.04f;
        eyeX+=step*(-forward*(float)(Math.cos(yaw)*Math.cos(pitch))-right*(float)Math.sin(yaw));
        eyeY+=step*(-forward*(float)(Math.sin(yaw)*Math.cos(pitch))+right*(float)Math.cos(yaw));
        eyeZ+=step*(-forward*(float)Math.sin(pitch)+up);
        eyeX=Math.max(-99f,Math.min(99f,eyeX)); eyeY=Math.max(-99f,Math.min(99f,eyeY));
        eyeZ=Math.max(-99f,Math.min(99f,eyeZ)); changed();
    }
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout layout=new LinearLayout(this); layout.setOrientation(LinearLayout.VERTICAL);
        int padding=(int)(24*getResources().getDisplayMetrics().density);
        layout.setPadding(padding,padding*2,padding,padding);
        TextView heading=new TextView(this); heading.setText("Republic Commando\nOriginalgeometrie in Rust");
        heading.setTextSize(20); layout.addView(heading);
        status=new TextView(this); status.setTextSize(14);
        status.setText("Karte (.ctm) oder Szene (.rcscene, max. 16 MiB) öffnen. Ziehen dreht den Blick. Noch kein Gameplay.");
        layout.addView(status);
        open=new Button(this); open.setText("Karte / Szene öffnen");
        open.setOnClickListener(v -> {
            Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT); intent.setType("*/*");
            intent.addCategory(Intent.CATEGORY_OPENABLE); startActivityForResult(intent,1);
        }); layout.addView(open);
        LinearLayout camera=new LinearLayout(this);
        control(camera,"Zoom +",() -> { if(free) move(1,0,0); else { zoom=Math.min(32f,zoom*1.4f); changed(); } });
        control(camera,"Zoom −",() -> { if(free) move(-1,0,0); else { zoom=Math.max(0.5f,zoom/1.4f); changed(); } });
        control(camera,"Übersicht",this::resetCamera);
        control(camera,"Startpunkt",this::goToStart); layout.addView(camera);
        LinearLayout checks=new LinearLayout(this);
        control(checks,"BSP prüfen",this::inspectBsp); layout.addView(checks);
        mode=new Button(this); mode.setText("Freie Kamera");
        mode.setOnClickListener(v -> {
            if(!loaded) return;
            if(free) { resetCamera(); return; }
            eyeX=(float)(Math.cos(yaw)*Math.cos(pitch))*2.8f/zoom;
            eyeY=(float)(Math.sin(yaw)*Math.cos(pitch))*2.8f/zoom;
            eyeZ=(float)Math.sin(pitch)*2.8f/zoom;
            free=true; mode.setText("Freie Kamera aktiv · ohne Kollision");
            movement.setVisibility(android.view.View.VISIBLE); changed();
        }); layout.addView(mode);
        movement=new LinearLayout(this); movement.setOrientation(LinearLayout.VERTICAL);
        LinearLayout horizontal=new LinearLayout(this);
        control(horizontal,"Links",() -> move(0,-1,0)); control(horizontal,"Vor",() -> move(1,0,0));
        control(horizontal,"Zurück",() -> move(-1,0,0)); control(horizontal,"Rechts",() -> move(0,1,0));
        movement.addView(horizontal);
        LinearLayout vertical=new LinearLayout(this);
        control(vertical,"Hoch",() -> move(0,0,1)); control(vertical,"Runter",() -> move(0,0,-1));
        movement.addView(vertical); movement.setVisibility(android.view.View.GONE); layout.addView(movement);
        view=new ImageView(this); view.setScaleType(ImageView.ScaleType.FIT_CENTER);
        view.setContentDescription("Originalgeometrie mit optionalen Basistexturen");
        layout.addView(view,new LinearLayout.LayoutParams(-1,0,1));
        view.setOnTouchListener((v,event) -> {
            if(!loaded) return false;
            if(event.getActionMasked()==MotionEvent.ACTION_DOWN) { touchX=event.getX(); touchY=event.getY(); return true; }
            if(event.getActionMasked()==MotionEvent.ACTION_MOVE) {
                leaveAnchor();
                yaw+=(event.getX()-touchX)*0.008f;
                pitch=Math.max(-1.4f,Math.min(1.4f,pitch+(event.getY()-touchY)*0.008f));
                touchX=event.getX(); touchY=event.getY(); changed(); return true;
            }
            if(event.getActionMasked()==MotionEvent.ACTION_UP) { v.performClick(); queueFrame(); }
            return true;
        }); setContentView(layout);
    }
    @Override protected void onActivityResult(int request,int result,Intent data) {
        super.onActivityResult(request,result,data);
        if(request!=1 || result!=RESULT_OK || data==null || data.getData()==null) return;
        open.setEnabled(false); loaded=false; generation++; startIndex=-1; startCount=0; atAnchor=false; resetCamera(); view.setImageDrawable(null);
        status.setText("Paket wird im nativen Rust-Code gelesen …");
        worker.execute(() -> {
            String message; boolean ready=false; releaseScene(scene); scene=0;
            try(InputStream input=getContentResolver().openInputStream(data.getData()); ByteArrayOutputStream bytes=new ByteArrayOutputStream()) {
                if(input==null) throw new IOException("Datei konnte nicht geöffnet werden");
                byte[] block=new byte[16384]; int n;
                while((n=input.read(block))!=-1) {
                    if(bytes.size()+n>16*1024*1024) throw new IOException("Datei überschreitet 16 MiB");
                    bytes.write(block,0,n);
                }
                byte[] payload=bytes.toByteArray(); message=inspectPackage(payload); scene=createScene(payload); ready=true;
                message+="\nZiehen: Blick drehen. Zoom oder freie Kamera mit Bewegungstasten. Ohne Kollision und Gameplay; Basistexturen.";
            } catch(Exception ex) { message="Fehler: "+ex.getMessage(); }
            final String finalMessage=message; final boolean finalReady=ready;
            runOnUiThread(() -> { if(!isDestroyed()) { sceneMessage=finalMessage; status.setText(finalMessage); open.setEnabled(true); loaded=finalReady; if(loaded) queueFrame(); } });
        });
    }
    private void queueFrame() {
        if(!loaded || isDestroyed() || !drawing.compareAndSet(false,true)) return;
        final float requestedYaw=yaw, requestedPitch=pitch, requestedZoom=zoom, x=eyeX,y=eyeY,z=eyeZ;
        final boolean requestedFree=free;
        final int requestedRevision=revision, requestedGeneration=generation;
        worker.execute(() -> {
            Bitmap bitmap=null; String error=null;
            try { int[] pixels=drawScene(scene,640,400,requestedYaw,requestedPitch,requestedZoom,x,y,z,requestedFree);
                bitmap=Bitmap.createBitmap(pixels,640,400,Bitmap.Config.ARGB_8888);
            } catch(Exception ex) { error=ex.getMessage(); }
            final Bitmap result=bitmap; final String failure=error;
            runOnUiThread(() -> {
                drawing.set(false); if(isDestroyed()) return;
                if(result!=null && loaded && generation==requestedGeneration) view.setImageBitmap(result);
                if(failure!=null && generation==requestedGeneration) status.setText("Renderfehler: "+failure);
                if(loaded && (revision!=requestedRevision || generation!=requestedGeneration)) queueFrame();
            });
        });
    }
    @Override protected void onDestroy() {
        loaded=false; worker.execute(() -> {releaseScene(scene); scene=0;}); worker.shutdown(); super.onDestroy();
    }
}
