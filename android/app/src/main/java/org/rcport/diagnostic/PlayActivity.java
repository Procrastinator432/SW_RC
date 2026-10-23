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
    private static native void travelGame(long id,byte[] data) throws IOException;
    private static native void restoreHandoff(long id,byte[] data) throws IOException;
    private static native int[] gameFrame(long id,double elapsed,float forward,float strafe,
        float lookX,float lookY,int buttons,int width,int height,long[] metrics) throws IOException;
    private static native byte[] gameAudio(long id,int kind) throws IOException;
    private static native byte[] gpuWorld(long id) throws IOException;
    private static native byte[] gpuFrame(long id,double elapsed,float forward,float strafe,float lookX,float lookY,int buttons,long[] metrics) throws IOException;
    private static native void gameCommand(long id,int command) throws IOException;
    private static native int[] jumpStatus(long id) throws IOException;
    private static native String actorStatus(long id) throws IOException;
    private static native String missionStatus(long id) throws IOException;
    private static native byte[] mapHandoff(long id) throws IOException;
    private static native byte[] gameCheckpoint(long id) throws IOException;
    private static native void restoreCheckpoint(long id,byte[] data) throws IOException;
    private static native long checkpointMarker(long id);
    private static native int[] gameStatus(long id) throws IOException;
    private static native int gameMap(long id);
    private static native int gameEnergy(long id);
    private static native boolean checkpointSafe(long id);
    private static String campaignMap(long id){int n=gameMap(id);return n>=1 && n<=4?CampaignStore.MAPS[n-1]:"";}
    private static String guidance(long id)throws IOException {String text=missionStatus(id),map=campaignMap(id);return map.isEmpty() || text.startsWith(map)?text:map+" · "+text;}
    private CheckpointStore checkpoints;
    private String currentScene; // worker-owned immutable cache key
    private long savedTick=-1,savedMarker,nextSaveAttempt; // worker-owned
    private final CheckpointPolicy checkpointPolicy=new CheckpointPolicy();
    private CheckpointStore.Record restoredRecord;
    private int restoredTick;
    private boolean defeatRecorded;
    private String lastJumpState=""; // worker-owned observation only
    private static native void releaseGame(long id);
    private final ExecutorService worker=Executors.newSingleThreadExecutor();
    private final Handler handler=new Handler(Looper.getMainLooper());
    private final Runnable pump=() -> queueFrame(false);
    private long game; // worker only
    private boolean loaded,paused,resumed,drawing,finished,pendingTravel,loading;
    private int audioGeneration;
    private int generation,lastShots,lastHP=100,lastPhase,lastEnemies=3,lastAmmo=20;
    private long lastFrame;
    private ImageView image;
    private GpuSceneView gpu;
    private boolean useGpu;
    private boolean campaignMode;
    private String lastGuidance="";
    private Controls controls;
    private TextView status;
    private Button open,pause;
    private String nextMap="";
    private Bitmap bitmap;
    private ToneGenerator audio;
    private SceneAudio sceneAudio;
    private boolean profiling;
    private long profileSequence;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        checkpoints=new CheckpointStore(getFilesDir());
        campaignMode=getIntent().getBooleanExtra("rcCampaign",false);
        profiling=getIntent().getBooleanExtra("rcProfile",false);
        android.app.ActivityManager manager=(android.app.ActivityManager)getSystemService(ACTIVITY_SERVICE);
        useGpu=manager.getDeviceConfigurationInfo().reqGlEsVersion>=0x20000 && !getIntent().getBooleanExtra("rcSoftware",false);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_FULLSCREEN
            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY);
        try { audio=new ToneGenerator(AudioManager.STREAM_MUSIC,22); } catch(RuntimeException ignored) {}
        LinearLayout root=new LinearLayout(this); root.setOrientation(LinearLayout.VERTICAL); root.setBackgroundColor(Color.rgb(16,25,35));
        LinearLayout bar=new LinearLayout(this);bar.setGravity(Gravity.CENTER_VERTICAL);
        pause=button(bar,"Pause",this::togglePause);
        status=new TextView(this);status.setTextColor(Color.rgb(190,230,224));status.setTextSize(12);status.setMaxLines(2);
        status.setText("Kampagne wird geladen …");bar.addView(status,new LinearLayout.LayoutParams(0,-2,1));
        open=button(bar,"Weiter",()->{if(campaignMode && !nextMap.isEmpty())loadCampaign(nextMap,true);else chooseScene();});open.setVisibility(View.GONE);
        button(bar,"Menü",this::showPauseMenu);
        root.addView(bar,new LinearLayout.LayoutParams(-1,Math.round(48*getResources().getDisplayMetrics().density)));
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
        if((state!=null && checkpoints.hasSave()) || getIntent().getBooleanExtra("rcContinue",false))loadCheckpoint(false);
        else if(getIntent().getStringExtra("rcMap")!=null)loadCampaign(getIntent().getStringExtra("rcMap"),false);
        else if(getIntent().getData()!=null) loadScene(getIntent().getData());
        else {status.setText("Eine Szene über das Menü auswählen");showPauseMenu();}
    }
    private Button button(LinearLayout row,String title,Runnable action) {
        Button b=new Button(this); b.setText(title); b.setTextSize(12);int minimum=Math.round(56*getResources().getDisplayMetrics().density);b.setMinWidth(minimum);b.setMinimumWidth(minimum);b.setPadding(12,0,12,0); b.setOnClickListener(v -> action.run()); row.addView(b); return b;
    }
    static void validateImport(File scene,String map)throws IOException {
        byte[] bytes;try(InputStream in=new FileInputStream(scene)){bytes=SceneBytes.read(in,scene.length(),scene.getParentFile());}
        long candidate=createGame(bytes);
        try {if(!campaignMap(candidate).equals(map))throw new IOException("Falsche Karte im Szenenpaket: "+map);}
        finally {releaseGame(candidate);}
    }
    private void chooseScene(){
        pendingTravel=finished && lastPhase==1 && !nextMap.isEmpty();
        Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT).setType("*/*").addCategory(Intent.CATEGORY_OPENABLE);startActivityForResult(intent,1);
    }
    private void loadCampaign(String map,boolean travel){
        if(loading)return;prepareLoad();
        worker.execute(()->{
            try {File file=new CampaignStore(getFilesDir()).scene(map);loadSceneWorker(Uri.fromFile(file),null,travel,!travel && getIntent().getBooleanExtra("rcFresh",false));}
            catch(Exception e){runOnUiThread(()->{
                if(isDestroyed())return;loading=false;paused=true;pause.setText("Fortsetzen");open.setEnabled(true);
                status.setText("Szenenpaket prüfen");
                new android.app.AlertDialog.Builder(this).setTitle("Karte nicht verfügbar").setMessage(e.getMessage()).setPositiveButton("Zum Hauptmenü",(d,w)->finish()).setNegativeButton("Abbrechen",null).show();
            });}
        });
    }
    private void showPauseMenu(){
        if(loading)return;if(loaded && !paused && !finished)togglePause();
        LinearLayout list=new LinearLayout(this);list.setOrientation(LinearLayout.VERTICAL);
        ScrollView scroll=new ScrollView(this);scroll.addView(list);
        android.app.AlertDialog dialog=new android.app.AlertDialog.Builder(this).setTitle("Pause").setView(scroll).setNegativeButton("Schließen",null).create();
        String[] labels={"Fortsetzen","Checkpoint speichern","Checkpoint laden","Steuerung","Karte neu starten","Hauptmenü","Szene manuell öffnen",useGpu?"Softwaregrafik verwenden":"GPU verwenden"};
        Runnable[] actions={()->{if(paused)togglePause();},this::saveManually,()->showCheckpointChoices("Spielstand auswählen"),()->ControlSettings.show(this),()->new android.app.AlertDialog.Builder(this).setTitle("Karte neu starten?").setMessage("Fortschritt seit dem Karteneinstieg wird zurückgesetzt.").setPositiveButton("Neustart",(d,w)->restart()).setNegativeButton("Abbrechen",null).show(),this::finish,this::chooseScene,()->switchBackend(!useGpu)};
        for(int i=0;i<labels.length;i++){final int n=i;Button item=new Button(this);item.setText(labels[i]);item.setMinHeight(Math.round(48*getResources().getDisplayMetrics().density));item.setOnClickListener(v->{dialog.dismiss();actions[n].run();});list.addView(item);}
        dialog.show();
    }
    private void switchBackend(boolean enabled) {
        useGpu=enabled;generation++;controls.clear();lastFrame=0;
        if(sceneAudio!=null) sceneAudio.stop();
        gpu.setVisibility(useGpu ? View.VISIBLE : View.GONE);image.setVisibility(useGpu ? View.GONE : View.VISIBLE);
        Log.i("RCPlay","backend="+(useGpu ? "gpu" : "cpu"));
        if(loaded) {worker.execute(() -> {try {gameCommand(game,0);} catch(Exception e) {failure(e.getMessage());}});queueFrame(true);}
    }
    private void togglePause() {
        if(!loaded || loading || finished) return;
        paused=!paused; generation++; controls.clear(); lastFrame=0; handler.removeCallbacks(pump);
        pause.setText(paused ? "Fortsetzen" : "Pause");
        if(paused) {
            if(sceneAudio!=null) sceneAudio.suspend();
            worker.execute(() -> {try {gameCommand(game,0);saveCheckpointWorker(true);} catch(Exception e) {Log.e("RCPlay","pause",e);} });
            status.setText("Pausiert");
        } else {if(sceneAudio!=null && resumed) sceneAudio.resume();status.setText(lastGuidance); queueFrame(false);}
    }
    private void restart() {
        if(!loaded || loading) return;
        generation++; audioGeneration++; controls.clear(); handler.removeCallbacks(pump); lastFrame=0; finished=false; paused=false;
        open.setVisibility(View.GONE);nextMap="";
        pause.setText("Pause"); status.setText("Neustart …");
        if(sceneAudio!=null) sceneAudio.reset();
        worker.execute(() -> {
            try { gameCommand(game,1);checkpointPolicy.reset();restoredRecord=null;defeatRecorded=false;saveCheckpointWorker(true); runOnUiThread(() -> {
                if(isDestroyed()) return; lastShots=0; lastHP=100; lastPhase=0; lastEnemies=3; lastAmmo=20;
                status.setText(lastGuidance); queueFrame(true);
            }); } catch(Exception e) {failure(e.getMessage());}
        });
    }
    @Override protected void onActivityResult(int request,int result,Intent data) {
        super.onActivityResult(request,result,data);
        if(request!=1 || result!=RESULT_OK || data==null || data.getData()==null) return;
        loadScene(data.getData());
    }
    private void saveCheckpointWorker(boolean force) {saveCheckpointWorker(force,false);}
    private void saveCheckpointWorker(boolean force,boolean announce) {
        if(game==0 || currentScene==null)return;
        try {
            int[] values=gameStatus(game);long marker=checkpointMarker(game);
            if(values[4]==2 || values[0]==0) {
                if(!defeatRecorded){defeatRecorded=true;int failures=restoredRecord!=null && values[5]-restoredTick<=900?checkpoints.quickDeaths(restoredRecord,true):0;
                    runOnUiThread(()->showCheckpointChoices(failures>=2?"Wiederholt kurz nach dem Laden gefallen · früheren Spielstand wählen":"Gefallen · Spielstand auswählen"));}
                if(announce)runOnUiThread(() -> Toast.makeText(this,"Der letzte spielbare Checkpoint bleibt erhalten.",Toast.LENGTH_LONG).show());return;
            }
            boolean stable=checkpointPolicy.observe(values,checkpointSafe(game),marker);
            if(!force && (System.nanoTime()<nextSaveAttempt || (!stable && savedTick>=0 && values[5]-savedTick<600 && marker==savedMarker)))return;
            byte[] state=gameCheckpoint(game);String title=guidance(game);
            checkpoints.save(currentScene,title,state);
            if(announce)checkpoints.saveNamed(CheckpointStore.MANUAL,currentScene,title,state);
            if(stable){checkpoints.saveNamed(CheckpointStore.SAFE,currentScene,title,state);checkpointPolicy.saved(values[5],marker);}
            savedTick=values[5];savedMarker=marker;nextSaveAttempt=0;
            Log.i("RCCheckpoint","saved ticks="+savedTick+" scene="+currentScene);
            if(announce)runOnUiThread(() -> {if(!isDestroyed())Toast.makeText(this,"Checkpoint gespeichert",Toast.LENGTH_SHORT).show();});
        } catch(Exception e) {
            nextSaveAttempt=System.nanoTime()+30_000_000_000L;Log.e("RCCheckpoint","save failed",e);
            runOnUiThread(() -> {if(!isDestroyed())Toast.makeText(this,"Checkpoint konnte nicht gespeichert werden: "+e.getMessage(),Toast.LENGTH_LONG).show();});
        }
    }
    private void saveManually() {
        if(!loaded || loading)return;
        worker.execute(() -> {try {gameCommand(game,0);saveCheckpointWorker(true,true);}catch(Exception e){failure(e.getMessage());}});
    }
    private void loadCheckpoint(boolean previous) {
        loadCheckpoint(previous?"previous.save":"resume.save");
    }
    private void showCheckpointChoices(String title) {
        if(isDestroyed() || loading)return;
        String[] slots={CheckpointStore.SAFE,CheckpointStore.SAFE_PREVIOUS,CheckpointStore.MANUAL,"resume.save","previous.save",CheckpointStore.ENTRY};
        String[] names={"Sicherer Checkpoint","Früherer sicherer Checkpoint","Manuell gespeichert","Letzte Sitzung (kann im Kampf sein)","Vorherige Sitzung (kann im Kampf sein)","Karteneinstieg"};
        LinearLayout list=new LinearLayout(this);list.setOrientation(LinearLayout.VERTICAL);ScrollView scroll=new ScrollView(this);scroll.addView(list);
        android.app.AlertDialog dialog=new android.app.AlertDialog.Builder(this).setTitle(title).setView(scroll).setNegativeButton("Abbrechen",null).create();
        for(int i=0;i<slots.length;i++)if(checkpoints.has(slots[i])){final String slot=slots[i];Button b=new Button(this);b.setText(names[i]);b.setMinHeight(Math.round(48*getResources().getDisplayMetrics().density));b.setOnClickListener(v->{dialog.dismiss();loadCheckpoint(slot);});list.addView(b);
            TextView detail=new TextView(this);detail.setText("Spielstand wird geprüft …");list.addView(detail);
            worker.execute(()->{String text;try{CheckpointStore.Record r=checkpoints.load(slot);text=r.title+" · "+new java.text.SimpleDateFormat("dd.MM. HH:mm",Locale.GERMAN).format(new Date(r.savedAt));}catch(IOException e){text="Beschädigt · andere Sicherung wählen";}final String description=text;runOnUiThread(()->{if(!isDestroyed())detail.setText(description);});});
        }
        Button restart=new Button(this);restart.setText("Karte neu starten");restart.setEnabled(loaded);restart.setOnClickListener(v->{dialog.dismiss();restart();});list.addView(restart);dialog.show();
    }
    private void loadCheckpoint(String slot) {
        if(loading)return;
        if(!checkpoints.has(slot)) {showCheckpointChoices("Verfügbare Spielstände");return;}
        prepareLoad();
        worker.execute(() -> {
            try {CheckpointStore.Record record=checkpoints.load(slot);if(loadSceneWorker(null,record,false,false) && !slot.equals("resume.save"))saveCheckpointWorker(true);}
            catch(Exception e) {runOnUiThread(() -> recovery("Checkpoint konnte nicht geladen werden: "+e.getMessage(),null,true));}
        });
    }
    private void prepareLoad() {
        loading=true;generation++;audioGeneration++;controls.clear();handler.removeCallbacks(pump);lastFrame=0;open.setEnabled(false);
        paused=true;if(sceneAudio!=null)sceneAudio.suspend();status.setText("Spielstand und Szene werden geladen …");
    }
    private void loadScene(Uri uri) {
        final boolean travel=pendingTravel;pendingTravel=false;prepareLoad();
        worker.execute(() -> loadSceneWorker(uri,null,travel,false));
    }
    private boolean loadSceneWorker(Uri uri,CheckpointStore.Record record,boolean travel,boolean ignoreHandoff) {
        long candidate=0;boolean created=false;Uri failedUri=uri;
        try {
            final byte[] bytes;
            if(record!=null)bytes=checkpoints.scene(record.scene,getCacheDir());
            else try(android.content.res.AssetFileDescriptor descriptor=getContentResolver().openAssetFileDescriptor(uri,"r")) {
                if(descriptor==null)throw new IOException("Szenendatei konnte nicht geöffnet werden");
                try(InputStream input=descriptor.createInputStream()){bytes=SceneBytes.read(input,descriptor.getLength(),getCacheDir());}
            }
            String cached=checkpoints.cache(bytes);
            if(travel) {travelGame(game,bytes);candidate=game;}
            else {
                candidate=createGame(bytes);created=true;
                if(record!=null) {
                    restoreCheckpoint(candidate,record.state);gameCommand(candidate,0);gameAudio(candidate,1);
                    Log.i("RCCheckpoint","restored "+record.title);
                } else {
                    String destination=guidance(candidate),saveName=CampaignRoutes.saved(destination);
                    File saved=saveName==null?null:new File(getFilesDir(),saveName);
                    if(saved!=null && saved.exists() && !ignoreHandoff) {
                        if(saved.length()<1 || saved.length()>4*1024*1024)throw new IOException("Gespeicherte Kartenübergabe ist beschädigt");
                        try(InputStream in=new FileInputStream(saved)){restoreHandoff(candidate,SceneBytes.read(in,saved.length(),getCacheDir()));}
                        Log.i("RCPlay","Saved handoff restored on "+CampaignRoutes.map(destination));
                    }
                    if(ignoreHandoff && saved!=null)CheckpointStore.quarantine(saved);
                }
            }
            final GpuSceneView.World world=new GpuSceneView.World(gpuWorld(candidate));
            final byte[] bank=gameAudio(candidate,0);String guidance=guidance(candidate);
            // Rejected scenes/checkpoints must not replace the recovery scene.
            checkpoints.markScene(cached);
            if(candidate!=game){releaseGame(game);game=candidate;}created=false;
            currentScene=cached;savedTick=-1;savedMarker=0;checkpointPolicy.reset();restoredRecord=record;defeatRecorded=false;
            restoredTick=gameStatus(game)[5];
            // A previous-generation recovery leaves the rejected primary intact for diagnostics.
            if(record==null){
                try{checkpoints.saveNamed(CheckpointStore.ENTRY,cached,guidance,gameCheckpoint(game));}
                catch(IOException e){Log.e("RCCheckpoint","entry save failed",e);runOnUiThread(()->{if(!isDestroyed())Toast.makeText(this,"Karteneinstieg konnte nicht gespeichert werden: "+e.getMessage(),Toast.LENGTH_LONG).show();});}
                saveCheckpointWorker(true);
            }
            else {int[] values=gameStatus(game);savedTick=values[5];savedMarker=checkpointMarker(game);}
            Log.i("RCPlay","loaded "+(CampaignRoutes.map(guidance).isEmpty()?"geo_01a":CampaignRoutes.map(guidance).toLowerCase(Locale.ROOT))+" game="+game);
            final boolean restoring=record!=null;
            runOnUiThread(() -> {
                if(isDestroyed())return;
                if(sceneAudio!=null)sceneAudio.close();
                try {sceneAudio=new SceneAudio(this,bank);}catch(IOException e){failure(e.getMessage());open.setEnabled(true);return;}
                loading=false;nextMap="";open.setVisibility(View.GONE);lastGuidance=guidance;gpu.world(world);loaded=true;paused=restoring;finished=false;lastFrame=0;
                lastShots=0;lastHP=100;lastPhase=0;lastEnemies=3;lastAmmo=20;pause.setText(paused?"Fortsetzen":"Pause");open.setEnabled(true);open.setText("Szene öffnen");
                status.setText(restoring?"Checkpoint geladen · "+guidance:guidance);
                controls.clear();controls.invalidate();queueFrame(true);
            });
            return true;
        } catch(Exception e) {
            if(created)releaseGame(candidate);
            final Uri recoveryUri=failedUri;
            runOnUiThread(() -> recovery("Laden fehlgeschlagen: "+e.getMessage(),recoveryUri,record!=null));
            return false;
        }
    }
    private void recovery(String message,Uri uri,boolean checkpoint) {
        if(isDestroyed())return;
        loading=false;paused=true;open.setEnabled(true);pause.setText("Fortsetzen");status.setText(message);
        ArrayList<String> labels=new ArrayList<>();ArrayList<Runnable> actions=new ArrayList<>();
        labels.add("Sichere und manuelle Spielstände auswählen");actions.add(()->showCheckpointChoices("Wiederherstellung"));
        if(checkpoints.hasPrevious()) {
            labels.add("Vorherigen Checkpoint laden");actions.add(() -> {worker.execute(() -> {try {checkpoints.quarantine(false);}catch(IOException e){Log.e("RCCheckpoint","quarantine",e);}});loadCheckpoint(true);});
        }
        labels.add(uri==null?"Karte am Anfang starten":"Ohne gespeicherte Übergabe starten");
        actions.add(() -> {
            prepareLoad();worker.execute(() -> {
                try {
                    if(checkpoint)checkpoints.quarantine(false);
                    Uri source=uri;
                    if(source==null) {String name=checkpoints.lastScene();byte[] b=checkpoints.scene(name,getCacheDir());File file=new File(getCacheDir(),"checkpoint-recovery.rcplay");CheckpointStore.atomic(file,b);source=Uri.fromFile(file);}
                    loadSceneWorker(source,null,false,uri!=null);
                }catch(Exception e){runOnUiThread(() -> recovery("Wiederherstellung fehlgeschlagen: "+e.getMessage(),uri,false));}
            });
        });
        labels.add("Andere Szene auswählen");actions.add(() -> {pendingTravel=false;Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT);intent.setType("*/*");intent.addCategory(Intent.CATEGORY_OPENABLE);startActivityForResult(intent,1);});
        LinearLayout choices=new LinearLayout(this);choices.setOrientation(LinearLayout.VERTICAL);int pad=(int)(16*getResources().getDisplayMetrics().density);choices.setPadding(pad,0,pad,pad);
        TextView explanation=new TextView(this);explanation.setText(message+"\nDie bisherige Sitzung und der vorherige Checkpoint bleiben erhalten.");explanation.setTextSize(14);choices.addView(explanation);
        ScrollView scroll=new ScrollView(this);scroll.addView(choices);
        android.app.AlertDialog dialog=new android.app.AlertDialog.Builder(this).setTitle("Spielstand wiederherstellen").setView(scroll).setNegativeButton("Abbrechen",null).create();
        for(int i=0;i<labels.size();i++){final int index=i;Button choice=new Button(this);choice.setText(labels.get(i));choice.setOnClickListener(v -> {dialog.dismiss();actions.get(index).run();});choices.addView(choice);}
        dialog.show();
    }
    private void failure(String message) {
        runOnUiThread(() -> {if(!isDestroyed()) {paused=true; controls.clear(); lastFrame=0;
            if(sceneAudio!=null) sceneAudio.stop();
            status.setText("Spielfehler: "+message); pause.setText("Fortsetzen");}});
    }
    private void queueFrame(boolean still) {
        if(loading || !loaded || !resumed || isDestroyed() || drawing || (!still && (paused || finished))) return;
        if(!still && sceneAudio!=null)try {
            if(!sceneAudio.ready()){lastFrame=0;handler.removeCallbacks(pump);handler.postDelayed(pump,16);status.setText("Originalton wird vorbereitet …");return;}
        }catch(IOException e){failure(e.getMessage());return;}
        drawing=true;
        long now=System.nanoTime(); final long interval=lastFrame==0 ? 0 : now-lastFrame; double elapsed=still || lastFrame==0 ? 0 : Math.min(0.25,(now-lastFrame)/1e9); lastFrame=now;
        final int requestedGeneration=generation; final int requestedAudioGeneration=audioGeneration; final boolean renderGpu=useGpu;
        final float forward=controls.forward(), strafe=controls.strafe(), lookX=controls.lookX, lookY=controls.lookY;
        final int buttons=controls.buttons(); controls.lookX=0; controls.lookY=0; controls.reload=false; controls.fireQueued=false; controls.jumpQueued=false; controls.grenadeQueued=false;
        worker.execute(() -> {
            int[] pixels=null; GpuSceneView.Frame graphics=null; String error=null; byte[] soundEvents=null,ambientSnapshot=null; String missionText=""; String actorText="[]"; int energy=-1;
            final long workStart=System.nanoTime();
            final long[] metrics=profiling ? new long[26] : null;
            try {
                if(renderGpu) {graphics=new GpuSceneView.Frame(gpuFrame(game,elapsed,forward,strafe,lookX,lookY,buttons,metrics),still);pixels=graphics.status;}
                else pixels=gameFrame(game,elapsed,forward,strafe,lookX,lookY,buttons,640,400,metrics);
                soundEvents=gameAudio(game,4);
                ambientSnapshot=gameAudio(game,6);
                missionText=guidance(game);
                energy=gameEnergy(game);
                if(pixels[4]==1 && !still) {
                    byte[] handoff=mapHandoff(game);
                    if(handoff.length>0) {
                        org.json.JSONObject receipt=new org.json.JSONObject(new String(handoff,java.nio.charset.StandardCharsets.UTF_8));
                        String source=receipt.getString("source");
                        if(!source.equals("GEO_01A.ctm") && !source.equals("GEO_01B.ctm") && !source.equals("GEO_01C.ctm"))throw new IOException("Unbekannte Übergabequelle");
                        String filename=source.equals("GEO_01C.ctm")?"geo_01c-handoff.json":source.equals("GEO_01B.ctm")?"geo_01b-handoff.json":"geo_01a-handoff.json";
                        android.util.AtomicFile target=new android.util.AtomicFile(new java.io.File(getFilesDir(),filename));
                        java.io.FileOutputStream stream=null;
                        try {stream=target.startWrite();stream.write(handoff);target.finishWrite(stream);}
                        catch(IOException ex) {if(stream!=null)target.failWrite(stream);throw ex;}
                        Log.i("RCPlay","Map handoff saved: "+receipt.getString("destination")+" ("+handoff.length+" bytes)");
                    }
                }
                saveCheckpointWorker(false);
                if(profiling) actorText=actorStatus(game);
                String jumpState=Arrays.toString(jumpStatus(game));
                if(!jumpState.equals(lastJumpState)) {Log.i("RCJump","ticks="+pixels[5]+" "+jumpState);lastJumpState=jumpState;}
            } catch(Exception ex) {error=ex.getMessage();}
            final long workEnd=System.nanoTime();
            final byte[] events=soundEvents;
            final byte[] loops=ambientSnapshot;
            final String guidance=missionText;
            final int energyLevel=energy;
            final String actorObservation=actorText;
            final int[] result=pixels; final String problem=error; final GpuSceneView.Frame graphicsFrame=graphics;
            runOnUiThread(() -> {
                drawing=false; if(isDestroyed()) return;
                // Pause/backend generations may invalidate the frame, but its audio
                // has already been consumed from Rust. Keep it for this same scene.
                if(problem==null && sceneAudio!=null && events!=null && requestedAudioGeneration==audioGeneration) {
                    try {sceneAudio.events(events);if(generation==requestedGeneration && !still && loops!=null)sceneAudio.ambientLoops(loops);} catch(IOException ex) {failure(ex.getMessage());return;}
                }
                if(generation!=requestedGeneration) {if(loaded) queueFrame(true); return;}
                if(problem!=null) {failure(problem); return;}
                controls.setEnergy(energyLevel);
                long bitmapStart=System.nanoTime();
                if(renderGpu) gpu.submit(graphicsFrame);
                else {
                    if(bitmap==null) bitmap=Bitmap.createBitmap(640,400,Bitmap.Config.ARGB_8888);
                    bitmap.setPixels(result,10,640,0,0,640,400); image.setImageBitmap(bitmap);
                }
                long bitmapEnd=System.nanoTime();
                if(profiling) recordPerformance(result,metrics,now,workStart,workEnd,bitmapStart,bitmapEnd,interval,still,forward,strafe,lookX,lookY,buttons,actorObservation);
                if(audio!=null && (sceneAudio==null || !sceneAudio.hasOriginalSounds()) && result[6]>lastShots) audio.startTone(ToneGenerator.TONE_PROP_BEEP,35);
                else if(audio!=null && (sceneAudio==null || !sceneAudio.hasOriginalSounds()) && result[0]<lastHP) audio.startTone(ToneGenerator.TONE_PROP_NACK,50);
                if(result[0]!=lastHP || result[1]!=lastAmmo || result[3]!=lastEnemies || result[4]!=lastPhase || result[6]!=lastShots)
                    Log.i("RCPlay","state hp="+result[0]+" ammo="+result[1]+" alive="+result[3]+" phase="+result[4]+" ticks="+result[5]+" shots="+result[6]+" kills="+result[7]+" jumps="+result[8]);
                lastShots=result[6]; lastHP=result[0]; lastPhase=result[4]; lastEnemies=result[3]; lastAmmo=result[1];
                if(!guidance.isEmpty()){lastGuidance=guidance;if(!paused)status.setText(guidance);}
                if(result[4]!=0) {
                    finished=true;controls.clear();
                    if(result[4]==1){
                        nextMap=CampaignRoutes.next(guidance);open.setVisibility(nextMap.isEmpty()?View.GONE:View.VISIBLE);open.setText("Weiter");
                        if(campaignMode && !nextMap.isEmpty()){
                            final String destination=nextMap;final int travelGeneration=generation;
                            handler.postDelayed(()->{if(resumed && !isDestroyed() && !loading && generation==travelGeneration && destination.equals(nextMap))loadCampaign(destination,true);},600);
                        }
                    }
                    status.setText(result[4]==1?"Abschnitt abgeschlossen · "+CampaignRoutes.display(nextMap):"Gefallen · Checkpoint im Menü laden");
                }
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
    @Override protected void onResume() {super.onResume(); gpu.onResume(); resumed=true; lastFrame=0; if(sceneAudio!=null && !paused) sceneAudio.resume(); if(loaded) queueFrame(true);}
    @Override protected void onPause() {
        resumed=false;generation++;if(loaded && !finished)paused=true;pause.setText("Fortsetzen"); if(sceneAudio!=null) sceneAudio.suspend(); handler.removeCallbacks(pump); lastFrame=0; if(controls!=null) controls.clear();
        worker.execute(() -> {if(game!=0) {try {gameCommand(game,0);saveCheckpointWorker(true);} catch(Exception e) {Log.e("RCPlay","suspend",e);} }});
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
        private boolean fire,jump,reload,fireQueued,jumpQueued,w,s,a,d,walk,use,grenade,grenadeQueued,crouch;
        private int energy=-1;
        void setEnergy(int level) {
            if(energy==level)return;
            if((energy<0)!=(level<0))setContentDescription(level>=0?"Links Joystick; rechts Blick drehen; Feuer, Sprung, Energieanzeige, Benutzen, Granate und Ducken":"Links Joystick; rechts Blick drehen; Feuer, Sprung, Laden, Benutzen, Granate und Ducken");
            energy=level;invalidate();
        }
        Controls() {super(PlayActivity.this); setContentDescription("Links Joystick; rechts Blick drehen; Feuer, Sprung, Laden, Benutzen, Granate und Ducken"); setFocusable(true);}
        private TouchLayout layout(){return new TouchLayout(getWidth(),getHeight(),getResources().getDisplayMetrics().density);}
        @Override protected void onSizeChanged(int w,int h,int oldw,int oldh){
            super.onSizeChanged(w,h,oldw,oldh);TouchLayout l=layout();
            StringBuilder details=new StringBuilder("{\"width\":").append(w).append(",\"height\":").append(h).append(",\"density\":").append(getResources().getDisplayMetrics().density).append(",\"joystick\":[").append(l.joystickX).append(',').append(l.joystickY).append(',').append(l.joystickRadius).append("],\"keys\":[");
            for(int i=0;i<l.keys.length;i++){TouchLayout.Key k=l.keys[i];if(i>0)details.append(',');details.append('[').append(k.role).append(',').append(k.x).append(',').append(k.y).append(',').append(k.r).append(']');}Log.i("RCTouch",details.append("]}").toString());
        }
        float forward() {return Math.max(-1,Math.min(1,-dy+(w?1:0)-(s?1:0)));}
        float strafe() {return Math.max(-1,Math.min(1,dx+(d?1:0)-(a?1:0)));}
        int buttons() {return (jump||jumpQueued?1:0)|(fire||fireQueued?2:0)|(reload?4:0)|(walk?8:0)|(use?16:0)|(grenade||grenadeQueued?32:0)|(crouch?64:0);}
        void clear() {roles.clear(); dx=dy=lookX=lookY=0; fire=jump=reload=fireQueued=jumpQueued=w=s=a=d=walk=use=grenade=grenadeQueued=crouch=false; invalidate();}
        boolean key(int key,boolean down) {
            switch(key) {
                case KeyEvent.KEYCODE_W: case KeyEvent.KEYCODE_DPAD_UP:w=down;return true;
                case KeyEvent.KEYCODE_S: case KeyEvent.KEYCODE_DPAD_DOWN:s=down;return true;
                case KeyEvent.KEYCODE_A: case KeyEvent.KEYCODE_DPAD_LEFT:a=down;return true;
                case KeyEvent.KEYCODE_D: case KeyEvent.KEYCODE_DPAD_RIGHT:d=down;return true;
                case KeyEvent.KEYCODE_SPACE:jump=down;if(down) jumpQueued=true;return true;
                case KeyEvent.KEYCODE_CTRL_LEFT:fire=down;if(down) fireQueued=true;return true;
                case KeyEvent.KEYCODE_SHIFT_LEFT:walk=down;return true;
                case KeyEvent.KEYCODE_R:if(down && energy<0) reload=true;return true;
                case KeyEvent.KEYCODE_E:use=down;return true;
                case KeyEvent.KEYCODE_C: crouch=down;return true;
                case KeyEvent.KEYCODE_G:if(down && !grenade) grenadeQueued=true;grenade=down;return true;
                case KeyEvent.KEYCODE_ENTER:if(down) restart();return true;
                default:return false;
            }
        }
        private boolean inside(float x,float y,float cx,float cy,float r) {return Math.hypot(x-cx,y-cy)<r;}
        @Override public boolean onTouchEvent(MotionEvent e) {
            if(!loaded || paused || finished) return true;
            int action=e.getActionMasked(), index=e.getActionIndex();TouchLayout layout=layout();float width=getWidth();
            if(action==MotionEvent.ACTION_CANCEL) {clear();return true;}
            if(action==MotionEvent.ACTION_DOWN || action==MotionEvent.ACTION_POINTER_DOWN) {
                int id=e.getPointerId(index); float x=e.getX(index),y=e.getY(index); int role;
                role=0;for(TouchLayout.Key key:layout.keys)if(key.contains(x,y)){role=key.role;break;}
                if(role==2){fire=true;fireQueued=true;}
                else if(role==3){jump=true;jumpQueued=true;}
                else if(role==4 && energy<0)reload=true;
                else if(role==6)use=true;
                else if(role==7){if(!grenade)grenadeQueued=true;grenade=true;}
                else if(role==8)crouch=true;
                else if(x<width*.4f && !roles.containsValue(1))role=1;
                else if(!roles.containsValue(5)){role=5;lastX=x;lastY=y;}
                roles.put(id,role);
            }
            // Release events can carry the final look position even when Android coalesces MOVE events.
            if(action==MotionEvent.ACTION_MOVE || action==MotionEvent.ACTION_DOWN || action==MotionEvent.ACTION_POINTER_DOWN || action==MotionEvent.ACTION_UP || action==MotionEvent.ACTION_POINTER_UP) {
                for(int i=0;i<e.getPointerCount();i++) {
                    int role=roles.getOrDefault(e.getPointerId(i),0); float x=e.getX(i),y=e.getY(i);
                    if(role==1) {dx=(x-layout.joystickX)/layout.joystickRadius;dy=(y-layout.joystickY)/layout.joystickRadius;float norm=Math.max(1,(float)Math.hypot(dx,dy));dx/=norm;dy/=norm;}
                    if(role==5) {lookX=Math.max(-6,Math.min(6,lookX+TouchLayout.look(x-lastX,getResources().getDisplayMetrics().density,ControlSettings.sensitivity(PlayActivity.this))));lookY=Math.max(-6,Math.min(6,lookY+TouchLayout.look(y-lastY,getResources().getDisplayMetrics().density,ControlSettings.sensitivity(PlayActivity.this))));lastX=x;lastY=y;}
                }
            }
            if(action==MotionEvent.ACTION_UP || action==MotionEvent.ACTION_POINTER_UP) {
                Integer role=roles.remove(e.getPointerId(index));
                if(role!=null && role==1) {dx=dy=0;}
                if(role!=null && role==2) fire=roles.containsValue(2);
                if(role!=null && role==3) jump=roles.containsValue(3);
                if(role!=null && role==6) use=roles.containsValue(6);
                if(role!=null && role==7) grenade=roles.containsValue(7);
                if(role!=null && role==8) crouch=roles.containsValue(8);
                if(action==MotionEvent.ACTION_UP) performClick();
            }
            invalidate(); return true;
        }
        @Override public boolean performClick() {super.performClick();return true;}
        private void circle(Canvas canvas,float x,float y,float r,String text,boolean active) {
            paint.setColor(active ? 0xb0e6a658 : 0x804d7278); canvas.drawCircle(x,y,r,paint);
            paint.setColor(0xffe4f4ed); paint.setTextAlign(Paint.Align.CENTER);paint.setTextSize(Math.max(11*getResources().getDisplayMetrics().scaledDensity,r*.38f));
            if(!text.isEmpty() && paint.measureText(text)>r*1.85f)paint.setTextSize(paint.getTextSize()*r*1.85f/paint.measureText(text));canvas.drawText(text,x,y+r*0.12f,paint);
        }
        @Override protected void onDraw(Canvas canvas) {
            if(!loaded) return; TouchLayout layout=layout();float width=getWidth(),height=getHeight();
            circle(canvas,layout.joystickX,layout.joystickY,layout.joystickRadius,"",false);
            circle(canvas,layout.joystickX+dx*layout.joystickRadius*.6f,layout.joystickY+dy*layout.joystickRadius*.6f,layout.joystickRadius*.38f,"",true);
            for(TouchLayout.Key key:layout.keys){boolean active=key.role==2?fire:key.role==3?jump:key.role==4?(energy>=0?energy<1000:reload):key.role==6?use:key.role==7?grenade:crouch;String label=key.role==4 && energy>=0?"E "+Math.round(energy/10f)+"%":key.label;circle(canvas,key.x,key.y,key.r,label,active);}
        }
    }
}
