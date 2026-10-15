package org.rcport.diagnostic;

import android.app.*;
import android.content.*;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.net.Uri;
import android.os.Bundle;
import android.widget.*;
import java.io.*;
import java.text.DateFormat;
import java.util.Date;
import java.util.concurrent.*;

/** Campaign launcher; package import is the only normal file selection. */
public final class MainActivity extends Activity {
    private final ExecutorService worker=Executors.newSingleThreadExecutor();
    private Button start,continueGame,importPackage;
    private TextView packageStatus,saveStatus;
    private boolean importing;
    private CampaignStore campaign;
    private int dp(int value){return Math.round(value*getResources().getDisplayMetrics().density);}
    private Button button(LinearLayout layout,String text,Runnable action){
        Button b=new Button(this);b.setText(text);b.setTextSize(16);b.setMinHeight(dp(48));b.setOnClickListener(v->action.run());layout.addView(b,new LinearLayout.LayoutParams(-1,-2));return b;
    }
    @Override public void onCreate(Bundle state){
        super.onCreate(state);campaign=new CampaignStore(getFilesDir());
        LinearLayout layout=new LinearLayout(this);layout.setOrientation(LinearLayout.VERTICAL);layout.setPadding(dp(20),dp(12),dp(20),dp(12));layout.setBackgroundColor(Color.rgb(16,25,35));
        TextView title=new TextView(this);title.setText("REPUBLIC COMMANDO");title.setTextColor(Color.rgb(222,239,235));title.setTextSize(26);layout.addView(title);
        TextView subtitle=new TextView(this);subtitle.setText("Mission 1 · Geonosis");subtitle.setTextColor(Color.rgb(159,201,200));subtitle.setTextSize(16);layout.addView(subtitle);
        continueGame=button(layout,"Fortsetzen",()->play(true));continueGame.setBackgroundTintList(ColorStateList.valueOf(Color.rgb(139,209,193)));
        saveStatus=new TextView(this);saveStatus.setTextColor(Color.LTGRAY);saveStatus.setTextSize(13);layout.addView(saveStatus);
        start=button(layout,"Neue Mission starten",()->{
            if(new CheckpointStore(getFilesDir()).hasSave())new AlertDialog.Builder(this).setTitle("Neue Mission starten?").setMessage("Der letzte Checkpoint wird durch den neuen Missionseinstieg ersetzt.").setPositiveButton("Starten",(d,w)->play(false)).setNegativeButton("Abbrechen",null).show();
            else play(false);
        });
        importPackage=button(layout,"Szenenpaket importieren",()->{
            Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT).setType("*/*").addCategory(Intent.CATEGORY_OPENABLE);startActivityForResult(intent,10);
        });
        packageStatus=new TextView(this);packageStatus.setTextColor(Color.LTGRAY);packageStatus.setTextSize(13);layout.addView(packageStatus);
        button(layout,"Steuerung",()->ControlSettings.show(this));
        Button diagnostics=button(layout,"Diagnose",()->startActivity(new Intent(this,DiagnosticActivity.class)));diagnostics.setTextSize(12);
        ScrollView scroll=new ScrollView(this);scroll.setFillViewport(true);scroll.addView(layout);setContentView(scroll);
    }
    private void play(boolean resume){
        Intent intent=new Intent(this,PlayActivity.class).putExtra("rcCampaign",campaign.ready()).putExtra("rcProfile",getIntent().getBooleanExtra("rcProfile",false)).putExtra("rcSoftware",getIntent().getBooleanExtra("rcSoftware",false));
        if(resume)intent.putExtra("rcContinue",true);else intent.putExtra("rcMap","GEO_01A").putExtra("rcFresh",true);
        startActivity(intent);
    }
    @Override protected void onResume(){super.onResume();if(!importing)refresh();}
    private void refresh(){
        boolean ready=campaign.ready();start.setEnabled(ready);importPackage.setEnabled(true);
        packageStatus.setText(ready?"Szenenpaket bereit · vier Karten lokal verfügbar":"Einmal das Mission-1-Szenenpaket (.zip) importieren.");
        CheckpointStore saves=new CheckpointStore(getFilesDir());continueGame.setEnabled(saves.hasSave());
        if(!saves.hasSave())saveStatus.setText("Noch kein Spielstand");
        else try {CheckpointStore.Record record=saves.load(false);saveStatus.setText(CampaignRoutes.display(CampaignRoutes.map(record.title))+" · "+DateFormat.getDateTimeInstance(DateFormat.SHORT,DateFormat.SHORT).format(new Date(record.savedAt)));}
        catch(IOException e){saveStatus.setText("Spielstand wiederherstellen · Fortsetzen wählen");}
    }
    @Override protected void onActivityResult(int request,int result,Intent data){
        super.onActivityResult(request,result,data);if(request!=10 || result!=RESULT_OK || data==null || data.getData()==null)return;
        Uri uri=data.getData();importing=true;start.setEnabled(false);continueGame.setEnabled(false);importPackage.setEnabled(false);packageStatus.setText("Szenenpaket wird importiert …");
        worker.execute(()->{
            String problem=null;
            try(InputStream input=getContentResolver().openInputStream(uri)){
                if(input==null)throw new IOException("Paket konnte nicht geöffnet werden");
                campaign.importPackage(input,PlayActivity::validateImport,message->runOnUiThread(()->{if(!isDestroyed())packageStatus.setText(message);}));
            }catch(Exception e){problem=e.getMessage();}
            final String error=problem;runOnUiThread(()->{
                if(isDestroyed())return;importing=false;refresh();
                if(error==null)Toast.makeText(this,"Szenenpaket importiert",Toast.LENGTH_LONG).show();
                else new AlertDialog.Builder(this).setTitle("Import fehlgeschlagen").setMessage(error+"\nEin vorhandenes Paket und seine Spielstände bleiben erhalten.").setPositiveButton("OK",null).show();
            });
        });
    }
    @Override protected void onDestroy(){worker.shutdown();super.onDestroy();}
}
