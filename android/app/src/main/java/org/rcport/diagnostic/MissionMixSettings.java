package org.rcport.diagnostic;
import android.app.*;
import android.content.*;
import android.widget.*;
import java.util.function.Consumer;

final class MissionMixSettings {
    private static final String[] KEYS={"master","effects","dialog","ambient","music"};
    private static final String[] LABELS={"Gesamt","Waffen und Geräusche","Sprache","Umgebung","Musik"};
    static MissionMix load(Context context){
        SharedPreferences prefs=context.getSharedPreferences("rcMissionMix",Context.MODE_PRIVATE);int[] values=new int[5];
        for(int i=0;i<5;i++){try{values[i]=Math.max(0,Math.min(100,prefs.getInt(KEYS[i],100)));}catch(ClassCastException bad){values[i]=100;}}
        return new MissionMix(values);
    }
    static void show(Activity activity,Consumer<MissionMix> saved){
        MissionMix initial=load(activity);LinearLayout list=new LinearLayout(activity);list.setOrientation(LinearLayout.VERTICAL);
        int pad=Math.round(16*activity.getResources().getDisplayMetrics().density);list.setPadding(pad,pad,pad,pad);
        TextView hint=new TextView(activity);hint.setText("Lautstärke für den nativen Mixer. Änderungen gelten nach dem Speichern; 100 % entspricht dem bisherigen Pegel.");list.addView(hint);
        SeekBar[] controls=new SeekBar[5];
        for(int i=0;i<5;i++){
            final int index=i;TextView label=new TextView(activity);label.setText(LABELS[i]+": "+initial.value(i)+" %");list.addView(label);
            SeekBar control=new SeekBar(activity);controls[i]=control;control.setMax(100);control.setProgress(initial.value(i));control.setContentDescription(LABELS[i]);
            control.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener(){public void onStartTrackingTouch(SeekBar b){}public void onStopTrackingTouch(SeekBar b){}public void onProgressChanged(SeekBar b,int value,boolean user){label.setText(LABELS[index]+": "+value+" %");}});
            list.addView(control,new LinearLayout.LayoutParams(-1,Math.round(48*activity.getResources().getDisplayMetrics().density)));
        }
        ScrollView scroll=new ScrollView(activity);scroll.addView(list);
        new AlertDialog.Builder(activity).setTitle("Lautstärke – nativer Mixer").setView(scroll).setNegativeButton("Abbrechen",null)
            .setNeutralButton("Standard",(d,w)->save(activity,MissionMix.defaults(),saved))
            .setPositiveButton("Speichern",(d,w)->save(activity,new MissionMix(controls[0].getProgress(),controls[1].getProgress(),controls[2].getProgress(),controls[3].getProgress(),controls[4].getProgress()),saved)).show();
    }
    private static void save(Context context,MissionMix mix,Consumer<MissionMix> saved){
        SharedPreferences.Editor editor=context.getSharedPreferences("rcMissionMix",Context.MODE_PRIVATE).edit();for(int i=0;i<5;i++)editor.putInt(KEYS[i],mix.value(i));editor.apply();saved.accept(mix);
    }
}
