package org.rcport.diagnostic;
public final class CheckpointPolicyCheck {
    static int[] state(int tick){return new int[]{100,20,180,0,0,tick,0,0,0,0};}
    static void require(boolean value){if(!value)throw new AssertionError();}
    public static void main(String[] args){
        CheckpointPolicy p=new CheckpointPolicy();require(!p.observe(state(0),true,1));
        require(!p.observe(state(299),true,1));require(p.observe(state(300),true,1));p.saved(300,1);
        require(!p.observe(state(600),true,1));require(p.observe(state(601),true,2));p.saved(601,2);
        int[] attacked=state(700);attacked[9]=1;require(!p.observe(attacked,true,3));
        attacked[5]=999;require(!p.observe(attacked,true,3));attacked[5]=1000;require(p.observe(attacked,true,3));
        require(!p.observe(state(1001),false,3));require(!p.observe(state(1100),true,3));
        require(!p.observe(state(0),true,3));require(p.observe(state(300),true,3));
        p.reset();require(!p.observe(state(9000),true,9));require(p.observe(state(9300),true,9));
        int[] damaged=state(9301);damaged[0]=90;require(!p.observe(damaged,true,10));
        damaged[5]=9601;require(p.observe(damaged,true,10));
        damaged[6]=1;require(!p.observe(damaged,true,11));
        System.out.println("{\"passed\":true,\"quiet_simulation_time\":true,\"combat_and_damage_reset\":true,\"restore_resets_eligibility\":true}");
    }
}
