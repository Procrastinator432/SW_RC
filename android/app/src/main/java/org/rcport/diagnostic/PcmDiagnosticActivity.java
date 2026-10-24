package org.rcport.diagnostic;

import android.app.Activity;
import android.content.Intent;
import android.os.*;
import android.widget.*;
import java.io.*;
import java.util.*;
import java.util.concurrent.*;

/** Explicit PCM sink acceptance, separate from the mission's SoundPool host. */
public final class PcmDiagnosticActivity extends Activity {
    private final ExecutorService files=Executors.newSingleThreadExecutor();
    private final Handler ui=new Handler(Looper.getMainLooper());
    private List<PcmPacket> packets;
    private PcmTrackOutput output;
    private TextView status;
    private Button load,start;
    private volatile long generation;
    private boolean visible;
    private void message(String text){status.setText(text);}
    @Override public void onCreate(Bundle state){
        super.onCreate(state);
        LinearLayout layout=new LinearLayout(this);layout.setOrientation(1);layout.setPadding(24,16,24,16);
        status=new TextView(this);status.setTextSize(18);layout.addView(status);
        message("PCM-Ausgabediagnose · Stereo S16\nOpenAL-Testmix (.rcpcm) importieren. Start beginnt am Dateianfang; Pause verwirft gepufferte Daten.");
        load=new Button(this);load.setText("PCM-Testmix importieren");layout.addView(load);
        load.setOnClickListener(v->startActivityForResult(new Intent(Intent.ACTION_OPEN_DOCUMENT).setType("*/*").addCategory(Intent.CATEGORY_OPENABLE),1));
        start=new Button(this);start.setText("Testmix starten");start.setEnabled(false);layout.addView(start);start.setOnClickListener(v->play());
        Button pause=new Button(this);pause.setText("Stopp / Puffer leeren");layout.addView(pause);pause.setOnClickListener(v->stop("Gestoppt · erneuter Start beginnt am Anfang"));
        ScrollView scroll=new ScrollView(this);scroll.addView(layout);setContentView(scroll);
    }
    @Override protected void onResume(){super.onResume();visible=true;}
    private void stop(String text){generation++;if(output!=null){output.close();output=null;}message(text);}
    private void play(){
        stop("PCM-Ausgabe wird vorbereitet …");if(!visible || packets==null)return;
        final long token=generation;final List<PcmPacket> source=packets;
        output=new PcmTrackOutput(this,source.get(0).rate,()->{if(token==generation)stop("Audiofokus verloren · zum Neustart Start wählen");});
        final PcmTrackOutput current=output;
        if(!current.start()){stop("Kein Audiofokus oder Ausgabefehler");return;}
        message("Testmix läuft · "+source.get(0).rate+" Hz");
        files.execute(()->{
            try{
                long epoch=current.pump.epoch(),sequence=0;
                for(PcmPacket packet:source){
                    PcmPacket tagged=packet.retag(epoch,sequence++);
                    while(!current.pump.offer(tagged)){
                        if(token!=generation || !current.pump.isPlaying())return;
                        Thread.sleep(10);
                    }
                    if(token!=generation)return;
                }
                if(token==generation)current.pump.finish();
            }catch(Exception e){ui.post(()->{if(token==generation)stop("PCM-Fehler: "+e.getMessage());});}
        });
        monitor(token,current);
    }
    private void monitor(long token,PcmTrackOutput current){
        ui.postDelayed(()->{if(token!=generation)return;if(current.pump.failure()!=null)stop(current.pump.failure());else if(!current.pump.isPlaying())stop("PCM-Testmix beendet · Geräteklang separat abnehmen");else monitor(token,current);},250);
    }
    @Override protected void onActivityResult(int request,int result,Intent data){
        super.onActivityResult(request,result,data);if(request!=1 || result!=RESULT_OK || data==null || data.getData()==null)return;
        stop("PCM-Testmix wird gelesen …");final long token=generation;load.setEnabled(false);start.setEnabled(false);
        final android.net.Uri uri=data.getData();
        files.execute(()->{
            List<PcmPacket> imported=null;String problem=null;
            try(InputStream input=getContentResolver().openInputStream(uri)){if(input==null)throw new IOException("Datei fehlt");imported=PcmPacket.readPackage(input);}
            catch(Exception e){problem=e.getMessage();}
            final List<PcmPacket> ready=imported;final String error=problem;
            ui.post(()->{if(isDestroyed())return;load.setEnabled(true);if(token!=generation){start.setEnabled(packets!=null);return;}
                if(error!=null)message("Import fehlgeschlagen: "+error);else{packets=ready;message(ready.size()+" PCM-Pakete bereit · Start zum Prüfen");}start.setEnabled(packets!=null);});
        });
    }
    @Override protected void onPause(){visible=false;stop("Ausgabe angehalten · zum Neustart Start wählen");super.onPause();}
    @Override protected void onDestroy(){stop("Geschlossen");ui.removeCallbacksAndMessages(null);files.shutdownNow();super.onDestroy();}
}
