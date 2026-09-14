# arkhe-allo

A real, compiling, tested Foundry implementation of the Allo.Capital capital-allocation
suite. Built to replace a series of pasted "production-ready" contracts that did not
compile and re-introduced their own critical bugs. Every fix here is backed by a passing
test **and** was checked against a throwaway demo proving the same assertion fails on the
buggy design — so the fixes are load-bearing, not cosmetic.

## Quick start

Self-contained: OpenZeppelin 5.x is vendored under `lib/`, and the tests use a minimal
inline cheatcode interface instead of `forge-std`, so there is **nothing to install**.

```bash
forge build
forge test            # 37 tests
forge test -vvv       # with traces
forge fmt             # format
forge snapshot        # gas snapshot -> .gas-snapshot
forge script script/DeployAll.s.sol   # simulate deploying + wiring the whole suite
```

Requires Foundry (`forge`). If `forge` isn't on PATH it may be at `~/.foundry/bin/forge`.

## Layout

```
arkhe-allo/
├── src/
│   ├── AlloPool.sol                    capital pool w/ available-balance accounting
│   ├── QuadraticFundingStrategy.sol    real QF: (Σ√c)² − Σc
│   ├── FutarchyStrategy.sol            CPMM prediction market + share redemption
│   ├── FutarchyGovernor.sol            market verdict -> pool funding (the futarchy loop)
│   ├── DirectToContractIncentives.sol  oracle-signature-gated rewards
│   ├── CookieJar.sol                   grants: claim vs approval decoupled
│   ├── DedicatedDomainAllocation.sol   per-domain steward budgets
│   └── RetroFundingStrategy.sol        score-proportional retro payouts
├── test/                               one *.t.sol per contract + PoolInvariant fuzz
├── script/DeployAll.s.sol              deploy + role wiring (one pool per strategy)
├── lib/@openzeppelin/                  vendored OZ 5.x (self-contained)
└── foundry.toml                        solc 0.8.24, relative OZ remapping
```

## What each contract fixes (original audit → fix → proof)

| Contract | Original critical | Fix | Proven by |
|---|---|---|---|
| AlloPool | double-spend accounting | `availableBalance()` reserves against unlocked balance; `distribute` capped at `totalAllocated` | `PoolInvariant` fuzz: `distributed ≤ allocated ≤ deposits` |
| QuadraticFundingStrategy | fake linear "QF" | genuine `(Σ√c)² − Σc`; funds actually pooled + distributed | same 4 ETH total, more contributors ⇒ ~75 vs ~25 |
| FutarchyStrategy | 1:1 exchange, no price | constant-product AMM + 1:1 share redemption | 2nd identical buy yields fewer shares; dust buy can't flip |
| FutarchyGovernor | market decided nothing | YES funds beneficiary from a *separate* pool; trader collateral untouched | redemption & funding independence test |
| DirectToContractIncentives | open `claimReward` drain | oracle EIP-191 signature + per-user nonce + cap | unsigned/forged claim reverts |
| CookieJar | unapproved claim locked the pool | claim only records; pool touched only on approval | pending grant leaves `availableBalance` full |
| DedicatedDomainAllocation | funds stuck (never forwarded) | `fundDomain` forwards to pool; per-domain budget cap | budget isolation across domains |
| RetroFundingStrategy | evaluators hard-capped at 3 | dynamic, unbounded evaluator set | 5 evaluators all score |

## Notes / non-goals

- Each strategy uses its **own** `AlloPool` (a pool's `STRATEGY_ROLE` is single-holder).
- Not audited; no upgradeability/pausing; oracle key management is out of scope.
- The `DeployAll` oracle defaults to the deployer — set a real oracle for Incentives.
