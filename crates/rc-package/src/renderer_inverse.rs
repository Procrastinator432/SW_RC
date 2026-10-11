//! Scalar D3D inverse 10031709 under nearest x87 53-bit precision.
//! Generated from the reviewed original stack/dataflow; f32 spills remain explicit.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub matrix: [u32; 16],
    pub store_determinant: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Output {
    pub determinant: Option<u32>,
    pub reciprocal: u32,
    pub matrix: Option<[u32; 16]>,
}
fn load(word: u32) -> Result<f64, String> {
    let value = f32::from_bits(word);
    if value.is_finite() {
        Ok(f64::from(value))
    } else {
        Err("Nonfinite scalar inverse input/spill".into())
    }
}
/// This models the scalar fallback with precision-control=53, rounding=nearest.
/// It does not select a native CPU variant. Finite f32 inputs/spills keep every
/// extended intermediate within binary64's exponent range; arithmetic therefore
/// has the same significand rounding. The rounded reciprocal alone gates writes.
pub fn scalar53(input: [u32; 16], store_determinant: bool) -> Result<Output, String> {
    for word in input {
        load(word)?;
    }
    let v0 = load(input[0])?; // 10031715
    let v1 = load(input[1])?; // 10031717
    let v2 = load(input[5])?; // 10031723
    let v3 = v2 * v0; // 10031729
    let v4 = load(input[4])?; // 1003172e
    let v5 = v4 * v1; // 10031734
    let v6 = v3 - v5; // 1003173f
    let w7 = (v6 as f32).to_bits(); // 1003174a
    let v8 = load(input[9])?; // 10031750
    let v9 = v8 * v0; // 10031756
    let v10 = load(input[8])?; // 10031758
    let v11 = v10 * v1; // 1003175b
    let v12 = v9 - v11; // 1003175d
    let w13 = (v12 as f32).to_bits(); // 1003175f
    let v14 = load(input[13])?; // 10031762
    let v15 = v14 * v0; // 10031765
    let v16 = load(input[12])?; // 10031767
    let v17 = v16 * v1; // 1003176a
    let v18 = v15 - v17; // 1003176c
    let w19 = (v18 as f32).to_bits(); // 1003176e
    let v20 = load(input[9])?; // 10031775
    let v21 = load(input[4])?; // 10031778
    let v22 = v20 * v21; // 10031778
    let v23 = load(input[8])?; // 1003177b
    let v24 = load(input[5])?; // 1003177e
    let v25 = v23 * v24; // 1003177e
    let v26 = v22 - v25; // 10031781
    let w27 = (v26 as f32).to_bits(); // 10031783
    let v28 = load(input[13])?; // 10031786
    let v29 = load(input[4])?; // 10031789
    let v30 = v28 * v29; // 10031789
    let v31 = load(input[12])?; // 1003178c
    let v32 = load(input[5])?; // 1003178f
    let v33 = v31 * v32; // 1003178f
    let v34 = v30 - v33; // 10031792
    let w35 = (v34 as f32).to_bits(); // 10031794
    let v36 = load(input[13])?; // 10031797
    let v37 = load(input[8])?; // 1003179d
    let v38 = v36 * v37; // 1003179d
    let v39 = load(input[12])?; // 100317a0
    let v40 = load(input[9])?; // 100317a3
    let v41 = v39 * v40; // 100317a3
    let v42 = v38 - v41; // 100317a6
    let w43 = (v42 as f32).to_bits(); // 100317a8
    let v44 = load(input[2])?; // 100317ab
    let v45 = load(input[3])?; // 100317ae
    let v46 = load(input[6])?; // 100317b1
    let w47 = (v46 as f32).to_bits(); // 100317b4
    let v48 = load(input[7])?; // 100317b7
    let w49 = (v48 as f32).to_bits(); // 100317ba
    let v50 = load(input[10])?; // 100317bd
    let w51 = (v50 as f32).to_bits(); // 100317c0
    let v52 = load(input[11])?; // 100317c3
    let w53 = (v52 as f32).to_bits(); // 100317c6
    let v54 = load(w27)?; // 100317cb
    let v55 = v44 * v54; // 100317cb
    let v56 = load(w13)?; // 100317d0
    let v57 = v46 * v56; // 100317d0
    let v58 = v55 - v57; // 100317d3
    let v59 = load(w7)?; // 100317d7
    let v60 = v50 * v59; // 100317d7
    let v61 = v58 + v60; // 100317da
    let w62 = (v61 as f32).to_bits(); // 100317dc
    let v63 = load(w19)?; // 100317e1
    let v64 = v46 * v63; // 100317e1
    let v65 = load(input[14])?; // 100317e4
    let v66 = load(w7)?; // 100317e7
    let v67 = v65 * v66; // 100317e7
    let v68 = v64 - v67; // 100317ea
    let v69 = load(w35)?; // 100317ee
    let v70 = v44 * v69; // 100317ee
    let v71 = v68 - v70; // 100317f1
    let w72 = (v71 as f32).to_bits(); // 100317f3
    let v73 = load(w43)?; // 100317f8
    let v74 = v44 * v73; // 100317f8
    let v75 = load(w19)?; // 100317fd
    let v76 = v50 * v75; // 100317fd
    let v77 = v74 - v76; // 10031800
    let v78 = load(input[14])?; // 10031802
    let v79 = load(w13)?; // 10031805
    let v80 = v78 * v79; // 10031805
    let v81 = v77 + v80; // 10031808
    let w82 = (v81 as f32).to_bits(); // 1003180a
    let v83 = load(w35)?; // 1003180f
    let v84 = v50 * v83; // 1003180f
    let v85 = load(input[14])?; // 10031812
    let v86 = load(w27)?; // 10031815
    let v87 = v85 * v86; // 10031815
    let v88 = v84 - v87; // 10031818
    let v89 = load(w43)?; // 1003181c
    let v90 = v46 * v89; // 1003181c
    let v91 = v88 - v90; // 1003181f
    let w92 = (v91 as f32).to_bits(); // 10031821
    let v93 = load(w13)?; // 10031824
    let v94 = v93 * v48; // 10031827
    let v95 = load(w7)?; // 10031829
    let v96 = v95 * v52; // 1003182c
    let v97 = v94 - v96; // 1003182e
    let v98 = load(w27)?; // 10031830
    let v99 = v98 * v45; // 10031833
    let v100 = v97 - v99; // 10031835
    let w101 = (v100 as f32).to_bits(); // 10031837
    let v102 = load(w35)?; // 1003183a
    let v103 = v102 * v45; // 1003183d
    let v104 = load(w19)?; // 1003183f
    let v105 = v104 * v48; // 10031842
    let v106 = v103 - v105; // 10031844
    let v107 = load(w7)?; // 10031846
    let v108 = load(input[15])?; // 10031849
    let v109 = v107 * v108; // 10031849
    let v110 = v106 + v109; // 1003184c
    let w111 = (v110 as f32).to_bits(); // 1003184e
    let v112 = load(w19)?; // 10031851
    let v113 = v112 * v52; // 10031854
    let v114 = load(w13)?; // 10031856
    let v115 = load(input[15])?; // 10031859
    let v116 = v114 * v115; // 10031859
    let v117 = v113 - v116; // 1003185c
    let v118 = load(w43)?; // 1003185e
    let v119 = v118 * v45; // 10031861
    let v120 = v117 - v119; // 10031863
    let w121 = (v120 as f32).to_bits(); // 10031865
    let v122 = load(w43)?; // 10031868
    let v123 = v122 * v48; // 1003186b
    let v124 = load(w35)?; // 1003186d
    let v125 = v124 * v52; // 10031870
    let v126 = v123 - v125; // 10031872
    let v127 = load(w27)?; // 10031874
    let v128 = load(input[15])?; // 10031877
    let v129 = v127 * v128; // 10031877
    let v130 = v126 + v129; // 1003187a
    let w131 = (v130 as f32).to_bits(); // 1003187c
    let v132 = v44 * v48; // 10031885
    let v133 = v46 * v45; // 10031889
    let v134 = v132 - v133; // 1003188b
    let w135 = (v134 as f32).to_bits(); // 1003188d
    let v136 = load(w53)?; // 10031896
    let v137 = v44 * v136; // 10031896
    let v138 = load(w51)?; // 10031899
    let v139 = v138 * v45; // 1003189c
    let v140 = v137 - v139; // 1003189e
    let w141 = (v140 as f32).to_bits(); // 100318a0
    let v142 = load(input[15])?; // 100318a5
    let v143 = v44 * v142; // 100318a5
    let v144 = load(input[14])?; // 100318a8
    let v145 = v144 * v45; // 100318ab
    let v146 = v143 - v145; // 100318ad
    let v147 = load(w47)?; // 100318b3
    let v148 = load(w53)?; // 100318b8
    let v149 = v147 * v148; // 100318b8
    let v150 = load(w51)?; // 100318be
    let v151 = load(w49)?; // 100318c4
    let v152 = v150 * v151; // 100318c4
    let v153 = v149 - v152; // 100318d0
    let v154 = load(w47)?; // 100318d5
    let v155 = load(input[15])?; // 100318d8
    let v156 = v154 * v155; // 100318d8
    let v157 = load(input[14])?; // 100318db
    let v158 = load(w49)?; // 100318de
    let v159 = v157 * v158; // 100318de
    let v160 = v156 - v159; // 100318e1
    let v161 = load(w51)?; // 100318e3
    let v162 = load(input[15])?; // 100318e6
    let v163 = v161 * v162; // 100318e6
    let v164 = load(input[14])?; // 100318e9
    let v165 = load(w53)?; // 100318ec
    let v166 = v164 * v165; // 100318ec
    let v167 = v163 - v166; // 100318f8
    let v168 = load(input[1])?; // 100318fa
    let v169 = load(input[5])?; // 100318fd
    let v170 = load(w141)?; // 10031903
    let v171 = v170 * v169; // 10031909
    let v172 = load(w135)?; // 1003190b
    let v173 = load(input[9])?; // 1003190e
    let v174 = v172 * v173; // 1003190e
    let v175 = v171 - v174; // 10031911
    let v176 = v153 * v168; // 10031915
    let v177 = v175 - v176; // 10031917
    let w178 = (v177 as f32).to_bits(); // 10031919
    let v179 = v160 * v168; // 1003191e
    let v180 = v146 * v169; // 10031922
    let v181 = v179 - v180; // 10031924
    let v182 = load(w135)?; // 10031926
    let v183 = load(input[13])?; // 10031929
    let v184 = v182 * v183; // 10031929
    let v185 = v181 + v184; // 1003192c
    let w186 = (v185 as f32).to_bits(); // 1003192e
    let v187 = load(input[9])?; // 10031933
    let v188 = v146 * v187; // 10031933
    let v189 = load(w141)?; // 10031936
    let v190 = load(input[13])?; // 10031939
    let v191 = v189 * v190; // 10031939
    let v192 = v188 - v191; // 1003193c
    let v193 = v167 * v168; // 10031940
    let v194 = v192 - v193; // 10031942
    let w195 = (v194 as f32).to_bits(); // 10031944
    let v196 = v167 * v169; // 10031949
    let v197 = load(input[9])?; // 1003194d
    let v198 = v160 * v197; // 1003194d
    let v199 = v196 - v198; // 10031950
    let v200 = load(input[13])?; // 10031954
    let v201 = v153 * v200; // 10031954
    let v202 = v199 + v201; // 10031957
    let w203 = (v202 as f32).to_bits(); // 10031959
    let v204 = load(input[0])?; // 10031962
    let v205 = v153 * v204; // 10031962
    let v206 = load(w141)?; // 10031965
    let v207 = load(input[4])?; // 10031968
    let v208 = v206 * v207; // 10031968
    let v209 = v205 - v208; // 1003196b
    let v210 = load(w135)?; // 1003196d
    let v211 = load(input[8])?; // 10031970
    let v212 = v210 * v211; // 10031970
    let v213 = v209 + v212; // 10031973
    let w214 = (v213 as f32).to_bits(); // 10031975
    let v215 = load(input[4])?; // 1003197a
    let v216 = v146 * v215; // 1003197a
    let v217 = load(w135)?; // 1003197d
    let v218 = load(input[12])?; // 10031980
    let v219 = v217 * v218; // 10031980
    let v220 = v216 - v219; // 10031983
    let v221 = load(input[0])?; // 1003198c
    let v222 = v160 * v221; // 1003198c
    let v223 = v220 - v222; // 1003198f
    let w224 = (v223 as f32).to_bits(); // 10031991
    let v225 = load(input[0])?; // 10031994
    let v226 = v225 * v167; // 10031997
    let v227 = load(input[8])?; // 1003199b
    let v228 = v146 * v227; // 1003199b
    let v229 = v226 - v228; // 1003199e
    let v230 = load(w141)?; // 100319a0
    let v231 = load(input[12])?; // 100319a3
    let v232 = v230 * v231; // 100319a3
    let v233 = v229 + v232; // 100319a6
    let w234 = (v233 as f32).to_bits(); // 100319a8
    let v235 = load(input[8])?; // 100319ad
    let v236 = v160 * v235; // 100319ad
    let v237 = load(input[12])?; // 100319b2
    let v238 = v153 * v237; // 100319b2
    let v239 = v236 - v238; // 100319b5
    let v240 = load(input[4])?; // 100319b9
    let v241 = v167 * v240; // 100319b9
    let v242 = v239 - v241; // 100319bc
    let w243 = (v242 as f32).to_bits(); // 100319be
    let v244 = load(w178)?; // 100319c9
    let v245 = load(input[12])?; // 100319cc
    let v246 = v244 * v245; // 100319cc
    let v247 = load(w186)?; // 100319cf
    let v248 = load(input[8])?; // 100319d2
    let v249 = v247 * v248; // 100319d2
    let v250 = v246 + v249; // 100319d5
    let v251 = load(w195)?; // 100319d7
    let v252 = load(input[4])?; // 100319da
    let v253 = v251 * v252; // 100319da
    let v254 = v250 + v253; // 100319dd
    let v255 = load(w203)?; // 100319df
    let v256 = load(input[0])?; // 100319e2
    let v257 = v255 * v256; // 100319e2
    let v258 = v254 + v257; // 100319e5
    let w259 = (v258 as f32).to_bits(); // 100319e9
    let determinant = store_determinant.then_some(w259);
    let v260 = 1.0f64; // 100319eb
    let v261 = v260 / v258; // 100319ee
    let w262 = (v261 as f32).to_bits(); // 100319f1
    let reciprocal = w262;
    if !f32::from_bits(reciprocal).is_finite() {
        return Ok(Output {
            determinant,
            reciprocal,
            matrix: None,
        });
    }
    let v263 = load(w262)?; // 10031a0e
    let v264 = load(w203)?; // 10031a11
    let v265 = v264 * v263; // 10031a14
    let w266 = (v265 as f32).to_bits(); // 10031a16
    let v267 = load(w195)?; // 10031a18
    let v268 = v267 * v263; // 10031a1b
    let w269 = (v268 as f32).to_bits(); // 10031a1d
    let v270 = load(w186)?; // 10031a20
    let v271 = v270 * v263; // 10031a23
    let w272 = (v271 as f32).to_bits(); // 10031a25
    let v273 = load(w178)?; // 10031a28
    let v274 = v273 * v263; // 10031a2b
    let w275 = (v274 as f32).to_bits(); // 10031a2d
    let v276 = load(w243)?; // 10031a30
    let v277 = v276 * v263; // 10031a33
    let w278 = (v277 as f32).to_bits(); // 10031a35
    let v279 = load(w234)?; // 10031a38
    let v280 = v279 * v263; // 10031a3b
    let w281 = (v280 as f32).to_bits(); // 10031a3d
    let v282 = load(w224)?; // 10031a40
    let v283 = v282 * v263; // 10031a43
    let w284 = (v283 as f32).to_bits(); // 10031a45
    let v285 = load(w214)?; // 10031a48
    let v286 = v285 * v263; // 10031a4b
    let w287 = (v286 as f32).to_bits(); // 10031a4d
    let v288 = load(w131)?; // 10031a50
    let v289 = v288 * v263; // 10031a53
    let w290 = (v289 as f32).to_bits(); // 10031a55
    let v291 = load(w121)?; // 10031a58
    let v292 = v291 * v263; // 10031a5b
    let w293 = (v292 as f32).to_bits(); // 10031a5d
    let v294 = load(w111)?; // 10031a60
    let v295 = v294 * v263; // 10031a63
    let w296 = (v295 as f32).to_bits(); // 10031a65
    let v297 = load(w101)?; // 10031a68
    let v298 = v297 * v263; // 10031a6b
    let w299 = (v298 as f32).to_bits(); // 10031a6d
    let v300 = load(w92)?; // 10031a70
    let v301 = v300 * v263; // 10031a73
    let w302 = (v301 as f32).to_bits(); // 10031a75
    let v303 = load(w82)?; // 10031a78
    let v304 = v303 * v263; // 10031a7b
    let w305 = (v304 as f32).to_bits(); // 10031a7d
    let v306 = load(w72)?; // 10031a80
    let v307 = v306 * v263; // 10031a83
    let w308 = (v307 as f32).to_bits(); // 10031a85
    let v309 = load(w62)?; // 10031a88
    let v310 = v309 * v263; // 10031a8b
    let w311 = (v310 as f32).to_bits(); // 10031a8d
    Ok(Output {
        determinant,
        reciprocal,
        matrix: Some([
            w266, w269, w272, w275, w278, w281, w284, w287, w290, w293, w296, w299, w302, w305,
            w308, w311,
        ]),
    })
}
