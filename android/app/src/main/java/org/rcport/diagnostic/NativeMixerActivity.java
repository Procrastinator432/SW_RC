package org.rcport.diagnostic;

import android.app.Activity;
import android.content.Intent;
import android.os.*;
import android.widget.*;
import java.io.*;
import java.util.concurrent.*;

/** Actual Android OpenAL -> Rust PCM packets -> AudioTrack, separate from the mission host. */
public final class NativeMixerActivity extends Activity {
    private final ExecutorService producer=Executors.newSingleThreadExecutor(r->new Thread(r,"rc-native-mixer"));
    private final Handler ui=new Handler(Looper.getMainLooper());
    private volatile long generation;
    private volatile long writableGeneration=-1;
    private byte[] wave;
    private boolean visible;
    private TextView status;
    private Button start,load;
    private PcmTrackOutput output;
    private boolean mixed;
    private ArrayBlockingQueue<String> controls=new ArrayBlockingQueue<>(8);
    private android.util.AtomicFile checkpoint(){return new android.util.AtomicFile(new File(getFilesDir(),"mixed-audio-checkpoint.rcmix"));}
    private void stop(String message){generation++;writableGeneration=-1;controls.clear();controls=new ArrayBlockingQueue<>(8);if(output!=null){output.close();output=null;}status.setText(message);}
    private void control(String action){if(output==null || !output.pump.isPlaying() || writableGeneration!=generation){status.setText("Zuerst den gemischten Mixer starten");return;}if(!controls.offer(action))status.setText("Befehlsqueue voll · erneut versuchen");}
    private Button action(LinearLayout layout,String text,Runnable run){Button b=new Button(this);b.setText(text);b.setOnClickListener(v->run.run());layout.addView(b);return b;}
    @Override public void onCreate(Bundle state){
        super.onCreate(state);mixed=getIntent().getBooleanExtra("mixed",false);LinearLayout layout=new LinearLayout(this);layout.setOrientation(1);layout.setPadding(24,16,24,16);
        status=new TextView(this);status.setTextSize(18);status.setText("Nativer OpenAL-Live-Test\nEine PCM-WAV (8/16 Bit, Mono/Stereo, höchstens 2 MiB) importieren. Der Mixer wiederholt sie fünf Sekunden lang. Start beginnt neu.");layout.addView(status);
        load=new Button(this);load.setText("WAV importieren");layout.addView(load);load.setOnClickListener(v->startActivityForResult(new Intent(Intent.ACTION_OPEN_DOCUMENT).setType("*/*").addCategory(Intent.CATEGORY_OPENABLE),1));
        start=new Button(this);start.setText("Nativen Mixer starten");start.setEnabled(false);layout.addView(start);start.setOnClickListener(v->play());
        Button stop=new Button(this);stop.setText("Stopp");layout.addView(stop);stop.setOnClickListener(v->stop("Gestoppt · neuer Start lädt die gewählte Audiodatei"));
        if(mixed){
            status.setText("Gemischter WAV-/Vorbis-Host\n.rcmix-Paket importieren. Pause steuert die Testquelle 1001; der Audiotakt läuft weiter. Ein gespeicherter Audiozustand kann separat fortgesetzt werden.");load.setText("Gemischtes Audiopaket importieren");
            action(layout,"Stream 1001 pausieren",()->control("{\"Pause\":{\"actor\":42,\"sound\":1001,\"paused\":true}}"));
            action(layout,"Stream 1001 fortsetzen",()->control("{\"Pause\":{\"actor\":42,\"sound\":1001,\"paused\":false}}"));
            action(layout,"Akteur nach links",()->control("{\"Actor\":{\"id\":42,\"location\":[-10,0,0],\"velocity\":[0,0,0],\"valid\":true}}"));
            action(layout,"Akteur nach rechts",()->control("{\"Actor\":{\"id\":42,\"location\":[10,0,0],\"velocity\":[0,0,0],\"valid\":true}}"));
            action(layout,"Audiozustand speichern",()->control("SAVE"));
            action(layout,"Audiozustand fortsetzen",()->{
                stop("Audiozustand wird geladen …");long token=generation;
                producer.execute(()->{try(InputStream input=checkpoint().openRead()){byte[] saved=readBounded(input,16*1024*1024);ui.post(()->{if(token==generation && visible){wave=saved;start.setEnabled(true);play();}});}catch(Exception e){ui.post(()->{if(token==generation)status.setText("Kein nutzbarer Audiozustand: "+e.getMessage());});}});
            });
        }
        ScrollView scroll=new ScrollView(this);scroll.addView(layout);setContentView(scroll);
    }
    @Override protected void onResume(){super.onResume();visible=true;}
    private void play(){
        stop("Nativer Mixer wird vorbereitet …");if(!visible || wave==null)return;
        final long token=generation;final byte[] input=wave;final int rate=48000;final ArrayBlockingQueue<String> inputControls=controls;
        output=new PcmTrackOutput(this,rate,()->{if(token==generation)stop("Audiofokus verloren · zum Neustart Start wählen");});
        final PcmTrackOutput current=output;
        if(!current.start()){stop("Kein Audiofokus oder Ausgabefehler");return;}
        status.setText(mixed?"Gemischter nativer Host · WAV und Vorbis · 48 kHz":"Nativer OpenAL-Mixer · Stereo S16 / 48 kHz · fünf Sekunden");
        final String library=getApplicationInfo().nativeLibraryDir+"/libopenal.so";
        producer.execute(()->{
            try{
                PcmLiveProducer.produce(current.pump,epoch->{
                    long id=mixed?NativeMixer.mixedOpen(library,input,epoch):NativeMixer.open(library,input,rate,epoch);
                    try {
                    byte[] info=mixed?NativeMixer.mixedExport(id,2):null;
                    java.nio.ByteBuffer metadata=info==null?null:java.nio.ByteBuffer.wrap(info).order(java.nio.ByteOrder.LITTLE_ENDIAN);
                    final int remaining=metadata==null?-1:Math.toIntExact(metadata.getLong());
                    final long firstControl=metadata==null?0:metadata.getLong();
                    if(mixed)writableGeneration=token;
                    return new PcmLiveProducer.Source(){
                        long nextControl=firstControl;
                        public int remainingFrames(){return remaining;}
                        public byte[] render(int frames,long e,long sequence)throws Exception{
                            for(int i=0;i<8;i++){String action=inputControls.poll();if(action==null)break;
                                if(action.equals("SAVE")){
                                    byte[] saved=NativeMixer.mixedExport(id,1);android.util.AtomicFile store=checkpoint();FileOutputStream file=null;
                                    try{file=store.startWrite();file.write(saved);store.finishWrite(file);}catch(Exception error){if(file!=null)store.failWrite(file);throw error;}
                                    ui.post(()->{if(token==generation)status.setText("Audiozustand gespeichert · Ausgabe läuft weiter");});
                                }else{
                                    String json="{\"sequence\":"+nextControl+",\"action\":"+action+"}";
                                    NativeMixer.mixedControl(id,json.getBytes(java.nio.charset.StandardCharsets.UTF_8));nextControl++;
                                }
                            }
                            return NativeMixer.render(id,frames,e,sequence);
                        }
                        public void close()throws Exception{if(writableGeneration==token)writableGeneration=-1;NativeMixer.release(id);}
                    };
                    }catch(Exception | LinkageError error){NativeMixer.release(id);throw error;}
                },rate*(mixed?10:5),480,()->token!=generation);
            }catch(Throwable error){ui.post(()->{if(token==generation)stop("Nativer Mixer: "+error.getMessage());});}
        });
        monitor(token,current);
    }
    private void monitor(long token,PcmTrackOutput current){ui.postDelayed(()->{
        if(token!=generation)return;
        if(current.pump.failure()!=null)stop(current.pump.failure());
        else if(!current.pump.isPlaying())stop("Native PCM-Ausgabe beendet · Hörabnahme am Gerät separat bestätigen");
        else monitor(token,current);
    },250);}
    @Override protected void onActivityResult(int request,int result,Intent data){
        super.onActivityResult(request,result,data);if(request!=1 || result!=RESULT_OK || data==null || data.getData()==null)return;
        stop("WAV wird gelesen …");long token=generation;load.setEnabled(false);start.setEnabled(false);final android.net.Uri uri=data.getData();
        producer.execute(()->{
            byte[] imported=null;String problem=null;
            try(InputStream input=getContentResolver().openInputStream(uri)){
                if(input==null)throw new IOException("Datei fehlt");imported=readBounded(input,(mixed?16:2)*1024*1024);
                if(imported.length<(mixed?2:44))throw new IOException("Audiodatei zu kurz");
            }catch(Exception e){problem=e.getMessage();}
            final byte[] ready=imported;final String error=problem;ui.post(()->{
                if(isDestroyed())return;load.setEnabled(true);if(token==generation){if(error==null){wave=ready;status.setText(mixed?"Gemischtes Paket geladen · Start öffnet den Host":"WAV geladen · Start prüft Format und öffnet den nativen Mixer");}else status.setText("Import fehlgeschlagen: "+error);}start.setEnabled(wave!=null);
            });
        });
    }
    private static byte[] readBounded(InputStream input,int limit)throws IOException{
        ByteArrayOutputStream bytes=new ByteArrayOutputStream();byte[] chunk=new byte[8192];int n;
        while((n=input.read(chunk))!=-1){if(bytes.size()>limit-n)throw new IOException("Audiodatei zu groß");bytes.write(chunk,0,n);}return bytes.toByteArray();
    }
    @Override protected void onPause(){visible=false;stop("Angehalten · neuer Start beginnt am Anfang");super.onPause();}
    @Override protected void onDestroy(){stop("Geschlossen");ui.removeCallbacksAndMessages(null);producer.shutdownNow();super.onDestroy();}
}
