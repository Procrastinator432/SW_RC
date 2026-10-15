/* Actual map1 input completion -> production map2 travel and persisted restore. */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
extern uint64_t rc_game_create(const uint8_t*,size_t,char*,size_t);
extern int32_t rc_game_advance(uint64_t,double,float,float,float,float,uint32_t);
extern int32_t rc_game_status(uint64_t,uint32_t*,size_t);
extern int32_t rc_game_squad_status(uint64_t,uint32_t*,size_t);
extern int32_t rc_game_observe(uint64_t,double*,size_t);
extern int32_t rc_game_travel(uint64_t,const uint8_t*,size_t,char*,size_t);
extern int32_t rc_game_restore_handoff(uint64_t,const uint8_t*,size_t,char*,size_t);
extern intptr_t rc_game_map_handoff(uint64_t,uint8_t*,size_t);
extern intptr_t rc_game_audio(uint64_t,uint32_t,uint8_t*,size_t);
extern intptr_t rc_game_draw(uint64_t,size_t,size_t,uint32_t*,size_t);
extern int32_t rc_game_command(uint64_t,uint32_t);
extern intptr_t rc_game_actor_status(uint64_t,uint8_t*,size_t);
extern intptr_t rc_game_mission_status(uint64_t,uint8_t*,size_t);
extern void rc_game_release(uint64_t);
static uint8_t*read_file(const char*path,size_t*n){FILE*f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long size=ftell(f);assert(size>0);rewind(f);uint8_t*b=malloc((size_t)size);assert(b);assert(fread(b,1,(size_t)size,f)==(size_t)size);fclose(f);*n=(size_t)size;return b;}
int main(int argc,char**argv){
 assert(argc==9);size_t n1,n2,ni;uint8_t*one=read_file(argv[1],&n1),*two=read_file(argv[2],&n2),*input=read_file(argv[3],&ni);char error[512]={0};
 uint64_t h=rc_game_create(one,n1,error,sizeof(error));assert(h);uint32_t initial[10],status[10],completed[10],entry[10],squad[12],prior_squad[12];
 assert(rc_game_status(h,initial,10)==10);assert(rc_game_travel(h,two,n2,error,sizeof(error))==-1);assert(rc_game_status(h,status,10)==10 && !memcmp(status,initial,sizeof(status)));
 assert(ni>=12 && !memcmp(input,"RCINPUT1",8));uint32_t count;memcpy(&count,input+8,4);assert(ni==12+(size_t)count*20);
 for(uint32_t i=0;i<count;i++){float v[4];uint32_t flags;memcpy(v,input+12+i*20,16);memcpy(&flags,input+28+i*20,4);assert(rc_game_advance(h,1.0/60,v[0],v[1],v[2],v[3],flags)==1);}
 assert(rc_game_status(h,completed,10)==10 && completed[4]==1 && completed[0]==20);assert(rc_game_squad_status(h,prior_squad,12)==12);
 intptr_t hn=rc_game_map_handoff(h,NULL,0);assert(hn>0);uint8_t*handoff=malloc((size_t)hn);assert(handoff && rc_game_map_handoff(h,handoff,(size_t)hn)==hn);
 FILE*f=fopen("map1-handoff.json","wb");assert(f && fwrite(handoff,1,(size_t)hn,f)==(size_t)hn);fclose(f);
 assert(rc_game_travel(h,one,n1,error,sizeof(error))==-1);assert(rc_game_status(h,status,10)==10 && !memcmp(status,completed,sizeof(status)));
 assert(rc_game_travel(h,(const uint8_t*)"invalid",7,error,sizeof(error))==-1);assert(rc_game_status(h,status,10)==10 && !memcmp(status,completed,sizeof(status)));
 assert(rc_game_travel(h,two,n2,error,sizeof(error))==0);assert(rc_game_status(h,entry,10)==10 && entry[4]==0 && entry[5]==0 && !memcmp(entry,completed,3*sizeof(uint32_t)));
 assert(rc_game_squad_status(h,squad,12)==12 && !memcmp(squad,prior_squad,4*sizeof(uint32_t)));
 double pose[6];assert(rc_game_observe(h,pose,6)==6 && fabs(pose[0]+10640.3818359375)<0.0001 && fabs(pose[2]+808.0003662109375)<0.0001);
 uint64_t restored=rc_game_create(two,n2,error,sizeof(error));assert(restored && rc_game_restore_handoff(restored,handoff,(size_t)hn,error,sizeof(error))==0);assert(rc_game_status(restored,status,10)==10 && !memcmp(status,entry,sizeof(status)));rc_game_release(restored);
 size_t nb,ne;uint8_t*route=read_file(argv[4],&nb),*expected=read_file(argv[5],&ne);assert(ne==144);uint32_t expected_c[10],expected_failure[10];memcpy(expected_c,expected+64,40);memcpy(expected_failure,expected+104,40);
 assert(nb>=12 && !memcmp(route,"RCINPUT1",8));uint32_t route_count;memcpy(&route_count,route+8,4);assert(nb==12+(size_t)route_count*20);
 uint32_t sounds=0;
 for(uint32_t i=0;i<route_count;i++){
  float v[4];uint32_t flags;memcpy(v,route+12+i*20,16);memcpy(&flags,route+28+i*20,4);
  assert(rc_game_advance(h,1.0/60,v[0],v[1],v[2],v[3],flags)==1);
  intptr_t n=rc_game_audio(h,1,NULL,0);assert(n>=12);uint8_t*b=malloc((size_t)n);assert(b && rc_game_audio(h,1,b,(size_t)n)==n);uint32_t events;memcpy(&events,b+8,4);sounds+=events;free(b);
 }
 assert(rc_game_status(h,status,10)==10 && !memcmp(status,expected,40) && sounds>0);
 double final_pose[6],wanted_pose[3];memcpy(wanted_pose,expected+40,24);assert(rc_game_observe(h,final_pose,6)==6);
 // Measured cross-platform route drift is 0.298665 units after 16,414 B frames.
 // Half a unit is below 7% of one 7.5-unit movement step and 1.25% of body extent.
 // All ten integer status values and the original exit request remain exact.
 double max_delta=0;for(int i=0;i<3;i++){double d=fabs(final_pose[i]-wanted_pose[i]);if(d>max_delta)max_delta=d;fprintf(stderr,"pose[%d] %.17g expected %.17g delta %.17g\n",i,final_pose[i],wanted_pose[i],d);}assert(max_delta<0.5);
 intptr_t mn=rc_game_mission_status(h,NULL,0);assert(mn>0);uint8_t*mission=calloc((size_t)mn+1,1);assert(mission && rc_game_mission_status(h,mission,(size_t)mn)==mn);assert(strstr((char*)mission,"GEO_01C"));free(mission);
 uint32_t final_status[10];memcpy(final_status,status,sizeof(status));free(route);free(expected);
 intptr_t h2n=rc_game_map_handoff(h,NULL,0);assert(h2n>0);uint8_t*h2=malloc((size_t)h2n);assert(h2 && rc_game_map_handoff(h,h2,(size_t)h2n)==h2n);
 f=fopen("map2-handoff.json","wb");assert(f && fwrite(h2,1,(size_t)h2n,f)==(size_t)h2n);fclose(f);
 assert(rc_game_travel(h,two,n2,error,sizeof(error))==-1);assert(rc_game_status(h,status,10)==10 && !memcmp(status,final_status,sizeof(status)));
 assert(rc_game_travel(h,(const uint8_t*)"invalid",7,error,sizeof(error))==-1);assert(rc_game_status(h,status,10)==10 && !memcmp(status,final_status,sizeof(status)));
 size_t n3,nc;uint8_t*three=read_file(argv[6],&n3),*controls=read_file(argv[7],&nc);
 assert(rc_game_travel(h,three,n3,error,sizeof(error))==0);uint32_t entry3[10];assert(rc_game_status(h,entry3,10)==10);assert(entry3[0]==10 && entry3[1]==20 && entry3[2]==141 && entry3[3]==0 && entry3[4]==0 && entry3[5]==0);
 assert(rc_game_observe(h,pose,6)==6 && fabs(pose[0]+20039.7890625)<0.0001 && fabs(pose[1]-12575.505859375)<0.0001 && pose[2]==1144.);
 intptr_t an=rc_game_actor_status(h,NULL,0);assert(an>0);uint8_t*actors=calloc((size_t)an+1,1);assert(actors && rc_game_actor_status(h,actors,(size_t)an)==an && strstr((char*)actors,"Scorch") && strstr((char*)actors,"Fixer"));free(actors);
 restored=rc_game_create(three,n3,error,sizeof(error));assert(restored && rc_game_restore_handoff(restored,h2,(size_t)h2n,error,sizeof(error))==0);assert(rc_game_status(restored,status,10)==10 && !memcmp(status,entry3,sizeof(status)));assert(rc_game_restore_handoff(restored,h2,(size_t)h2n,error,sizeof(error))==-1);rc_game_release(restored);
 assert(nc>=12 && !memcmp(controls,"RCINPUT1",8));uint32_t ccount;memcpy(&ccount,controls+8,4);assert(ccount==expected_c[5] && nc==12+(size_t)ccount*20);uint32_t c_audio=0;
 for(uint32_t i=0;i<ccount;i++){float v[4];uint32_t flags;memcpy(v,controls+12+i*20,16);memcpy(&flags,controls+28+i*20,4);assert(rc_game_advance(h,1.0/60,v[0],v[1],v[2],v[3],flags)==1);intptr_t n=rc_game_audio(h,1,NULL,0);assert(n>=12);uint8_t*b=malloc((size_t)n);assert(b && rc_game_audio(h,1,b,(size_t)n)==n);uint32_t events;memcpy(&events,b+8,4);c_audio+=events;free(b);}
 assert(rc_game_status(h,status,10)==10 && !memcmp(status,expected_c,40) && c_audio>0);
 intptr_t cmn=rc_game_mission_status(h,NULL,0);assert(cmn>0);uint8_t*cm=calloc((size_t)cmn+1,1);assert(cm && rc_game_mission_status(h,cm,(size_t)cmn)==cmn && strstr((char*)cm,"Mission abgeschlossen"));free(cm);
 uint32_t*pixels=malloc(320*200*4);assert(pixels && rc_game_draw(h,320,200,pixels,320*200)==320*200);f=fopen("map3.argb","wb");assert(f && fwrite(pixels,4,320*200,f)==320*200);fclose(f);free(pixels);
 intptr_t h3n=rc_game_map_handoff(h,NULL,0);assert(h3n>0);uint8_t*h3=calloc((size_t)h3n+1,1);assert(h3 && rc_game_map_handoff(h,h3,(size_t)h3n)==h3n && strstr((char*)h3,"GEO_03A.ctm#playerstart"));
 f=fopen("map3-handoff.json","wb");assert(f && fwrite(h3,1,(size_t)h3n,f)==(size_t)h3n);fclose(f);free(h3);assert(rc_game_command(h,1)==0 && rc_game_status(h,status,10)==10 && !memcmp(status,entry3,sizeof(status)));
 size_t nf;uint8_t*failure=read_file(argv[8],&nf);assert(nf>=12 && !memcmp(failure,"RCINPUT1",8));uint32_t failure_count;memcpy(&failure_count,failure+8,4);assert(failure_count==expected_failure[5] && nf==12+(size_t)failure_count*20);
 for(uint32_t i=0;i<failure_count;i++){float v[4];uint32_t flags;memcpy(v,failure+12+i*20,16);memcpy(&flags,failure+28+i*20,4);assert(rc_game_advance(h,1.0/60,v[0],v[1],v[2],v[3],flags)==1);}
 assert(rc_game_status(h,status,10)==10 && !memcmp(status,expected_failure,40));assert(status[0]==0 && status[4]==2);assert(rc_game_command(h,1)==0 && rc_game_status(h,status,10)==10 && !memcmp(status,entry3,sizeof(status)));free(failure);
 printf("{\"passed\":true,\"real_map1_inputs\":%u,\"full_map2_inputs\":%u,\"map3_inputs\":%u,\"atomic_early_wrong_corrupt_travel\":true,\"saved_restore\":true,\"two_members_carried\":true,\"player_health\":%u,\"player_ammo\":%u,\"reserve\":%u,\"map2_maximum_position_delta\":%.17g,\"audio_events\":%u,\"map3_controls_and_reset\":true,\"ordinary_failure_without_sev\":true,\"failure_inputs\":%u}\n",count,route_count,ccount,entry3[0],entry3[1],entry3[2],max_delta,c_audio,failure_count);
 rc_game_release(h);free(handoff);free(one);free(two);free(input);free(three);free(controls);free(h2);return 0;
}
