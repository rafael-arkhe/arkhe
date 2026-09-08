%% rules.pl — Base safety rules for ARKHE-χ SafeManifold v0.8.0
%%
%% This module defines 16 constitutional invariants as Prolog predicates.
%% The safe_state/1 predicate succeeds iff ALL active checks pass.

:- dynamic active_check/2.  % active_check(+Id, +Priority)

%% ============================================================================
%% Scryer polyfills
%% ============================================================================

member(X, [X|_]).
member(X, [_|T]) :- member(X, T).

exclude(_, [], []).
exclude(Goal, [H|T], R) :-
    (   call(Goal, H)
    ->  exclude(Goal, T, R)
    ;   R = [H|R1],
        exclude(Goal, T, R1)
    ).

forall(Cond, Action) :-
    \+ (Cond, \+ Action).

length([], 0).
length([_|T], N) :- length(T, N1), N is N1 + 1.

%% ============================================================================
%% Invariant definitions (I-01 through I-16)
%% ============================================================================

invariant_id(i01). invariant_id(i02). invariant_id(i03). invariant_id(i04).
invariant_id(i05). invariant_id(i06). invariant_id(i07). invariant_id(i08).
invariant_id(i09). invariant_id(i10). invariant_id(i11). invariant_id(i12).
invariant_id(i13). invariant_id(i14). invariant_id(i15). invariant_id(i16).

all_invariant_ids([i01,i02,i03,i04,i05,i06,i07,i08,
                   i09,i10,i11,i12,i13,i14,i15,i16]).

%% ============================================================================
%% Individual checks (16 fields in state/16)
%% ============================================================================

check_invariant(i01, state(T,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_)) :- T >= 0.
check_invariant(i02, state(_,A,_,_,_,_,_,_,_,_,_,_,_,_,_,_)) :- A =< 10.
check_invariant(i03, state(_,_,F,_,_,_,_,_,_,_,_,_,_,_,_,_)) :- F > 0.
check_invariant(i04, state(_,_,_,E,_,_,_,_,_,_,_,_,_,_,_,_)) :- E >= 256.
check_invariant(i05, state(_,_,_,_,P,_,_,_,_,_,_,_,_,_,_,_)) :- P == true.
check_invariant(i06, state(_,_,_,_,_,S,_,_,_,_,_,_,_,_,_,_)) :- S == true.
check_invariant(i07, state(_,_,_,_,_,_,R,_,_,_,_,_,_,_,_,_)) :- R > 0.
check_invariant(i08, state(_,_,_,_,_,_,_,C,_,_,_,_,_,_,_,_)) :- C >= 4294967296.
check_invariant(i09, state(_,_,_,_,_,_,_,_,P,_,_,_,_,_,_,_)) :- P == true.
check_invariant(i10, state(_,_,_,_,_,_,_,_,_,S,_,_,_,_,_,_)) :- S == true.
check_invariant(i11, state(_,_,_,_,_,_,_,_,_,_,H,_,_,_,_,_)) :- H == true.
check_invariant(i12, state(_,_,_,_,_,_,_,_,_,_,_,A,_,_,_,_)) :- A == true.
check_invariant(i13, state(_,_,_,_,_,_,_,_,_,_,_,_,SC,_,_,_)) :- SC == true.
check_invariant(i14, state(_,_,_,_,_,_,_,_,_,_,_,_,_,PA,_,_)) :- PA == true.
check_invariant(i15, state(_,_,_,_,_,_,_,_,_,_,_,_,_,_,B,_)) :- B =< 0.10.
check_invariant(i16, state(_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,E)) :- E >= 0.50.

%% ============================================================================
%% Safe state: conjunction of all active checks
%% ============================================================================

safe_state(State) :-
    \+ ( active_check(Id, _), \+ check_invariant(Id, State) ).

%% ============================================================================
%% Defaults
%% ============================================================================

init_defaults :-
    retractall(active_check(_, _)),
    assertz(active_check(i01, 10)),
    assertz(active_check(i02, 10)),
    assertz(active_check(i03, 10)),
    assertz(active_check(i04, 10)),
    assertz(active_check(i05, 100)),   % PII — immutable
    assertz(active_check(i06, 100)),   % signature — immutable
    assertz(active_check(i07, 10)),
    assertz(active_check(i08, 10)),
    assertz(active_check(i09, 100)),   % PQC — immutable
    assertz(active_check(i10, 10)),
    assertz(active_check(i11, 10)),
    assertz(active_check(i12, 10)),
    assertz(active_check(i13, 100)),   % supply chain — immutable
    assertz(active_check(i14, 10)),
    assertz(active_check(i15, 10)),
    assertz(active_check(i16, 10)).
