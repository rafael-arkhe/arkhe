# Orbe — Manufacturing Documentation

**Modelo:** ORB-01 · **Versão:** v1.0 · **Data:** 2026-08-30
**Documentos relacionados:** [Manufacturing Files](./Manufacturing_Files.md), [BOM](../02-design/BOM.csv)

---

## 1. Instruções de Montagem

### 1.1. Materiais Necessários

- Estrutura Helmholtz impressa em PLA (protótipo) ou PEEK/PA12+CF (produção a 77 K).
- (Visão) Placa de circuito impresso (PCB) com componentes SMT.
- (Visão) Câmara de plasma Xe (pré-montada).
- (Visão) Metassuperfície TiO₂.
- Parafusos de fixação M6 (se aplicável).

### 1.2. Processo de Montagem (Passo a Passo)

1. Imprima a estrutura a partir de `Orbe_Helmholtz_100mm_corrigido.gcode`.
2. Inspecione a estrutura (ver Checklist em Manufacturing_Files.md).
3. Lixe levemente as abas de ponte se necessário (sem solventes).
4. (Visão) Instale a PCB no plano de montagem do anel inferior.
5. (Visão) Posicione a câmara de plasma no centro coaxial.
6. (Visão) Conecte a metassuperfície à superfície superior.
7. (Visão) Conecte a alimentação USB-C.
8. (Produção) Feche e realize vedação hermética (IP68+).

---

## 2. Procedimentos de Teste

| Teste | Procedimento | Critério de Aceitação |
|-------|--------------|----------------------|
| Dimensional | Medir diâmetro externo | 100 ±0.5 mm |
| Coaxialidade | Alinhar eixos dos anéis | ≤ 0.5 mm de desvio |
| Integridade das pontes | Inspeção visual | Sem trincas/deformação |
| (Visão) Funcional | Boot do firmware | LED/LED de status verde |
| (Visão) Coerência C(t) | Medir com sensor | C(t) ≥ 0.85 |
| (Visão) Handover | Emitir e verificar | Detecção em < 1 ms |
| (Visão) Temperatura | Medir núcleo | 77 K ± 0.5 K |

---

## 3. Lista de Ferramentas

- Impressora 3D FDM (nozzle 0.4 mm) + ventoinha de peça/ponte.
- Paquímetro digital (leitura 0.01 mm).
- Chave de torque 2 N·m (produção).
- Multímetro digital (visão).
- Osciloscópio ≥ 1 GHz (visão).
- Termômetro infravermelho (visão).
- Fonte de alimentação 5 V / 2 A (visão).

---

## 4. Controle de Qualidade / Inspeção

- Inspeção visual de cada lote.
- Amostragem dimensional conforme tolerâncias do PSD.
- Organizar registros de calibração da impressora (flatness + offset Z).

---

## 5. Histórico de Processo

| Data | Etapa | Responsável | Status |
|------|-------|-------------|--------|
| 2026-08-30 | Correção do G-code (análise estática) | Catedral OS | ✅ |
| — | Impressão de validação | *a definir* | ⏳ |
| — | Teste dimensional | *a definir* | ⏳ |
