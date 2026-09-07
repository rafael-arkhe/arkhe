# Evidência — Experimentos E1–E4 (bloco 992)

Arquivo de prova empírica da Fase 3 (plano v375.2), preservado como
**evidência congelada** da decisão do Arquiteto (addendum bloco 992;
decisão formal no bloco 993).

**Fonte original:** `packages/arkhe-field-stability/mission_output/<eN>_results/`
(cópias invariantes neste diretório; integridade via `SHA256SUMS`, verificada
10/10).

| Artefato | Experimento | Conteúdo |
|:---|:---|:---|
| `e1_phi_series.csv` | E1 | série temporal Φ por janela (200 janelas × 50 amostras) |
| `e1_summary.json` | E1 | Φ médio 0.9837 (σ 0.0023), Pearson(Q,Φ) 0.9873, PASS |
| `e2_v2_deep_phi055.csv` | E2 | trajetória do refiner V2-deep (26 iterações, monotônica) |
| `e2_v2_canonical_phi070.csv` | E2 | trajetória do refiner V2-canonical (21 iterações, monotônica) |
| `e2_summary.json` | E2 | 0.5584→0.9515 e 0.6983→0.9581, PASS |
| `e3_scan_uniforme.csv` | E3 | varredura uniforme (passo 0.002), Gap-1 em d=0.424 |
| `e3_scan_enviesado.csv` | E3 | varredura enviesada, Gap-1 em d=0.564 |
| `e3_summary.json` | E3 | cruzamentos identificados, PASS |
| `e4_noise.csv` | E4 | Φ × jitter {0, 0.05, 0.10, 0.15, 0.20} |
| `e4_summary.json` | E4 | fronteira quieta ≤ 5%; robustez ≤ 15%; colapso a 20% (Φ 0.7214) |

**Ressalva de prova:** a reclassificação do E4 para `PASS CONDICIONAL`
(decisão do Arquiteto) não altera nenhum valor aqui congelado. O critério
estrito (1σ do sem-ruído) **falha para jitter ≥ 10%** — verificado e mantido
nos dados.

Verificação de integridade:

```
shasum -a 256 -c SHA256SUMS
```

**Selo:** `CATEDRAL-OS-EVIDENCIA-FASE3-E1E4-2026-09-06`