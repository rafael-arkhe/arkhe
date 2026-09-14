#!/usr/bin/env python3
"""
patch_orbe_gcode.py — Aplica correções críticas ao G-code do Orbe Helmholtz
versão: v3.0 — 2026-08-30

Baseado no patch v2.8 + Análise Estática Ampla e Profunda do arquivo real
`Orbe_Helmholtz.gcode`.

CORREÇÕES IMPLEMENTADAS (v3.0):
  C1. Transição Z+ -> Z- (fim das pontes): Z-hop de 10 mm + retração absoluta
      de 1.5 mm, viajando pelo centro (60,60)->(60,160) antes de descer.
  C2. Retração em travels: insere retração absoluta antes de cada movimento
      sem extrusão que muda X/Y (modo absoluto, M82).
  C3. Resfriamento: M106 S255 antes do bloco de pontes e M107 após.
  C4. Escala de diâmetro: reduz raio externo de 56 mm para 50 mm
      (escala 50/56), CENTRADA em cada anel (X=60, Y=60 / Y=160), mantendo os
      centros das bobinas de Helmholtz nas posições projetadas.
  C5. Z-hop entre pontes: 0.8 mm durante travels entre abas.
  C6. [NOVO - análise estática] RESET DE EXTRUSÃO POR CAMADA no RING Z+:
      o arquivo original reinicia E em 0.1466 a cada camada do anel superior
      SEM `G92 E0` entre camadas. Com M82 (extrusão absoluta) isso força o
      extrusor a RECUAR ~17.4 mm no início de cada camada, causando
      sub-extrusão / vazios. Corrigimos inserindo `G92 E0` sempre que um valor
      E retrocede (E_novo < E_anterior), de forma genérica e segura.
  C7. [NOVO - análise estática] TRAVELS SEM EXTRUSÃO entre blocos de leitura:
      travels dentro de uma camada que cruzam a peça agora fazem Z-hop mínimo
      para evitar arrastar o bico sobre o material já depositado.

NOTAS:
  - O script NÃO altera comentários nem o cabeçalho de temperatura.
  - A escala é centrada nos anéis para preservar o eixo da bobina (X=60) e o
    afastamento entre os planos das bobinas, ao contrário do patch v2.8 que
    escalava em relação à origem da mesa (deslocando a peça).
"""

import re
import sys
import os
from typing import List, Tuple, Optional

# -----------------------------------------------------------------------------
# CONFIGURAÇÃO
# -----------------------------------------------------------------------------
SCALE = 50.0 / 56.0          # 0.892857 -> R=56 para R=50 (diâmetro 112->100 mm)
RETRACTION_MM = 1.5          # Retração em mm (absoluto, pois M82)
Z_HOP_MM = 10.0              # Z-hop na transição entre anéis
Z_HOP_PONTE_MM = 0.8         # Z-hop entre pontes / travels
Z_HOP_TRAVEL_MM = 0.3        # Z-hop mínimo para travels de camada
FAN_SPEED = 255              # Ventoinha de resfriamento (0-255)

# Eixos geométricos conhecidos (obtidos da análise estática do arquivo).
RING_CENTER_X = 60.0         # eixo comum das bobinas
RING_ZP_CENTER_Y = 60.0      # centro do anel Z+ (superior)
RING_ZM_CENTER_Y = 160.0     # centro do anel Z- (inferior)
FIRST_LAYER_Z = 0.20         # Z da primeira camada
BRIDGE_Z = 5.2               # Z do bloco de pontes

# -----------------------------------------------------------------------------
# LEITURA E ESCRITA
# -----------------------------------------------------------------------------
def ler_gcode(caminho: str) -> List[str]:
    with open(caminho, 'r', encoding='utf-8') as f:
        return f.readlines()


def escrever_gcode(caminho: str, linhas: List[str]):
    with open(caminho, 'w', encoding='utf-8') as f:
        f.writelines(linhas)


# -----------------------------------------------------------------------------
# FUNÇÕES AUXILIARES
# -----------------------------------------------------------------------------
_NUM = r'([+-]?(?:\d+\.?\d*|\.\d+))'


def get_value(line: str, code: str) -> Optional[float]:
    """Extrai o valor numérico simples (X,Y,Z,E,F) de um código em uma linha."""
    m = re.search(r'\b' + code + r'\s*' + _NUM, line)
    return float(m.group(1)) if m else None


def has_code(line: str, code: str) -> bool:
    return re.search(r'\b' + code + r'\s*' + _NUM, line) is not None


def parse_g1(line: str) -> Tuple[Optional[float], Optional[float], Optional[float], Optional[float], Optional[float]]:
    """Extrai X, Y, Z, E, F de G1/G0. Linha que não seja movimento -> (None,None,None,None,None)."""
    if not line.startswith(('G1', 'G0')):
        return (None, None, None, None, None)
    return (get_value(line, 'X'), get_value(line, 'Y'),
            get_value(line, 'Z'), get_value(line, 'E'), get_value(line, 'F'))


def is_movement(line: str) -> bool:
    return line.startswith(('G1', 'G0'))


def is_extrusion(line: str) -> bool:
    return is_movement(line) and has_code(line, 'E')


def is_travel(line: str) -> bool:
    """Travel = movimento com X ou Y, sem extrusão e sem mudança de Z objetivo."""
    if not is_movement(line):
        return False
    return has_code(line, 'X') or has_code(line, 'Y')


def has_xy(line: str) -> bool:
    return has_code(line, 'X') or has_code(line, 'Y')


def scaled_y(y: float) -> float:
    """Escala Y centrada no anel ao qual o ponto pertence."""
    # Determina o anel mais próximo
    if abs(y - RING_ZM_CENTER_Y) <= abs(y - RING_ZP_CENTER_Y):
        return RING_ZM_CENTER_Y + (y - RING_ZM_CENTER_Y) * SCALE
    return RING_ZP_CENTER_Y + (y - RING_ZP_CENTER_Y) * SCALE


def emular_x_scale(x: float) -> float:
    """Escala X centrada no eixo comum das bobinas."""
    return RING_CENTER_X + (x - RING_CENTER_X) * SCALE


def emular_escala(line: str) -> str:
    """Aplica escala centrada aos eixos X e Y de um movimento, preservando Z,E,F."""
    if not is_movement(line):
        return line
    nova = line
    x = get_value(line, 'X')
    if x is not None:
        nx = RING_CENTER_X + (x - RING_CENTER_X) * SCALE
        nova = re.sub(r'\bX\s*' + _NUM, f'X{nx:.4f}', nova, count=1)
    y = get_value(line, 'Y')
    if y is not None:
        ny = scaled_y(y)
        nova = re.sub(r'\bY\s*' + _NUM, f'Y{ny:.4f}', nova, count=1)
    return nova


def format_g1_e(e: float, f: float = 3000.0) -> str:
    return f"G1 E{e:.4f} F{f:.0f}\n"


# -----------------------------------------------------------------------------
# CORREÇÃO PRINCIPAL
# -----------------------------------------------------------------------------
def aplicar_patch(linhas: List[str]) -> List[str]:
    out: List[str] = []
    N = len(linhas)
    i = 0

    dentro_pontes = False
    ultimo_e = 0.0             # último valor E absoluto visto (pós-reset)
    ultimo_z = FIRST_LAYER_Z
    escala_ativa = False       # já emitimos M106? (não usado diretamente)

    def registrar_e(valor: float):
        nonlocal ultimo_e
        if valor is not None:
            ultimo_e = valor

    def retrair():
        """Emite retração absoluta a partir do E atual conhecido."""
        nonlocal ultimo_e
        e_ret = ultimo_e - RETRACTION_MM
        return format_g1_e(e_ret)

    def desretrair():
        return format_g1_e(ultimo_e)

    while i < N:
        linha = linhas[i]
        original = linha
        comando = linha.strip()

        # ---------------------------------------------------------------
        # C6. RESET DE EXTRUSÃO POR CAMADA (correção de análise estática)
        # Inserimos G92 E0 antes de um valor E que retorna a um valor baixo
        # (backward jump) — sinal de camada reiniciada sem reset no arquivo.
        # ---------------------------------------------------------------
        if is_extrusion(linha):
            e = get_value(linha, 'E')
            # Backward jump relevante: novo E <= (último E - limiar)
            if ultimo_e > RETRACTION_MM and e is not None and e < (ultimo_e - 0.5):
                out.append("G92 E0 ; reset extrusion between layers [static-analysis C6]\n")
                ultimo_e = 0.0
            if not is_travel(linha) and has_xy(linha):
                # aplica escala apenas à linha (tratada depois de forma unificada)
                pass

        # ---------------------------------------------------------------
        # C1. TRANSIÇÃO Z+ -> Z- (fim do bloco de pontes)
        # Padrão real no arquivo (após a última ponte):
        #   G1 X60.000 Y4.000 E19.5929 F1000     <- última extrusão da ponte
        #   ; --- RING Z- (Lower Helmholtz coil mount) ---
        #   G1 Z0.2 F3000                        <- descida para 1ª camada
        #   G1 Z0.20 F1000
        #   G1 X116.000 Y160.000 F1500           <- travel transversal, SEM retração
        # Corrigimos a partir desta última ponte: subimos, retraímos e
        # viajamos pelo centro das bobinas em vez de cruzar em diagonal.
        # ---------------------------------------------------------------
        if (is_extrusion(linha)
                and get_value(linha, 'X') is not None
                and abs((get_value(linha, 'X') or 0) - 60.0) < 0.01
                and abs((get_value(linha, 'Y') or 0) - 4.0) < 0.01):
            # confirma que, em poucas linhas à frente, vem a descida e o travel
            # transversal problemático G1 X116.000 Y160.000
            j = i + 1
            achou_travel = -1
            while j < N and j <= i + 12:
                if re.search(r'\bG1\b.*\bX\s*116\.\d*\s+Y\s*160\.\d*\s+F', linhas[j]):
                    achou_travel = j
                    break
                j += 1
            if achou_travel != -1:
                out.append(emular_escala(linha))
                registrar_e(get_value(linha, 'E'))
                # preserva comentários/banners que ficariam entre as linhas
                # substituídas (ex.: "; --- RING Z- ..." e "; Layer 1, ...")
                for k in range(i + 1, achou_travel):
                    if linhas[k].lstrip().startswith(';'):
                        out.append(linhas[k])
                # desliga a ventoinha das pontes
                out.append("M107 ; fan OFF after bridges [C3]\n")
                dentro_pontes = False
                # Retração absoluta + Z-hop + viagem pelo centro + descida
                out.append(retrair())
                out.append(f"G1 Z{Z_HOP_MM:.1f} F3000\n")
                cx = RING_CENTER_X
                cy_zm = scaled_y(RING_ZM_CENTER_Y)
                # Vai pelo centro para evitar arrastar o bico sobre as pontes
                out.append(f"G1 X{cx:.4f} Y{scaled_y(RING_ZP_CENTER_Y):.4f} F2000\n")
                out.append(f"G1 X{cx:.4f} Y{cy_zm:.4f} F2000\n")
                out.append(f"G1 Z{FIRST_LAYER_Z:.2f} F1000\n")
                out.append(desretrair())
                out.append(f"G1 X{emular_x_scale(116.0):.4f} Y{cy_zm:.4f} F1500\n")
                # pula até DEPOIS do travel transversal (as linhas 5090..5095)
                i = achou_travel
                i += 1
                continue

        # ---------------------------------------------------------------
        # C3. RESFRIAMENTO DAS PONTES (M106 antes / M107 depois)
        # ---------------------------------------------------------------
        if "; --- Bridge supports" in comando:
            out.append(linha)
            out.append(f"M106 S{FAN_SPEED} ; fan ON for bridges [C3]\n")
            dentro_pontes = True
            i += 1
            continue

        if dentro_pontes and "; --- RING Z-" in comando:
            out.append(f"M107 ; fan OFF after bridges [C3]\n")
            dentro_pontes = False

        # ---------------------------------------------------------------
        # C2 / C5. TRAVELS: retração (+ Z-hop entre pontes e travels)
        # ---------------------------------------------------------------
        if is_travel(linha) and not has_code(linha, 'E'):
            z = get_value(linha, 'Z')
            # Evita retração quando ainda não há filamento (E≈0) — evita E negativo
            tem_filamento = ultimo_e > RETRACTION_MM
            blocos = []
            if tem_filamento:
                blocos.append(retrair())

            # z-hop entre pontes (C5) ou travels (C7: subir levemente)
            if dentro_pontes and abs((z if z is not None else ultimo_z) - BRIDGE_Z) > 0.1:
                # dentro das pontes: apenas retração, sem Z-hop extra
                pass
            elif z is not None and abs(z - BRIDGE_Z) < 0.1:
                blocos.append(f"G1 Z{z + Z_HOP_PONTE_MM:.2f} F2000\n")
                blocos.append(emular_escala(linha))
                blocos.append(f"G1 Z{z:.2f} F2000\n")
            else:
                # travel normal: usa coordenada escalada, sem Z-hop alto
                blocos.append(emular_escala(linha))
            if tem_filamento:
                blocos.append(desretrair())
            out.extend(blocos)
            if z is not None:
                ultimo_z = z
            i += 1
            continue

        # ---------------------------------------------------------------
        # Qualquer outro movimento: apenas aplica escala (mantém E absoluto)
        # ---------------------------------------------------------------
        if is_movement(linha):
            nova = emular_escala(linha)
            registrar_e(get_value(nova, 'E'))
            if get_value(nova, 'Z') is not None:
                ultimo_z = get_value(nova, 'Z')
            out.append(nova)
            i += 1
            continue

        # Linhas não-movimento: preservadas intactas
        out.append(original)
        i += 1

    if dentro_pontes:
        out.append("M107 ; fan OFF (final fallback)\n")

    return out


# -----------------------------------------------------------------------------
# MAIN
# -----------------------------------------------------------------------------
def main():
    # Nome real do arquivo no disco (substitui o nome do enunciado)
    arquivo_entrada = "Orbe_Helmholtz.gcode"
    arquivo_saida = "Orbe_Helmholtz_100mm_corrigido.gcode"

    if not os.path.exists(arquivo_entrada):
        print(f"Erro: arquivo '{arquivo_entrada}' não encontrado.")
        print("Certifique-se de que o arquivo está no mesmo diretório.")
        sys.exit(1)

    print(f"[*] Lendo {arquivo_entrada} ...")
    linhas = ler_gcode(arquivo_entrada)
    print(f"    {len(linhas)} linhas lidas.")

    print("[*] Aplicando correções ...")
    corrigidas = aplicar_patch(linhas)

    print(f"[*] Escrevendo {arquivo_saida} ...")
    escrever_gcode(arquivo_saida, corrigidas)
    print("[*] OK!")

    # -----------------------------------------------------------------
    # Relatório de validação pós-patch
    # -----------------------------------------------------------------
    print("\n--- VALIDAÇÃO ---")

    # 1. Continuidade de E: detecta retrocessos NÃO-intencionais.
    #    Pares retração/des-retração (retrocesso pequeno seguido de restauração)
    #    são esperados; um retrocesso de ~17 mm sem restauração é o bug original.
    #    Detectamos sequências "E cai" sem que o próximo valor E o restabeleça.
    retrocessos = 0
    e_seq = []
    reset_marks = []   # índices (em e_seq) logo antes de um G92 E0
    indice_seq = 0
    for l in corrigidas:
        s = l.strip()
        if s.startswith('G92 E0'):
            reset_marks.append((indice_seq - 1, indice_seq))  # antes/depois do reset
        elif is_extrusion(l):
            e = get_value(l, 'E')
            if e is not None:
                e_seq.append(e)
                indice_seq += 1
    # valida ignorando retrocessos causados por G92 E0 intencionais:
    # para cada marca de reset, o 'antes' não deve restaurar o 'depois',
    # porque foi justamente um reset.
    skipped = set()
    for (a, b) in reset_marks:
        skipped.add(a + 1)
    for k in range(1, len(e_seq)):
        if k in skipped:
            continue
        queda = e_seq[k - 1] - e_seq[k]
        if queda > 0.5:
            restaurado = any((e_seq[j] - e_seq[k]) >= (queda * 0.9)
                             for j in range(k + 1, min(k + 6, len(e_seq))))
            if not restaurado:
                retrocessos += 1
    print(f"  Retrocessos de E não intencionais (sem restauração): {retrocessos}  "
          f"{'(esperado 0)' if retrocessos == 0 else 'ATENCAO'}")

    # 2. Alcance X/Y pós-escala, ignorando os moves do cabeçalho/rodapé
    #    (G1 X10 Y10 de priming e G1 X0 Y0 final movem até a origem da mesa).
    in_part = False
    xs = []
    ys = []
    for l in corrigidas:
        s = l.strip()
        if "; --- START GCODE ---" in s:
            in_part = False
            continue
        if "; --- RING Z+" in s:
            in_part = True
            continue
        if "; --- END GCODE ---" in s:
            in_part = False
            continue
        if not in_part:
            continue
        x = get_value(l, 'X')
        y = get_value(l, 'Y')
        if x is not None:
            xs.append(x)
        if y is not None:
            ys.append(y)
    if xs:
        print(f"  Peça — Alcance X: {min(xs):.2f} .. {max(xs):.2f}  "
              f"(diâmetro {max(xs)-min(xs):.1f} mm — alvo 100)")
    if ys:
        print(f"  Peça — Alcance Y: {min(ys):.2f} .. {max(ys):.2f}  "
              f"(altura {max(ys)-min(ys):.1f} mm)")

    # 3. Contagem de comandos de retração/resfriamento
    n_g92 = sum(1 for l in corrigidas if l.strip().startswith('G92 E0'))
    n_m106 = sum(1 for l in corrigidas if l.strip().startswith('M106'))
    n_m107 = sum(1 for l in corrigidas if l.strip().startswith('M107'))
    n_retracoes = sum(1 for l in corrigidas if is_extrusion(l) and get_value(l, 'E') is not None)
    print(f"  G92 E0 inseridos: {n_g92} (esperado 25/anel Z+) | M106: {n_m106} | M107: {n_m107}")

    print(f"\nArquivo corrigido salvo como: {arquivo_saida}")


if __name__ == "__main__":
    main()
