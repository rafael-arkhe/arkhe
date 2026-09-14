# Orbe — Manual de Manutenção

**Modelo:** ORB-01 · **Versão:** v1.0 · **Data:** 2026-08-30
**Documentos relacionados:** [BOM](../02-design/BOM.csv), [Manufacturing Documentation](../03-fabricacao/Manufacturing_Documentation.md)

---

## 1. Procedimentos de Manutenção Preventiva

### 1.1. Inspeção Dimensional

- **Frequência:** A cada validação de lote / após manuseio.
- **Procedimento:** Medir diâmetro externo (100 ±0.5 mm) e raio interno (~39.3 mm).

### 1.2. Limpeza

- **Frequência:** Conforme necessidade.
- **Procedimento:** Use pano macio e seco; não use solventes no PLA.

### 1.3. Inspeção Visual

- Verificar integridade das abas de ponte.
- Verificar delaminação ou trincas.
- Verificar coaxialidade dos anéis.

---

## 2. Procedimentos de Reparo

### 2.1. Substituição da Estrutura

Se a estrutura apresentar dano estrutural:

1. Gere novo G-code corrigido (se necessário) executando `patch_orbe_gcode.py`.
2. Reimprima a partir de `Orbe_Helmholtz_100mm_corrigido.gcode`.
3. Inspecione conforme Check-List (Manufacturing_Files.md).

### 2.2. Componentes Eletrônicos (visão/roadmap)

- **Memristores:** substituir conforme procedimento SMT do fabricante.
- **Bateria:** substituir apenas com desligamento completo.
- **Sensor de temperatura (TMP117):** substituir via reflow SMT.

---

## 3. Diagramas de Referência

- Esquemático elétrico: [Schematics](../02-design/Schematics.md).
- Desenho mecânico: [Mechanical_Drawings](../02-design/Mechanical_Drawings.md).
- BOM: [BOM](../02-design/BOM.csv).

---

## 4. Lista de Peças de Serviço

| Peça | Código | Fornecedor |
|------|--------|------------|
| Estrutura Helmholtz | ORB-ST-001 | Fabricação interna (FDM) |
| Filamento PLA | PLA-01 | Fornecedor de filamento |
| (visão) Memristor | Si3N4-MEM-01 | Custom |
| (visão) Bateria | SP-100 | SolidPower |
| (visão) Sensor de temperatura | TMP117 | Texas Instruments |

---

## 5. Registro de Manutenção

| Data | Atividade | Responsável | Resultado |
|------|-----------|-------------|-----------|
| — | — | — | — |
