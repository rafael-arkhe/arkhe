# Orbe — Desenho Mecânico

**Modelo:** ORB-01 · **Versão:** v1.0 · **Data:** 2026-08-30
**Escala:** 1:1 · **Unidades:** mm · **Padrão:** BS 8888 (referência)
**Documentos relacionados:** [PSD](../01-conceituacao/PSD.md), [BOM](./BOM.csv)

> Dimensões derivadas da análise estática do G-code real e da correção de escala 50/56 aplicada em `Orbe_Helmholtz_100mm_corrigido.gcode`.

---

## 1. Vista do Conjunto (montagem)

```
          ANEL Z+ (superior)
          ┌─────────────────────────────────┐
          │           ┌───────────────┐     │
          │           │   R=50.0      │     │
          │     R=39.3│               │     │
          │   ◄───────┤   (interno)   ├───► │
          │           │               │     │
          │           └───────────────┘     │
          └─────────────────────────────────┘
                       ▲  ▲  ▲  ▲
                       │  │  │  │  4 abas de ponte (suportes)
                       ▼  ▼  ▼  ▼
          ┌─────────────────────────────────┐
          │           ┌───────────────┐     │
          │           │   R=50.0      │     │
          │     R=39.3│   centro      │     │
          │           │  Y=160-c      │     │
          │           └───────────────┘     │
          └─────────────────────────────────┘
          ANEL Z- (inferior) — plano de montagem
```

**Eixo compartilhado (coaxial):** X = centro das bobinas.

---

## 2. Vista Frontal (corte da seção dos anéis)

```
         ┌────────────────────────────────────┐
         │           ø100 ±0.5                │
         │   ┌───────────────────────────┐    │
         │   │   parede do anel          │    │
         │   │   (10.7 mm largura)       │    │
         │   └───────────────────────────┘    │
         │         R=50.0    R=39.3           │
         │         (externo) (interno)        │
         └────────────────────────────────────┘
```

---

## 3. Vista Lateral (empilhamento dos anéis)

```
  Z+  ┌────────────────────────────────────┐
      │        anel superior (5.0 mm)      │  Z 4.8–5.2
      ├────────────────────────────────────┤
      │   (pontes / suportes)              │
      ├────────────────────────────────────┤
  Z-  │        anel inferior (5.0 mm)      │  Z 0.2–5.0
      └────────────────────────────────────┘
```

| Item | Valor |
|------|-------|
| Altura de impressão por anel | 5.0 mm (25 camadas × 0.2 mm) |
| Z de início do anel Z+ | 0.2 mm |
| Z das pontes | 5.2 mm |

---

## 4. Tabela de Dimensões Principais

| Dimensão | Valor | Tolerância |
|----------|-------|------------|
| Diâmetro externo (D) | 100.0 mm | ±0.5 mm |
| Raio externo | 50.0 mm | ±0.25 mm |
| Raio interno | 39.3 mm | ±0.25 mm |
| Largura radial do anel | 10.7 mm | ±0.2 mm |
| Altura do anel | 5.0 mm | ±0.1 mm |
| Tolerância geral | — | ±0.2 mm |

---

## 5. Notas Técnicas

- **Material (protótipo):** PLA, impressão FDM 3D.
- **Material (produção a 77 K):** PEEK ou PA12+CF.
- **Resfriamento de ponte:** ventoinha a 100% (M106 S255) durante os suportes.
- **Orientações de impressão:** anéis no plano XY; eixo das bobinas no plano vertical (Y).
- **Acabamento:** lixamento leve das abas de ponte; sem solventes.

---

## 6. Arquivos CAD e de Fabricação

| Tipo | Arquivo | Status |
|------|---------|--------|
| G-code corrigido | `Orbe_Helmholtz_100mm_corrigido.gcode` | Gerado |
| G-code original | `Orbe_Helmholtz.gcode` | Referência |
| Script de correção | `patch_orbe_gcode.py` | Gerado |
| Modelo 3D (CAD) | `03-fabricacao/orbe_helmholtz.scad` | Gerado (OpenSCAD parametrizado) |
| STL anel Z+ | `03-fabricacao/orbe_ring_ZP.stl` | Gerado (manifold válido) |
| STL anel Z- | `03-fabricacao/orbe_ring_ZM.stl` | Gerado (manifold válido) |
