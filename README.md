# catchledger

The scorecard every gate should carry, fires, blocks, true catches, false blocks, flake rate, runtime, and cost per catch, joined from CI logs to what happened next.

## What this is

Dashboards total runs and failures, and no tool anywhere computes which of a team's checks has caught anything lately. Catchledger ingests CI history and git history for one repo and builds one page per check, attributing true catches conservatively from what happened after each block. A block followed by a fix in the flagged code counts as a catch, and a block followed by a bare retry counts against the check. It is the ledger the other gate instruments write their evidence into and the verdict tools read from. The boundary with its sibling is one sentence. Catchledger prices checks, secondbill prices work.

## Status

Seeded in October 2026. The build has not started. Part of the impatient programming portfolio, in the Gate Truth arc.

## Licence

MIT. See LICENSE.
