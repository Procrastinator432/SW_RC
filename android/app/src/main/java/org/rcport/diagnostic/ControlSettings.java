package org.rcport.diagnostic;

import android.app.*;
import android.content.*;
import android.widget.*;

final class ControlSettings {
    static float sensitivity(Context context){return context.getSharedPreferences("controls",0).getFloat("sensitivity",1f);}
    static void show(Activity activity){
        LinearLayout layout=new LinearLayout(activity);layout.setOrientation(LinearLayout.VERTICAL);
        int pad=(int)(16*activity.getResources().getDisplayMetrics().density);layout.setPadding(pad,pad,pad,pad);
        TextView value=new TextView(activity);value.setTextSize(18);layout.addView(value);
        SeekBar seek=new SeekBar(activity);seek.setMax(150);seek.setProgress(Math.round((sensitivity(activity)-.5f)*100));layout.addView(seek,new LinearLayout.LayoutParams(-1,Math.round(48*activity.getResources().getDisplayMetrics().density)));
        Runnable update=() -> value.setText("Blickempfindlichkeit: "+(seek.getProgress()+50)+" %");update.run();
        seek.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener(){public void onProgressChanged(SeekBar b,int p,boolean user){update.run();}public void onStartTrackingTouch(SeekBar b){}public void onStopTrackingTouch(SeekBar b){}});
        TextView hint=new TextView(activity);hint.setText("Links bewegen · freie Fläche rechts zum Zielen. Gleiche Empfindlichkeit für beide Blickachsen. Aktionsflächen mindestens 48 dp.");layout.addView(hint);
        ScrollView scroll=new ScrollView(activity);scroll.addView(layout);
        new AlertDialog.Builder(activity).setTitle("Steuerung").setView(scroll).setPositiveButton("Speichern",(d,w)->activity.getSharedPreferences("controls",0).edit().putFloat("sensitivity",.5f+seek.getProgress()/100f).apply()).setNeutralButton("Standard",(d,w)->activity.getSharedPreferences("controls",0).edit().putFloat("sensitivity",1f).apply()).setNegativeButton("Abbrechen",null).show();
    }
}
