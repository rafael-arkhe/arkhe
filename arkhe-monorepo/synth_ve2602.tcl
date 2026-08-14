# ============================================================
# synth_ve2602.tcl
# Sintese do Detector de CTC (BLOCK 11 v2.6) para VE2602 (Versal AI Edge)
# Uso:  vivado -mode batch -source synth_ve2602.tcl
# Ajuste a PARTE para o dispositivo exato instalado (ver XDC).
# ============================================================

# --- Dispositivo VE2602 (Versal AI Edge) ---
# Formato AMD: xqve2602-<speed>-<package>-<temp>
set part "xqve2602-2csgi1028-a"

set proj "ctc_detector_ve2602"
set top  "ctc_detector_enhanced_v2_6"

# --- Projeto ---
create_project $proj . -part $part -force
set_property target_language VHDL [current_project]

# --- Fontes ---
add_files -norecurse [list \
    sigmoid_lut_pkg.vhd \
    qdiv_pipe.vhd \
    ctc_detector_enhanced_v2_6.vhd \
]

# --- Restricoes ---
read_xdc ve2602_ctc.xdc

# --- Sintese ---
set_property top $top [current_fileset]
set_property strategy Performance_Explore [get_runs synth_1]

synth_design -top $top -part $part

# --- Relatorios ---
report_utilization         -file util_ctc.rpt
report_utilization -hierarchical -file util_ctc_hier.rpt
report_timing_summary      -file timing_ctc.rpt
report_clock_interaction   -file clock_ctc.rpt
write_checkpoint -force ctc_post_synth.dcp

puts "== SYNTHESE VE2602 CONCLUIDA: util_ctc.rpt / timing_ctc.rpt =="
