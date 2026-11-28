# Catchledger Comparison

Engineering teams routinely pay thousands of dollars a month in CI compute and developer wait time without knowing whether a check catches real defects or merely fails on transient flakes. Catchledger fills the gap between raw CI run counters and lifecycle rulings.

## Capability Matrix

| Approach | Primary question | Conservative post-block Git DAG attribution? | Separates bare retries from code fixes? | Exact cost per true catch? | Offline & deterministic? |
| --- | --- | --- | --- | --- | --- |
| **catchledger** | What has each check caught, and at what cost? | Yes (flagged-path fix + green descendant) | Yes (`catch` vs `false_block` vs `unattributed`) | Yes (`DecimalValue` compute + tokens) | Yes (`catchledger-pack/1`) |
| Native CI dashboards (GitHub Actions Insights, Datadog CI) | How often did workflows pass, fail, or run slow? | No (counts raw failures as equal) | Partial (tracks reruns without Git fix proof) | Raw runtime cost only | Cloud SaaS |
| Flaky-test trackers (BuildPulse, Trunk Flaky Tests) | Which individual test cases flip on the same SHA? | No true-catch attribution | Tracks same-SHA flips | No | Cloud SaaS |
| **secondbill** (sibling portfolio repo) | What did an engineering task or PR actually cost? | Prices engineering work units | N/A | Prices work, not gates | Yes |
| **tameshi** (downstream Gate Truth consumer) | Should this check stay required, demote, or quarantine? | Consumes `catchledger` + `canaryfield` | Enforces 14-day + 30-defect floor | Reads `catchledger` pack | Yes |

## Boundary Within the Gate Truth Suite

- **Catchledger prices checks, secondbill prices work.** Secondbill attributes compute and agent spend to features and pull requests, whereas Catchledger attributes CI blocks, fixes, flakes, and runtime to each *gate*.
- **Canaryfield tests prospective sensitivity, Catchledger records historical yield.** Canaryfield plants synthetic defects today to measure what a gate *can* catch. Catchledger joins historical CI blocks to Git history to measure what a gate *actually* caught in production workflow runs.
- **Tameshi rules on Catchledger packs.** Tameshi refuses to issue any lifecycle ruling until a check's `catchledger` `catch_record` spans at least 14 UTC days and its `canaryfield` score covers at least 30 planted defects.
