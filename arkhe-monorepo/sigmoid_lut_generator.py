#!/usr/bin/env python3
"""
sigmoid_lut_generator.py
Gera LUT para função sigmoide em formato compatível com BLOCK 11 v2.2.
Domínio: [-8.0, +8.0], 256 entradas, saída em Q8.24 (clamping).
"""

import numpy as np
import struct

def sigmoid(x):
    return 1.0 / (1.0 + np.exp(-x))

def generate_sigmoid_lut(
    n_entries=256,
    input_bits=8,
    output_bits=32,    # Q8.24 (8 bits inteiros, 24 bits fracionários)
    x_min=-8.0,
    x_max=8.0,
    out_format='verilog',
    filename='sigmoid_lut.v'
):
    """
    Gera LUT para sigmoide.

    Parâmetros:
        n_entries: potência de 2 (ex: 256)
        input_bits: largura do endereço (ex: 8)
        output_bits: largura da saída em bits (ex: 32 para Q8.24)
        x_min, x_max: domínio da função
        out_format: 'verilog', 'vhdl', 'c_header', 'binary'
        filename: arquivo de saída
    """
    # Gera valores de entrada uniformes no domínio
    x_values = np.linspace(x_min, x_max, n_entries)
    y_values = sigmoid(x_values)

    # Escala para Q8.24: 8 bits inteiros (sinal + 7) + 24 bits fracionários
    # O valor máximo é < 1, então usamos 1.0 como referência.
    # Q8.24 representa valores de -128.0 a 127.99999994.
    # Para sigmoide, usamos escala: y * 2^24, e truncamos para inteiro.
    scale = 2 ** (output_bits - 8)  # para Q8.24, 2^24 = 16777216
    y_scaled = y_values * scale
    y_int = np.round(y_scaled).astype(np.uint64)
    # Clamping: garantimos que não ultrapasse o máximo (2^32-1)
    max_val = 2**output_bits - 1
    y_int = np.clip(y_int, 0, max_val)

    # Escreve arquivo
    with open(filename, 'w') as f:
        if out_format == 'verilog':
            _write_verilog(f, y_int, input_bits, output_bits)
        elif out_format == 'vhdl':
            _write_vhdl(f, y_int, input_bits, output_bits)
        elif out_format == 'c_header':
            _write_c_header(f, y_int, input_bits, output_bits, n_entries)
        elif out_format == 'binary':
            _write_binary(f, y_int, output_bits)
        else:
            raise ValueError(f"Formato {out_format} não suportado")

    print(f"LUT gerada: {filename} ({n_entries} entradas, {output_bits} bits)")

def _write_verilog(f, data, addr_bits, data_bits):
    f.write(f"// Sigmoide LUT - {len(data)} entradas\n")
    f.write(f"// Input: {addr_bits} bits, Output: {data_bits} bits\n")
    f.write("module sigmoid_lut (\n")
    f.write(f"    input  wire [{addr_bits-1}:0] addr,\n")
    f.write(f"    output reg  [{data_bits-1}:0] data_out\n")
    f.write(");\n\n")
    f.write(f"    reg [{data_bits-1}:0] mem [0:{len(data)-1}];\n\n")
    f.write("    initial begin\n")
    for i, val in enumerate(data):
        f.write(f"        mem[{i}] = {data_bits}'d{val};\n")
    f.write("    end\n\n")
    f.write("    always @(*) data_out = mem[addr];\n")
    f.write("endmodule\n")

def _write_vhdl(f, data, addr_bits, data_bits):
    f.write(f"-- Sigmoide LUT - {len(data)} entradas\n")
    f.write(f"-- Input: {addr_bits} bits, Output: {data_bits} bits\n")
    f.write("library ieee;\n")
    f.write("use ieee.std_logic_1164.all;\n")
    f.write("use ieee.numeric_std.all;\n\n")
    f.write("entity sigmoid_lut is\n")
    f.write(f"    port (\n")
    f.write(f"        addr     : in  unsigned({addr_bits-1} downto 0);\n")
    f.write(f"        data_out : out unsigned({data_bits-1} downto 0)\n")
    f.write("    );\n")
    f.write("end entity;\n\n")
    f.write("architecture rtl of sigmoid_lut is\n")
    f.write(f"    type lut_array is array (0 to {len(data)-1}) of unsigned({data_bits-1} downto 0);\n")
    f.write("    constant lut : lut_array := (\n")
    for i, val in enumerate(data):
        suffix = "," if i < len(data)-1 else ""
        f.write(f"        {data_bits}x\"{val:08X}\"{suffix}\n")
    f.write("    );\n")
    f.write("begin\n")
    f.write("    data_out <= lut(to_integer(addr));\n")
    f.write("end architecture;\n")

def _write_c_header(f, data, addr_bits, data_bits, n_entries):
    f.write(f"// Sigmoide LUT - {n_entries} entradas\n")
    f.write(f"// Input: {addr_bits} bits, Output: {data_bits} bits\n\n")
    f.write(f"#define SIGMOID_LUT_SIZE {n_entries}\n")
    f.write(f"#define SIGMOID_DATA_BITS {data_bits}\n\n")
    f.write(f"const uint{data_bits}_t sigmoid_lut[{n_entries}] = {{\n")
    for i, val in enumerate(data):
        if i % 8 == 0 and i > 0:
            f.write("\n")
        suffix = "," if i < n_entries-1 else ""
        f.write(f"    {val}{suffix} ")
    f.write("\n};\n")

def _write_binary(f, data, data_bits):
    bytes_per = (data_bits + 7) // 8
    for val in data:
        f.write(val.to_bytes(bytes_per, byteorder='little'))

if __name__ == "__main__":
    # Gera versão Verilog para BLOCK 11
    generate_sigmoid_lut(
        n_entries=256,
        input_bits=8,
        output_bits=32,          # Q8.24
        x_min=-8.0,
        x_max=8.0,
        out_format='verilog',
        filename='sigmoid_lut_block11.v'
    )

    # Gera versão C header (para integração com software)
    generate_sigmoid_lut(
        n_entries=256,
        input_bits=8,
        output_bits=32,
        x_min=-8.0,
        x_max=8.0,
        out_format='c_header',
        filename='sigmoid_lut.h'
    )

    print("\nArquivos gerados: sigmoid_lut_block11.v e sigmoid_lut.h")
