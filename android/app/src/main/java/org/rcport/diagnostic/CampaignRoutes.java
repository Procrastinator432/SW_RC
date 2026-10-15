package org.rcport.diagnostic;

/** Campaign file choices; the native travel API remains the authority for acceptance. */
final class CampaignRoutes {
    private CampaignRoutes() {}
    static String display(String map) {
        switch(map){
            case "GEO_01A":return "Landezone";
            case "GEO_01B":return "Geonosianerturm";
            case "GEO_01C":return "Sun Fac";
            case "GEO_03A":return "Fabrikeinstieg";
            default:return "Letzter Checkpoint";
        }
    }
    static String map(String guidance) {
        for(String map:new String[]{"GEO_01A","GEO_01B","GEO_01C","GEO_03A"}) {
            if(guidance.startsWith(map))return map;
        }
        return "";
    }
    static String next(String guidance) {
        switch(map(guidance)) {
            case "GEO_01A":return "GEO_01B";
            case "GEO_01B":return "GEO_01C";
            case "GEO_01C":return "GEO_03A";
            default:return "";
        }
    }
    static String saved(String guidance) {
        switch(map(guidance)) {
            case "GEO_01B":return "geo_01a-handoff.json";
            case "GEO_01C":return "geo_01b-handoff.json";
            case "GEO_03A":return "geo_01c-handoff.json";
            default:return null;
        }
    }
}
