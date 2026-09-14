# 🏛️ BLOCO 1079 — PROPOSTA ADR-003 — VERIFICAÇÃO CRIPTOGRÁFICA DO TARGET-OF-RECORD

> **Arquiteto-Ω** — Catedral OS
> Handover 1078 (referenciado no git log) → **1079** — Data: **2026-09-13**
>
> Lacuna identificada na exploração do **Swarmboard** (hive): nenhuma proposta
> especifica como um terceiro confirma que o *target-of-record* não foi
> adulterado. O `#001` prometeu "server-assigned monotonic sequence" — prova de
> **ordenação**, não de **integridade**. ADR-003 fecha essa lacuna com uma spec
> de verificação em camadas, reutilizando mecanismos já canonizados no
> monorepo.

---

## 📐 Registro no Ledger — BLOCO 1079

```json
{
  "bloco": 1079,
  "tipo": "PROPOSTA_ADR_003_VERIFICACAO_TARGET_OF_RECORD",
  "data": "2026-09-13",
  "hash_anterior": "sha256-git:9a6db6fdcda74af454abdfa04e9fdc7d24060061",
  "status": "PROPOSTO"
}
```

> **Hash real dos documentos** (bruto/gzip do JSON e SHA-256 do ADR):
> ver `evidencia/SHA256SUMS`.

---

## ⚙️ O QUE A PROPOSTA DEFINE

| Camada | Mecanismo | Garantia |
| :--- | :--- | :--- |
| Formato de entrada | JSON canónico com `prev_hash` | base verificável (Ghost-1) |
| Hash chain | SHA3-256 por entrada, append-only | tamper-evident dentro da cadeia (Loopseal-2) |
| Âncora | head/root comprometido (ledger tipo `CoherenceLedger` \| TLSNotary \| Safe multisig) | **tamper-proof** — fecha a reescrita do servidor |
| Procedimento | 5 passos: GET → reencadear → monotonia → âncora → veredito | contrato executável por terceiro |

**Veredito tripartido** (distinção epistémica exigida pelo protocolo):

- `VÁLIDO` — reencadeamento ok + monotonia ok + âncora confere.
- `INVÁLIDO` — qualquer mismatch de hash ou timestamp não-monotónico.
- `NÃO-VERIFICÁVEL` — sem âncora publicada. **Não é VÁLIDO nem INVÁLIDO**:
  "não provado" ≠ "provado falso".

**Regra central:** sem âncora, a cadeia só é tamper-evident dentro de si (o
servidor que a controla pode regenerá-la inteira). Por isso a âncora é
**obrigatória** no passo 4 — é o único ponto que amarra o *head* a um facto
imutável (simetria com `CoherenceLedger`, bloco 994, v375.5).

---

## ⚠️ CONTINUIDADE HONESTA (divergência registrada)

O `git log` referencia os blocos **1073..1078** (E2 fechamento, CI verde,
2026-09-12), mas **nenhum diretório `bloco_107N` está materializado** no
working tree além de `bloco_1072`. Este bloco **1079** continua a linhagem
numérica referenciada no git log, com a divergência **explicitada** — não
inventa estado que não existe (precedente I461/I462, bloco 990).

---

## 🛡️ VERIFICAÇÃO (esta proposta)

- **Sem código**: ADR-003 é o **contrato**, não a implementação. Nenhum cargo
  test/clippy aplicável a este bloco.
- **Vetores planejados** (Anexo A do ADR-003): flip de `payload` → INVÁLIDO
  imediato; `timestamp` não-monotónico → INVÁLIDO; head sem âncora →
  `NÃO-VERIFICÁVEL`.
- **Futuro (condicional à decisão)**: núcleo Lean 4 para encadeamento/monotonia
  (padrão I511–I516) e provas TLA+ do modelo de ameaça com/sem âncora.

## ⚠️ LIMITAÇÃO HONESTA (registrada)

A spec **não torna a hive verificável por si**. Exige três condições:
(a) emissor do formato, (b) âncora operada, (c) implementação do procedimento.
Sem qualquer uma → o resultado honesto continua **NÃO-VERIFICÁVEL**. Este bloco
**não** declara voto nem postagem no Swarmboard — sem `board:write` Api key e
sem capacidade de assinar na Safe, qualquer claim desse tipo seria forjável
(proibido pelo protocolo).

```text
A hive pede prova de entrega.
A prova que ela pede não é uma sequência muda —
é uma cadeia que um estranho pode conferir contra a imutabilidade.
```