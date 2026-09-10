/-
   AgentIdentityNucleus — Moda de identidade e delegacao (bloco 1074).
   Sobre substrato REAL: packages/arkhe-aid (AID-002 delegacao atenuada,
   AID-003 audit hash chain). Ancora normativa: IETF
   draft-nyantakyi-vaip-agent-identity-01 (Vorim Agent Identity Protocol).
   Nucleo Lean 4 core (SEM Mathlib, SEM sorry) — convencao do repositorio
   (blocos 966/971/972/996/1009/1073: kernel real; `by`/`rfl`/`simp`).

   HONESTIDADE: o Rust e a realizacao de sanidade; este nucleo prova a
   SEMANTICA DE SUBCONJUNTO da atenucao (AID-002) e a MONOTONIA APPEND-ONLY
   da cadeia de auditoria (AID-003) sobre modelos Lean das mesmas estruturas
   de Lista. Sem FFI; nenhuma afirmacao de extensionalidade automatica.
-/

namespace AgentIdentityNucleus

/- ==========================================================================
   AID-002 — ATENUATED DELEGATION (subset semantics)
   Escopos ja ordenados como tipos (Read < Write < Execute < Transact <
   Communicate < Delegate < Elevate); a atenuacao e a inclusao de lista.
   ========================================================================== -/

/-- Escopo de permissao (VAIP, sete escopos). -/
inductive Scope where
| read | write | execute | transact | communicate | delegate | elevate
deriving DecidableEq

open Scope

/-- Avencao: os escopos concedidos sao um subconjunto dos escopos detidos. -/
def attenuated (held granted : List Scope) : Prop :=
  ∀ s : Scope, s ∈ granted → s ∈ held

/- Teorema (reflexividade): toda delegacao de si para si mesma e atenuada. -/
theorem attenuated_reflexive (os : List Scope) :
    attenuated os os := by
  intro s h
  exact h

/- Teorema (transitividade — a cadeia nunca amplia):
   se C ⊆ B (delegacao B→C) e B ⊆ A (delegacao A→B), entao C ⊆ A.
   Este e o invarian atomo de "no-widening": mesmo composindo N saltos,
   o escopo nunca rebenta o do primeiro delegador. -/
theorem attenuated_transitive {a b c : List Scope}
    (h_ab : attenuated a b) (h_bc : attenuated b c) :
    attenuated a c := by
  intro s h_s_in_c
  exact h_ab s (h_bc s h_s_in_c)

/- Corolario: a composicao de duas delegacoes preserva a atenuacao do autor
   original (forma ponto-a-ponto usada pela DF no ledger). -/
theorem delegation_chain_no_widening {a b c : List Scope}
    (h_ab : attenuated a b) (h_bc : attenuated b c) :
    ∀ s : Scope, s ∈ c → s ∈ a :=
  attenuated_transitive h_ab h_bc

/- ==========================================================================
   AID-003 — PER-EVENT SIGNING AT SOURCE (append-only ledger)
   O ledger real assina cada evento na fonte e encadeia pelo hash do anterior
   (`prev_hash`); aqui provamos a propriedade LISTAR de append-only: anexar
   um evento NOVO nunca remove eventos ja registrados.
   ========================================================================== -/

/-- Evento de auditoria (modelo): acao + hash do contexto (opaco). -/
structure AuditEvent where
  action : String
  resource : String

/-- Anexar um evento ao ledger sem remover nada do que ja existia. -/
def append_event (ledger : List AuditEvent) (e : AuditEvent) : List AuditEvent :=
  ledger ++ [e]

/-- Monotonia: todo evento que ja estava no ledger continua presente apos
   o append (nunca perdemos um elo da cadeia — Ghost-1/Loopseal-2). -/
theorem append_keeps_entries (ledger : List AuditEvent) (e : AuditEvent) :
    ∀ (old : AuditEvent), old ∈ ledger → old ∈ append_event ledger e := by
  intro old h
  exact List.mem_append_left [e] h

/- Fecho transitivo: mesmo apos N appends, nenhum evento do prefixo e perdido. -/
theorem append_only_never_loses (ledger : List AuditEvent)
    (ext : List AuditEvent) :
    ∀ old : AuditEvent, old ∈ ledger → old ∈ ledger ++ ext := by
  intro old h
  -- Inducao na extensao.
  induction ext with
  | nil =>
      -- old ∈ ledger ++ [] reduz a old ∈ ledger.
      simpa [List.append_nil] using h
  | cons e rest ih =>
      -- Passo: converte o membro do append via IFF (evita eliminação
      -- dependente sobre indices nao-construtores) e reanexa pela esquerda
      -- ou pela direita (cons).
      have ih_or : old ∈ ledger ∨ old ∈ rest := List.mem_append.mp ih
      rcases ih_or with h_l | h_r
      · exact List.mem_append_left (e :: rest) h_l
      · exact List.mem_append_right ledger (List.mem_cons_of_mem e h_r)

end AgentIdentityNucleus