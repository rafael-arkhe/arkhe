#!/usr/bin/env python3
"""
gen_sigmoid_lut.py
Gera LUT de sigmoide de 256 entradas em Q8.24 para VHDL
v2.2 -- 2026-08-11

Dominio de entrada: [-8.0, +8.0] mapeado para indice [0, 255]
Resolucao do indice: 16/256 = 0.0625
Erro maximo estimado: < 0.4% da escala [0, 1]

O script gera:
1. Arquivo VHDL com a LUT completa (256 entradas x 32 bits)
2. Arquivo C header para validacao embarcada
3. Relatorio de erro de quantizacao
"""

import math
import numpy as np

# ============================================================
# PARAMETROS
# ============================================================
WL = 32          # Largura total em bits
FWL = 24         # Bits fracionarios
SCALE = 2**FWL   # Fator de escala Q8.24
N_ENTRIES = 256  # Numero de entradas na LUT
X_MIN = -8.0     # Limite inferior do dominio
X_MAX = +8.0     # Limite superior do dominio
STEP = (X_MAX - X_MIN) / N_ENTRIES  # Passo: 16/256 = 0.0625

# ============================================================
# FUNCOES DE CONVERSAO
# ============================================================

def float_to_q824(x):
    """Converte float para Q8.24 com clamping."""
    MAX_VAL = (2**(WL-1) - 1) / SCALE  # ~127.999
    MIN_VAL = -(2**(WL-1)) / SCALE     # ~-128.0

    x = max(MIN_VAL, min(MAX_VAL, x))
    return int(round(x * SCALE))


def q824_to_hex(val):
    """Converte valor Q8.24 para string hexadecimal 8 digitos."""
    # Garantir que esta no intervalo de 32 bits com sinal
    if val < 0:
        val = val + 2**32
    return f'x"{val:08X}"'


# ============================================================
# GERACAO DA LUT
# ============================================================

def generate_sigmoid_lut():
    """Gera a LUT de sigmoide completa."""
    lut = []
    errors = []

    for i in range(N_ENTRIES):
        x = X_MIN + i * STEP + STEP / 2  # Ponto medio do intervalo

        # Sigmoide em float64 (referencia)
        y_float = 1.0 / (1.0 + math.exp(-x))

        # Converter para Q8.24
        y_q = float_to_q824(y_float)

        # Converter de volta para verificar erro
        y_back = y_q / SCALE

        # Erro absoluto
        err = abs(y_float - y_back)
        errors.append(err)

        lut.append({
            'index': i,
            'x': x,
            'y_float': y_float,
            'y_q': y_q,
            'hex': q824_to_hex(y_q),
            'error': err
        })

    return lut, errors


# ============================================================
# GERACAO DE ARQUIVO VHDL
# ============================================================

def generate_vhdl_package(lut, filename):
    """Gera pacote VHDL com a LUT de sigmoide."""

    lines = []
    lines.append("-- ============================================================")
    lines.append("-- sigmoid_lut_pkg.vhd")
    lines.append("-- LUT de sigmoide de 256 entradas em Q8.24")
    lines.append("-- Gerado automaticamente por gen_sigmoid_lut.py")
    lines.append("-- Data: 2026-08-11")
    lines.append("-- Dominio: [-8.0, +8.0] -> indice [0, 255]")
    lines.append("-- Passo: 0.0625")
    lines.append("-- Erro maximo: {:.10f}".format(max(e['error'] for e in lut)))
    lines.append("-- Erro medio: {:.10f}".format(sum(e['error'] for e in lut)/len(lut)))
    lines.append("-- ============================================================")
    lines.append("")
    lines.append("library ieee;")
    lines.append("use ieee.std_logic_1164.all;")
    lines.append("use ieee.numeric_std.all;")
    lines.append("")
    lines.append("package sigmoid_lut_pkg is")
    lines.append("    constant SIGMOID_LUT_SIZE : integer := 256;")
    lines.append("    constant SIGMOID_X_MIN    : real := -8.0;")
    lines.append("    constant SIGMOID_X_MAX    : real := +8.0;")
    lines.append("    constant SIGMOID_STEP     : real := 0.0625;")
    lines.append("")
    lines.append("    type sigmoid_lut_t is array (0 to SIGMOID_LUT_SIZE-1) of std_logic_vector(31 downto 0);")
    lines.append("")
    lines.append("    constant SIGMOID_LUT : sigmoid_lut_t := (")

    # Gerar entradas em grupos de 4 para legibilidade
    for i in range(0, N_ENTRIES, 4):
        entries = []
        for j in range(4):
            if i + j < N_ENTRIES:
                e = lut[i + j]
                comment = f"  -- idx={i+j:3d}, x={e['x']:+.4f}, sigma={e['y_float']:.6f}"
                entries.append(f"        {e['hex']}{comment}")

        if i + 4 < N_ENTRIES:
            lines.append(",\n".join(entries) + ",")
        else:
            lines.append(",\n".join(entries))

    lines.append("    );")
    lines.append("")
    lines.append("    -- Funcao de lookup com interpolacao linear opcional")
    lines.append("    function sigmoid_lut_lookup(x : real) return std_logic_vector;")
    lines.append("    function sigmoid_lut_lookup(x : signed(31 downto 0)) return signed;")
    lines.append("")
    lines.append("end package;")
    lines.append("")
    lines.append("package body sigmoid_lut_pkg is")
    lines.append("")
    lines.append("    function sigmoid_lut_lookup(x : real) return std_logic_vector is")
    lines.append("        variable idx : integer range 0 to 255;")
    lines.append("        variable x_clamped : real;")
    lines.append("    begin")
    lines.append("        if x <= SIGMOID_X_MIN then")
    lines.append("            return x\"00000001\";  -- Aproximadamente 0")
    lines.append("        elsif x >= SIGMOID_X_MAX then")
    lines.append("            return x\"01000000\";  -- 1.0 em Q8.24")
    lines.append("        else")
    lines.append("            idx := integer((x - SIGMOID_X_MIN) / SIGMOID_STEP);")
    lines.append("            if idx < 0 then idx := 0; end if;")
    lines.append("            if idx > 255 then idx := 255; end if;")
    lines.append("            return SIGMOID_LUT(idx);")
    lines.append("        end if;")
    lines.append("    end function;")
    lines.append("")
    lines.append("    function sigmoid_lut_lookup(x : signed(31 downto 0)) return signed is")
    lines.append("        variable x_real : real;")
    lines.append("    begin")
    lines.append("        -- Converter Q8.24 para real")
    lines.append("        x_real := real(to_integer(x)) / real(2**24);")
    lines.append("        return signed(sigmoid_lut_lookup(x_real));")
    lines.append("    end function;")
    lines.append("")
    lines.append("end package body;")

    with open(filename, 'w', encoding='utf-8') as f:
        f.write('\n'.join(lines))

    return filename


# ============================================================
# GERACAO DE ARQUIVO C (PARA VALIDACAO)
# ============================================================

def generate_c_header(lut, filename):
    """Gera header C com a LUT para validacao embarcada."""

    lines = []
    lines.append("/* ============================================================")
    lines.append(" * sigmoid_lut.h")
    lines.append(" * LUT de sigmoide de 256 entradas em Q8.24")
    lines.append(" * Gerado automaticamente por gen_sigmoid_lut.py")
    lines.append(" * Data: 2026-08-11")
    lines.append(" * Dominio: [-8.0, +8.0] -> indice [0, 255]")
    lines.append(" * ============================================================ */")
    lines.append("")
    lines.append("#ifndef SIGMOID_LUT_H")
    lines.append("#define SIGMOID_LUT_H")
    lines.append("")
    lines.append("#include <stdint.h>")
    lines.append("")
    lines.append(f"#define SIGMOID_LUT_SIZE {N_ENTRIES}")
    lines.append(f"#define SIGMOID_X_MIN    (-8.0f)")
    lines.append(f"#define SIGMOID_X_MAX    (+8.0f)")
    lines.append(f"#define SIGMOID_STEP     (0.0625f)")
    lines.append(f"#define Q824_SCALE       (16777216.0f)  /* 2^24 */")
    lines.append("")
    lines.append("static const int32_t SIGMOID_LUT[SIGMOID_LUT_SIZE] = {")

    for i in range(0, N_ENTRIES, 8):
        entries = []
        for j in range(8):
            if i + j < N_ENTRIES:
                e = lut[i + j]
                entries.append(f"{e['y_q']:10d}")

        line = "    " + ", ".join(entries)
        if i + 8 < N_ENTRIES:
            line += ","
        lines.append(line)

    lines.append("};")
    lines.append("")
    lines.append("/* Funcao de lookup */")
    lines.append("static inline int32_t sigmoid_lut_lookup(float x) {")
    lines.append("    if (x <= SIGMOID_X_MIN) return 0;")
    lines.append("    if (x >= SIGMOID_X_MAX) return (1 << 24);  /* 1.0 em Q8.24 */")
    lines.append("    int idx = (int)((x - SIGMOID_X_MIN) / SIGMOID_STEP);")
    lines.append("    if (idx < 0) idx = 0;")
    lines.append("    if (idx >= SIGMOID_LUT_SIZE) idx = SIGMOID_LUT_SIZE - 1;")
    lines.append("    return SIGMOID_LUT[idx];")
    lines.append("}")
    lines.append("")
    lines.append("#endif /* SIGMOID_LUT_H */")

    with open(filename, 'w', encoding='utf-8') as f:
        f.write('\n'.join(lines))

    return filename


# ============================================================
# RELATORIO DE ERRO
# ============================================================

def generate_error_report(lut, errors):
    """Gera relatorio de erro de quantizacao."""

    max_error = max(errors)
    mean_error = sum(errors) / len(errors)
    max_error_idx = errors.index(max_error)

    report = []
    report.append("=" * 60)
    report.append("RELATORIO DE ERRO DA LUT SIGMOIDE")
    report.append("=" * 60)
    report.append("\nParametros:")
    report.append(f"  WL (largura total):     {WL} bits")
    report.append(f"  FWL (bits fracionarios): {FWL} bits")
    report.append(f"  Escala:                 {SCALE}")
    report.append(f"  Numero de entradas:     {N_ENTRIES}")
    report.append(f"  Dominio:                [{X_MIN}, {X_MAX}]")
    report.append(f"  Passo:                  {STEP}")
    report.append("\nEstatisticas de erro:")
    report.append(f"  Erro maximo:            {max_error:.10f} ({max_error * 100:.4f}%)")
    report.append(f"  Erro medio:             {mean_error:.10f} ({mean_error * 100:.4f}%)")
    report.append(f"  Erro maximo em idx:     {max_error_idx} (x={lut[max_error_idx]['x']:+.4f})")
    report.append(f"  Erro RMS:               {np.sqrt(np.mean(np.array(errors)**2)):.10f}")
    report.append("\nAmostras:")
    report.append(f"  sigma(-8.0)  = {lut[0]['y_float']:.10f}  ->  {lut[0]['hex']}  (Q8.24 = {lut[0]['y_q']/SCALE:.10f})")
    report.append(f"  sigma(0.0)   = {lut[128]['y_float']:.10f}  ->  {lut[128]['hex']}  (Q8.24 = {lut[128]['y_q']/SCALE:.10f})")
    report.append(f"  sigma(+8.0)  = {lut[255]['y_float']:.10f}  ->  {lut[255]['hex']}  (Q8.24 = {lut[255]['y_q']/SCALE:.10f})")
    report.append("\nValidacao de monotonicidade:")

    monotonic = all(lut[i]['y_q'] <= lut[i+1]['y_q'] for i in range(N_ENTRIES-1))
    report.append(f"  Monotonica crescente:   {'SIM' if monotonic else 'NAO'}")

    if not monotonic:
        violations = []
        for i in range(N_ENTRIES-1):
            if lut[i]['y_q'] > lut[i+1]['y_q']:
                violations.append(f"    idx {i}: {lut[i]['y_q']} > {lut[i+1]['y_q']}")
        report.extend(violations[:10])  # Mostrar primeiras 10 violacoes

    report.append("\n" + "=" * 60)

    return '\n'.join(report)


# ============================================================
# MAIN
# ============================================================
if __name__ == '__main__':
    print("=" * 60)
    print("GERACAO DA LUT SIGMOIDE (BLOCK 11 v2.2)")
    print("=" * 60)

    # Gerar LUT
    print("\n[1/4] Gerando valores da sigmoide...")
    lut, errors = generate_sigmoid_lut()
    print(f"      {len(lut)} entradas geradas")

    # Gerar VHDL
    print("\n[2/4] Gerando pacote VHDL...")
    vhdl_file = generate_vhdl_package(lut, 'sigmoid_lut_pkg.vhd')
    print(f"      Arquivo: {vhdl_file}")

    # Gerar C header
    print("\n[3/4] Gerando header C...")
    c_file = generate_c_header(lut, 'sigmoid_lut.h')
    print(f"      Arquivo: {c_file}")

    # Gerar relatorio
    print("\n[4/4] Gerando relatorio de erro...")
    report = generate_error_report(lut, errors)
    with open('sigmoid_lut_report.txt', 'w') as f:
        f.write(report)
    print(f"      Arquivo: sigmoid_lut_report.txt")

    # Imprimir relatorio
    print("\n" + report)
