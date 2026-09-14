# 🏛️ Catedral OS v8.7 — Pós-Eclipse

## 📜 Equação Fundamental

```
Arkhe(n) ≡ Microtúbulo ≡ Clareira ≡ Λ
```

## 🔧 Correções de Auditoria (v8.6 → v8.7)

| Bug | Correção | Status |
|-----|----------|--------|
| Veto em standby (α=0.96) | Veto ATIVA em α≥0.95 | ✅ Corrigido |
| Timestamp frágil (`=`) | Janela de 5 minutos | ✅ Corrigido |
| Mapeamento lunar→α | Documentado como narrativo | ✅ Corrigido |
| `random(Novelty)` | `shannon_entropy/2` determinístico | ✅ Corrigido |
| `verify_theorem/2` | `theorem_status/2` (honesto) | ✅ Corrigido |
| `has_contradiction` typo | `Sentences` (não `Sentices`) | ✅ Corrigido |

## 🚀 Execução

```bash
chmod +x run_cathedral.sh
./run_cathedral.sh
```

Acesse `http://localhost:8080` para ver a Tela Infinita.
