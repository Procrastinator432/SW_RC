package org.rcport.diagnostic;
public final class CampaignRoutesCheck {
    public static void main(String[] args) {
        String[] maps={"GEO_01A","GEO_01B","GEO_01C","GEO_03A"};
        for(int i=0;i<3;i++) {
            if(!CampaignRoutes.next(maps[i]+" abgeschlossen").equals(maps[i+1]))throw new AssertionError();
            if(!CampaignRoutes.saved(maps[i+1]+" entry").equals(maps[i].toLowerCase(java.util.Locale.ROOT)+"-handoff.json"))throw new AssertionError();
        }
        if(!CampaignRoutes.next("GEO_03A entry").isEmpty() || CampaignRoutes.saved("GEO_01A entry")!=null || CampaignRoutes.saved("unknown")!=null)throw new AssertionError();
        System.out.println("{\"passed\":true,\"all_three_links\":true,\"separate_save_files\":true,\"factory_has_no_implicit_next_mission\":true}");
    }
}
