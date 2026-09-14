# 🏛️ Análise Constitucional — Catedral OS v11.1 → v11.2

## 📋 Resumo Executivo

A v11.1 eliminou stubs com sucesso, mas introduziu uma **inconsistência arquitetural crítica** na camada de autenticação: o Prolog usa HMAC-SHA256 com formato custom, enquanto o Python usa RS256 (RSA-4096) no padrão RFC 7519. Isso quebra a interoperabilidade entre as camadas.

A v11.2 corrige isso via **JWT Dual-Mode**:
- **RS256** para API externa (Python → Cliente)
- **HS256** para comunicação interna Prolog (formato RFC 7519)
- **Revogação** habilitada (I34)

---

## 🔴 Críticos (v11.1)

| Problema | Impacto | Correção v11.2 |
|----------|---------|----------------|
| JWT Prolog formato custom | Não interoperável com Python | RFC 7519 base64url |
| JWT Prolog sem exp/iat | Tokens eternos — viola I34 | exp=1h (RS256), 30min (HS256) |
| JWT Prolog sem header | Vulnerável a alg confusion | Header com alg:HS256, typ:JWT |
| JWT Python RS256 vs Prolog HMAC | Quebra de integração | Dual-Mode documentado |
| Chave HMAC hardcoded | Risco de segurança | `CATHEDRAL_HMAC_SECRET` env var |

## 🟡 Atenção (v11.1)

| Problema | Recomendação |
|----------|--------------|
| Alpha calculation heurístico | Manter como stub evolutivo; formalizar em v12 |
| Jailbreak detection por substring | Adicionar regex + embeddings em v12 |
| PySWIP dependência | Documentar fallback quando indisponível |

## ✅ Aceitos (v11.1 → v11.2 sem alterações)

- Luhn ISO 7812-1 nativo em Prolog
- SSL/TLS RSA-4096 com TLS 1.2+
- CORS allowlist explícita
- Error handling nos endpoints POST
- CT Logs com registro real
- Vault fallback

---

## 🏗️ Arquitetura JWT Dual-Mode v11.2

```
┌─────────────────────────────────────────────────────────────────┐
│                    JWT DUAL-MODE v11.2                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  CLIENTE EXTERNO                                                │
│  ├── Recebe token RS256 (RSA-4096, exp=1h)                    │
│  └── Envia: Authorization: Bearer <RS256>                      │
│                                                                 │
│  PYTHON ORQUESTRADOR                                            │
│  ├── Verifica RS256 com chave pública                          │
│  ├── Gera HS256 para Prolog (HMAC, exp=30min)                 │
│  └── Revogação: blacklist global (I34)                        │
│                                                                 │
│  PROLOG (agi_core_v112.pl)                                     │
│  ├── Verifica HS256 interno (HMAC-SHA256, RFC 7519)           │
│  ├── Pode verificar RS256 via rsa_verify/4 (chave pública)    │
│  └── Revogação: jwt_blacklist/1 dinâmico                      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📁 Arquivos Gerados

| Arquivo | Descrição |
|---------|-----------|
| `agi_core_v112.pl` | Núcleo Prolog com JWT HMAC RFC 7519 + RS256 verify |
| `cathedral_orchestrator_v112.py` | Orquestrador Python com JWT Dual-Mode + revogação |
| `test_sub212_v112.py` | Testes pytest para ambos os modos JWT |

---

*Ex Auditu, Veritas. Ex Veritate, Soverenitas.*
