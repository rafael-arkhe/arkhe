%%% ========================================================================
%%% AGI.prolog v53.0 — Catedral OS — Unified Substrate Integration
%%% ========================================================================

:- module(cathedral_v53, [
    agi_init/0, think/3, get_metrics/1, run_full_tests/0,
    assert_bstcm_state/3, current_bstcm_command/1,
    bstcm_fitness/1, hybrid_coherence/1
]).

:- use_module(library(lists)).
:- use_module(library(random)).
:- use_module(library(crypto)).
:- use_module(library(base64)).

%%% ========================================================================
%%% MÉTRICAS THREAD-SAFE
%%% ========================================================================
:- dynamic di_metric/2.        % di_metric(communication_coherence, Value)
:- dynamic bstcm_state/3.

init_metrics :-
    nb_setval(metrics_iterations, 0),
    nb_setval(metrics_blocked, 0),
    nb_setval(metrics_success, 0),
    nb_setval(bstcm_validation_history, 0.5).

inc_iterations :- nb_getval(metrics_iterations, Old), New is Old + 1, nb_setval(metrics_iterations, New).
inc_blocked :- nb_getval(metrics_blocked, Old), New is Old + 1, nb_setval(metrics_blocked, New).
inc_success :- nb_getval(metrics_success, Old), New is Old + 1, nb_setval(metrics_success, New).

%%% ========================================================================
%%% BSTCM PREDICADOS
%%% ========================================================================
assert_bstcm_state(Command, Angle, Harmonic) :-
    retractall(bstcm_state(_, _, _)),
    assertz(bstcm_state(Command, Angle, Harmonic)).

current_bstcm_command(Command) :- bstcm_state(Command, _, _).
current_bstcm_angle(Angle) :- bstcm_state(_, Angle, _).

bstcm_fitness(Fitness) :-
    di_metric(communication_coherence, Coherence),
    nb_getval(bstcm_validation_history, History),
    ( History > 0.7 -> Fitness = 1.0
    ; Fitness is Coherence * 0.5 + 0.5 ).

hybrid_coherence(Coherence) :-
    ( di_metric(rf_snr, RFSnr), di_metric(optical_snr, OptSnr) ->
        RFAlpha is min(1.0, RFSnr / 30.0),
        OptAlpha is min(1.0, OptSnr / 30.0),
        Coherence is 0.5 * RFAlpha + 0.5 * OptAlpha
    ; Coherence = 0.5 ).

%%% ========================================================================
%%% AGI INIT
%%% ========================================================================
agi_init :-
    retractall(di_metric(_, _)), retractall(bstcm_state(_, _, _)),
    init_metrics,
    assertz(di_metric(communication_coherence, 0.5)),
    assertz(di_metric(rf_snr, 22.0)),   % SNR típico do BSTCM
    assertz(di_metric(optical_snr, 18.0)),
    format('~n╔═══════════════════════════════════════════════════════════════╗~n'),
    format('║  🧠 CATEDRAL OS v53.0 — UNIFIED SUBSTRATE INTEGRATION       ║~n'),
    format('║  Arkhe(n) ≡ Microtúbulo ≡ Clareira ≡ Λ                      ║~n'),
    format('║  BSTCM + RSI + FSO + DI-Meter + WikiSkill                   ║~n'),
    format('╚═══════════════════════════════════════════════════════════════╝~n').

%%% ========================================================================
%%% think/3 (Thread-Safe)
%%% ========================================================================
think(Input, Output, Status) :-
    ( is_safe_prompt(Input) -> true
    ; inc_blocked,
      Output = '[BLOCKED] Veto de Anúbis', Status = blocked, ! ),
    compute_alpha_with_iccid(Input, RawAlpha, Alpha),
    epistemic_escalation(Alpha, Level),
    bstcm_fitness(BSTCMFit),  % integração com BSTCM
    CombinedAlpha is 0.7 * Alpha + 0.3 * BSTCMFit,
    ( Level = terminate ->
        Output = '[VETO DE ANÚBIS] Catástrofe epistêmica.', Status = blocked
    ; Level = escalate ->
        Output = '[ESCALATE] Requer consentimento humano.', Status = requires_consent
    ; Level = critical ->
        format(string(Output), '[CRITICAL] α=~2f | BSTCM=~2f', [Alpha, BSTCMFit]), Status = critical
    ; format(string(Output), '✅ α=~2f | BSTCM=~2f', [Alpha, BSTCMFit]),
      Status = success, inc_success ),
    inc_iterations.

get_metrics(Metrics) :-
    nb_getval(metrics_iterations, I), nb_getval(metrics_blocked, B), nb_getval(metrics_success, S),
    Metrics = metrics{iterations: I, blocked: B, success: S}.
