# ============================================================
# ve2602_ctc.xdc
# Restricoes de timing para o Detector de CTC (BLOCK 11 v2.6)
# Plataforma: AMD/Xilinx Versal AI Edge VE2602 - 100 MHz
# ============================================================

# --- Relogio de sistema: 100 MHz (10.000 ns) ---
create_clock -period 10.000 -name clk [get_ports clk]

# --- Reset assincrono, ativo em nivel baixo (sem skew) ---
set_property -quiet ASYNC_REG true [get_cells -hier -filter {NAME =~ *rst_n*}]

# --- Entradas de metricas espectrais (dominio combinacional) ---
# Ajuste os delays conforme o caminho real do Spectral Metrics block.
set_input_delay -clock clk -max 3.000 [get_ports {purity_p ricci_tilde kolmogorov_cd meso mode_n}]
set_input_delay -clock clk -min 0.500 [get_ports {purity_p ricci_tilde kolmogorov_cd meso mode_n}]
set_input_delay -clock clk -max 2.000 [get_ports valid_in]

# --- Saidas ---
set_output_delay -clock clk -max 3.000 [get_ports {retro_capacity retro_eff pctc_prob eff_eff torsion horizon valid_out}]
set_output_delay -clock clk -min 0.500 [get_ports {retro_capacity retro_eff pctc_prob eff_eff torsion horizon valid_out}]
set_output_delay -clock clk -max 3.000 [get_ports {ctc_flag retro_active horizon_alarm}]
set_output_delay -clock clk -min 0.500 [get_ports {ctc_flag retro_active horizon_alarm}]

# --- I/O: substitua pelos pins reais da placa (RFSoC AXI / Jtag) ---
# set_property PACKAGE_PIN <pin> [get_ports clk]
# set_property PACKAGE_PIN <pin> [get_ports rst_n]

# --- Margem de relogio de 2% (guard band) ---
set_clock_uncertainty 0.200 [get_clocks clk]
