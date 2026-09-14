# Documentação do Orbe (Substrato 156 — Dipolo de Arkhe)

Documentação técnica do **Orbe** (estrutura de bobinas de Helmholtz, modelo ORB-01), formalizando o dispositivo como produto de hardware segundo padrões internacionais de engenharia.

**Versão:** v1.0 · **Data:** 2026-08-30
**Base:** análise estática do G-code real (`Orbe_Helmholtz.gcode`) e correção de escala/diâmetro para 100 mm.

---

## Visão Geral

O Orbe é uma estrutura de **duas bobinas de Helmholtz coaxiais**, impressa em 3D (FDM), **Substrato 156 — Dipolo de Arkhe**. A documentação segue uma abordagem **híbrida**:

- **Estrutura física (real):** dois anéis coaxiais, raio externo 50 mm (diâmetro 100 mm), raio interno ~39.3 mm, unidos por 4 pontes. Fabricável a partir do G-code corrigido.
- **Conceito teórico (visão/roadmap):** processador coerente com campo C(t), handovers, memristores e metassuperfície — registrado como requisitos de visão.

### Arquivos de fabricação (referência)

| Arquivo | Finalidade |
|---------|------------|
| `Orbe_Helmholtz_100mm_corrigido.gcode` | G-code de produção (extrusão absoluta, retração, resfriamento, escala R=50 mm) |
| `Orbe_Helmholtz.gcode` | Referência histórica (original) |
| `patch_orbe_gcode.py` | Script que gera o G-code corrigido |

---

## Índice de Documentos

### Fase 1 — Conceituação e Requisitos
- [Product Requirements Document (PRD)](./01-conceituacao/PRD.md)
- [Product Specification Document (PSD)](./01-conceituacao/PSD.md)

### Fase 2 — Design e Engenharia
- [Hardware Design Document (HDD)](./02-design/HDD.md)
- [Schematics (Esquemático Elétrico)](./02-design/Schematics.md)
- [Mechanical Drawings (Desenho Mecânico)](./02-design/Mechanical_Drawings.md)
- [Bill of Materials (BOM)](./02-design/BOM.csv)
- [Firmware Documentation](./02-design/Firmware_Documentation.md)

### Fase 3 — Fabricação e Produção
- [Manufacturing Files](./03-fabricacao/Manufacturing_Files.md)
- [Manufacturing Documentation](./03-fabricacao/Manufacturing_Documentation.md)

### Fase 4 — Operação e Manutenção
- [User Manual (Manual do Usuário)](./04-operacao/User_Manual.md)
- [Maintenance Manual (Manual de Manutenção)](./04-operacao/Maintenance_Manual.md)

### Fase 5 — Conformidade Regulatória
- [Technical File (Arquivo Técnico)](./05-conformidade/Technical_File.md)
- [Declaração de Conformidade CE (modelo)](./05-conformidade/DoC_CE.md)
- [Certificações Regulatórias](./05-conformidade/Certificacoes.md)

---

## Estrutura de Diretórios

```
docs/orbe/
├── 01-conceituacao/   (PRD, PSD)
├── 02-design/         (HDD, Schematics, Mechanical, BOM, Firmware)
├── 03-fabricacao/     (Manufacturing Files, Manufacturing Documentation)
├── 04-operacao/       (User Manual, Maintenance Manual)
└── 05-conformidade/   (Technical File, DoC CE, Certificações)
└── README.md          (este índice)
```

---

## Status de Conformidade

> ⚠️ Documentos de conformidade (Fase 5) são **modelos/rascunhos**. As certificações (CE, FCC, ANATEL, RoHS) exigem testes em laboratório acreditado e **não foram executados**. Não assinar/publicar a DoC até a conclusão dos testes.
