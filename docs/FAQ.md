# Catchledger FAQ

## How does Catchledger decide whether a CI failure was a true catch?

Catchledger uses a deliberately conservative three-way attribution rule over the Git commit DAG for every run with `conclusion == "failure"`.

1. **`false_block`** If the same check later succeeds on the *exact same* `head_sha` (a bare retry without changing code), the block is recorded as a `false_block` against the check.
2. **`catch`** If no bare retry succeeded on that SHA, and a descendant commit modifies at least one path touched by the blocked commit (`flagged_paths`) followed by a green run of the same check on that descendant, the block counts as a true `catch`.
3. **`unattributed`** Every other block (for example an abandoned branch or a change without a validating green descendant run in the window) stays `unattributed` rather than inflating either catches or false blocks.

## Why did my repository name read `"unknown"` on a raw `gh api` export?

GitHub's `/repos/{owner}/{repo}/actions/runs` endpoint places `repository.full_name` inside each element of `workflow_runs[]` rather than emitting a top-level `"repo"` string, so Catchledger defaults the label to `"unknown"` unless told otherwise. Either inject `repo` during `jq -s` aggregation (`jq -s --arg repo "$REPO" '{repo: $repo, workflow_runs: map(.workflow_runs[])}'`) or pass `--repo-label "$REPO"` on the `catchledger` command line.

## Why should I pass `--no-show-signature` when generating `git-log.txt`?

Developers with `log.showSignature = true` in `~/.gitconfig` cause `git log` to inject `gpg:` verification lines before commit headers, which Catchledger's strict stanza parser rejects as malformed headers. Always include `--no-show-signature` in `git log` export commands.

## What happens when a check has zero true catches (`catches == 0`)?

`total_cost_usd` still records the full compute spend of all runs of that check, while `cost_per_catch_usd` is emitted as `null` in JSON and rendered as `not available` in Markdown so division by zero never occurs and zero-catch checks stand out clearly during `tameshi` triage.

## How do `--check-ledger`, `--check-report`, `--check-packs`, and `--check-token-cost` work?

Each `--check-*` flag runs Catchledger in a standalone read-only verification mode (mutually exclusive with `--output`, `--report`, `--packs-dir`, `--token-output`, and the other `--check-*` flags). Catchledger recomputes the expected output in memory without writing to disk, checks exact byte identity for `--check-report` and parsed JSON value equality for `--check-ledger`, `--check-packs`, and `--check-token-cost`, and exits `2` on any drift.
