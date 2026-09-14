/*
 * Orbe — Estrutura de Bobinas de Helmholtz (Substrato 156 — Dipolo de Arkhe)
 * Modelo 3D parametrizado em OpenSCAD
 * Modelo: ORB-01 · Versão: v1.0 · Data: 2026-08-30
 *
 * Baseado na análise estática do G-code real (Orbe_Helmholtz.gcode)
 * e na correção de escala 50/56 centrada nos eixos das bobinas
 * (Orbe_Helmholtz_100mm_corrigido.gcode).
 *
 * Geometria:
 *   - Dois anéis coaxiais (annulus) de R_ext=50, R_int=39.29, altura 5 mm
 *   - Anel Z+ (superior): centro (X=60, Y=60)
 *   - Anel Z- (inferior): centro (X=60, Y=160)
 *   - 4 abas de ponte radiais (do bordo interno ao externo) no anel Z+
 *
 * Uso no OpenSCAD:
 *   1. Abra este arquivo.
 *   2. Ajuste os parâmetros na seção "PARÂMETROS".
 *   3. Design > Preview (F5) e Render (F6).
 *   4. Arquivo > Exportar > Export as STL.
 */
$fn = 120; // resolução angular (aumente para suavizar)

/* ================== PARÂMETROS ================== */
// --- Dimensões da estrutura ---
r_outer     = 50.0;    // raio externo do anel (mm)  [56 -> 50 corrigido]
r_inner     = 39.29;   // raio interno do anel (mm)  [44 -> ~39.29]
ring_h      = 5.0;     // altura de cada anel (mm)   [25 camadas x 0.2]
layer_h     = 0.2;     // altura de camada (referência, mm)

// --- Posições (sistema de coordenadas da mesa do G-code) ---
center_x    = 60.0;    // eixo comum das bobinas (X)
ring_zp_y   = 60.0;    // centro do anel Z+ (superior)
ring_zm_y   = 160.0;   // centro do anel Z- (inferior)
ring_z_base = 0.0;     // base dos anéis em Z (mm)

// --- Abas de ponte (suportes no anel Z+, fundidos ao topo do anel) ---
show_bridges  = true;        // mostrar as 4 abas de ponte
bridge_thick  = 1.6;         // espessura (em Z) de cada aba (mm)
bridge_half_w = 8.0;         // meia-largura tangencial de cada aba (mm)
bridge_z      = ring_h + bridge_thick / 2 - 0.6; // centro Z (overlap de 0.6 mm p/ fundir ao anel)
// O comprimento radial de cada aba vai de r_inner até r_outer (como no G-code).

// --- Modo de exibição ---
mode = "print";        // "print"  = anéis planos na mesa (como impressos)
                       // "mounted"= bobinas em pé (coils), visual conceitual

// --- Seleção de parte exportada ---
// "zp"   = somente anel Z+ (com pontes)
// "zm"   = somente anel Z-
// "both" = os dois anéis (juntos no mesmo arquivo; na impressão real os
//          dois ficam no mesmo plano e se sobrepõem em Y — consulte README)
part = "both";

/* ================== FIM DOS PARÂMETROS ================== */

/* Anel (annulus) posicionado em (x, y) */
module ring(x, y) {
    translate([x, y, ring_z_base])
        linear_extrude(height = ring_h)
            difference() {
                circle(r = r_outer);
                circle(r = r_inner);
            }
}

/* 4 abas de ponte radiais nos pontos cardeais do anel Z+ */
module bridges_zp() {
    if (show_bridges) {
        half = bridge_half_w;
        // comprimento radial do tab (do bordo interno ao externo)
        rad_len = r_outer - r_inner;
        // centro radial do tab
        rad_center = (r_inner + r_outer) / 2;

        // +X (direita)
        translate([center_x + rad_center, ring_zp_y, bridge_z])
            cube([rad_len, half * 2, bridge_thick], center = true);
        // -X (esquerda)
        translate([center_x - rad_center, ring_zp_y, bridge_z])
            cube([rad_len, half * 2, bridge_thick], center = true);
        // +Y (frente)
        translate([center_x, ring_zp_y + rad_center, bridge_z])
            cube([half * 2, rad_len, bridge_thick], center = true);
        // -Y (trás)
        translate([center_x, ring_zp_y - rad_center, bridge_z])
            cube([half * 2, rad_len, bridge_thick], center = true);
    }
}

/* ================== GERAÇÃO ================== */
module orbe_print() {
    // anel Z+ (com pontes) e anel Z-
    if (part == "zp" || part == "both") { bridges_zp(); ring(center_x, ring_zp_y); }
    if (part == "zm" || part == "both") { ring(center_x, ring_zm_y); }
}

/* Visualização conceitual "mounted": duas bobinas em pé */
module orbe_mounted() {
    rotate([0, 45, 0])
        orbe_print();
}

if (mode == "print") {
    orbe_print();
} else if (mode == "mounted") {
    orbe_mounted();
} else {
    orbe_print();
}
