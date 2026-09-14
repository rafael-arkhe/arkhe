# Orbe — Manufacturing Files

**Modelo:** ORB-01 · **Versão:** v1.0 · **Data:** 2026-08-30
**Documentos relacionados:** [Manufacturing Documentation](./Manufacturing_Documentation.md), [Desenho Mecânico](../02-design/Mechanical_Drawings.md)

---

## 1. Pacote de Fabricação (estrutura FDM)

A estrutura Helmholtz é fabricada por manufatura aditiva (FDM). O pacote de fabricação da estrutura inclui:

| Item | Arquivo | Finalidade | Status |
|------|---------|------------|--------|
| G-code corrigido | `Orbe_Helmholtz_100mm_corrigido.gcode` | Impressão 3D da estrutura | ✅ Gerado |
| G-code original | `Orbe_Helmholtz.gcode` | Referência histórica | ✅ Referência |
| Script de correção | `patch_orbe_gcode.py` | Regenerar o G-code corrigido | ✅ Gerado |
| Modelo OpenSCAD | `orbe_helmholtz.scad` | Modelo 3D parametrizado (fonte) | ✅ Gerado |
| STL anel Z+ | `orbe_ring_ZP.stl` | Impressão 3D do anel superior (com pontes) | ✅ Gerado |
| STL anel Z- | `orbe_ring_ZM.stl` | Impressão 3D do anel inferior | ✅ Gerado |
| STL montagem | `orbe_assembly_both.stl` | Conjunto completo (visual; não imprimível) | ✅ Gerado |
| Preview montagem | `orbe_assembly_both.png` | Visualização do conjunto | ✅ Gerado |
| Desenho mecânico | `Mechanical_Drawings.md` | Dimensões e tolerâncias | ✅ Gerado |

> Os arquivos `.gcode` e `.py` residem na raiz do workspace (`C:\Users\Lemes\Downloads\sasc-v34.8-ω-__-real-implementation-engine\`).
>
> Os STLs de cada anel são **manifold válido** (2-manifold, imprimíveis). No G-code real os dois anéis se sobrepõem na mesa em Y (região 104–116 mm); para fabricação são exportados como duas peças separadas (`part = "zp"` / `part = "zm"`). O anel Z+ inclui as 4 abas de ponte fundidas ao topo (resfriamento 100% recomendado nas pontes).
>
> O **STL de montagem** (`orbe_assembly_both.stl`, `part = "both"` = padrão) mostra o conjunto completo no plano da mesa (X 10–110, Y 10–210, Z 0–6). Por representar fielmente a sobreposição real de 12 mm entre os anéis coplanares, ele **não é 2-manifold** — é um artefato de visualização (all-set), não para impressão. Use os dois STLs individuais para fabricação.

---

## 2. Parâmetros de Impressão Recomendados

| Parâmetro | Valor |
|-----------|-------|
| Material | PLA (protótipo) |
| Nozzle | 0.4 mm |
| Altura de camada | 0.2 mm |
| Infill | 20% |
| Temperatura do hotend | 210 °C |
| Temperatura da mesa | 60 °C |
| Velocidade de impressão | 1500 mm/min |
| Retração | 1.5 mm (absoluta) |
| Resfriamento das pontes | 100% (M106 S255) |
| Z-hop (transição de anel) | 10.0 mm |

---

## 3. Verificação de Impressão (Checklist)

- [ ] Diâmetro externo medido = 100 ±0.5 mm.
- [ ] Dois anéis coaxiais sem desalinhamento visível.
- [ ] Quatro abas de ponte íntegras (resfriamento aplicado).
- [ ] Sem stringing excessivo entre travels (retração ativa).
- [ ] Sem vazios/sub-extrusão no início de cada camada do anel superior (G92 E0 inserido).
- [ ] Dimensão raio interno ≈ 39.3 mm.

---

## 4. Fabricação de PCB (visão/roadmap)

Quando a camada eletrônica for desenvolvida, o pacote de PCB deverá conter:

1. **Gerber files** (camadas de cobre, máscara de solda, silk screen).
2. **Drill files** (furação).
3. **Pick-and-place files** (montagem SMT).
4. **Stencil files** (solda SMT).
5. **Assembly drawings** (vista explodida).

> XXXX-XX : *a gerar na fase de integração eletrônica.*

---

## 5. Controle de Versão

| Versão | Data | Conteúdo |
|--------|------|----------|
| v1.0 | 2026-08-30 | G-code corrigido + documentação |

> Regenere o G-code a partir do modelo 3D com R=50 mm para produção final, conforme recomendação do script.
