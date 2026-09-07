# Evidencia — Bloco 1004 — Verificacao factual da decisao v381.1

Data: 2026-09-07. Comandos via git/leitura de codigo no monorepo.

## 1. Formula canonica de Phi (fonte primaria: codigo)
packages/arkhe-field-stability/src/coherence.rs:26
  pub fn phi(components: [f64; 3], weights: [f64; 3]) -> f64 {
      let mut sum = 0.0;
      for (x, w) in components.iter().zip(weights.iter()) {
          sum += w * (1.0 - x).powi(2);
      }
      1.0 - sum.sqrt()
  }
=> QUADRATICA: Phi = 1 - sqrt( wO(1-O)^2 + wS(1-S)^2 + wL(1-L)^2 ), W=(0.4,0.4,0.2)
Arquivo que define a funcao: arkhe-monorepo/packages/arkhe-field-stability/src/coherence.rs

## 2. Significado canonico de O/S/L (docs/coherence_metric.md)
O = stability (estabilidade de campo, w=0.4)
S = success_rate (taxa de sucesso, w=0.4)
L = latency_score (score de latencia, w=0.2)
=> Medicao fisica, NAO Ontologica/Semantica/Temporal.

## 3. Crate citado na decisao (arkhe-coherence)
git ls-files -- 'arkhe-monorepo' | grep arkhe-coherence
Resultado: VAZIO - 0 tracks; crate INEXISTENTE
Crate real da coerencia: arkhe-field-stability (packages/arkhe-field-stability).

## 4. Bloco_1002 contem definicao de 4 componentes?
bloco_1002 descricao: execucao da Fase 6 (tautologia, IntegrityStatus, Lean I517-I523, CI).
=> NAO contem definicao de Phi de 4 componentes. Nao ha o que 'remover'.

## 5. Hash corrigido da decisao (7f83b165...) - reprodutibilidade
JSON comprimido do bloco_1003: sha256 cb4289962ed293e9d765b56f0ee2fb7ca83081102a503dcabf1c16bdb33171c2
JSON bruto do bloco_1003:        sha256 19e4d2607b21a3dadf47220aeb1483934b8e4729a861d4da61f1b18ca8ad2e85
Hash declarado na decisao:        7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069
=> Nao corresponde a nenhum blob gerado por mim; IRREPRODUZIVEL.
