# Orbe — Manual do Usuário
## Estrutura de Bobinas de Helmholtz (Dipolo de Arkhe)

**Modelo:** ORB-01 · **Versão:** v1.0 · **Data:** 2026-08-30

---

## 1. Introdução

Bem-vindo ao **Orbe**, uma estrutura de bobinas de Helmholtz impressa em 3D (Substrato 156 — Dipolo de Arkhe). Este manual orienta a montagem, a inspeção e o uso seguro da estrutura física.

> **Escopo deste manual:** a estrutura física v1.0 (mecânica). A camada eletrônica/de processamento coerente é conceito teórico em desenvolvimento e não está coberta pela v1.0.

### 1.1. O que está incluso

- 1 × Estrutura Orbe impressa (2 anéis coaxiais + 4 pontes).
- 1 × Arquivo de impressão `Orbe_Helmholtz_100mm_corrigido.gcode`.
- 1 × Guia de inspeção (este manual).

---

## 2. Especificações Técnicas

| Parâmetro | Valor |
|-----------|-------|
| Diâmetro externo | 100 mm |
| Raio interno | ~39.3 mm |
| Altura por anel | 5 mm |
| Material | PLA (protótipo) / PEEK (produção 77K) |
| Massa | ~150 g |

---

## 3. Montagem e Instalação

### 3.1. Requisitos

- Superfície plana e estável.
- Local seco e limpo.
- (Protótipo) Temperatura ambiente 10–35 °C.

### 3.2. Passo a Passo

1. Confirme que a estrutura foi impressa com os parâmetros do `Manufacturing_Files.md`.
2. Verifique que os dois anéis estão coaxiais (alinhados).
3. Inspecione as quatro abas de ponte.
4. Posicione a estrutura sobre a superfície de trabalho.
5. Se aplicável, fixe nos pontos de montagem (M6).

---

## 4. Inspeção de Qualidade

### 4.1. Dimensões

- Diâmetro externo deve medir **100 ±0.5 mm**.
- Raio interno ≈ **39.3 mm**.

### 4.2. Integridade

- Sem trincas visíveis nas pontes.
- Sem delaminação das camadas.
- Sem stringing excessivo.

---

## 5. Segurança

⚠️ **AVISO (produção criogênica):** quando operado com PEEK em 77 K, a superfície pode estar extremamente fria. Use equipamento de proteção adequado.

⚠️ **PRECAUÇÃO:** não imprima/opere o protótipo PLA acima de ~40 °C de ambiente contínuo (o PLA deforma acima de ~60 °C).

🔧 **Manutenção (prototipagem FDM):** não utilize solventes agressivos para limpeza.

---

## 6. Solução de Problemas

| Problema | Solução |
|----------|---------|
| Diâmetro maior que 101 mm | Re-verificar escala (deve ser 50/56 centrada nos eixos) |
| Pontes fraturadas | Reimprimir com resfriamento ativo (M106 S255) |
| Desalinhamento dos anéis | Verificar mesa/nivelamento e eixos da impressora |
| Sub-extrusão no anel superior | Confirmar o G92 E0 inserido entre camadas (correção do script) |
| Stringing | Confirmar retração de 1.5 mm absoluta |

---

## 7. Descarte/Reciclagem

- O protótipo PLA é reciclável como plástico PLA.
- Componentes eletrônicos (quando adicionados) devem ser descartados conforme legislação local (e-lixo).
