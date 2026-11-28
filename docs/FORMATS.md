# Catchledger Wire Formats

Catchledger ingests CI workflow-run JSON, Git commit logs, optional `canaryfield` catch tables, and optional `catchledger-token-usage/1` files, and emits `catchledger-ledger/1`, `catchledger-pack/1`, `catchledger-token-cost/1`, and deterministic Markdown scorecards. All JSON readers refuse duplicate object keys at every depth and enforce a `4000` nesting bound.

## 1. CI Workflow-Run Input (`--ci-history`)

```json
{
  "repo": "synthetic-ledger-repo",
  "workflow_runs": [
    {
      "conclusion": "failure",
      "created_at": "2026-01-01T00:09:55Z",
      "head_sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "id": 101,
      "name": "unit checks",
      "run_attempt": 1,
      "run_started_at": "2026-01-01T00:10:00Z",
      "status": "completed",
      "updated_at": "2026-01-01T00:11:00Z"
    }
  ]
}
```

### Field Notes

- `repo` (or top-level `repository`) provides the default repository label unless `--repo-label TEXT` is passed on the command line (defaulting to `"unknown"` when neither is present).
- `workflow_runs` holds completed run objects with `id`, `name` (check identifier), 40-hex `head_sha`, `status`, `conclusion` (`"success"`, `"failure"`, `"timed_out"`, `"cancelled"`, etc.), `run_attempt`, `created_at`, `run_started_at`, and `updated_at`.

## 2. Git History Input (`--git-log`)

Plain-text commit stanzas separated by blank lines, carrying 40-hex commit and parent SHAs, ISO-8601 timestamps, subjects, and tab-separated `--name-status` file changes (including Git C-quoted paths, which Catchledger decodes to raw tree paths).

```text
commit aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
parents 
date 2026-01-01T00:00:00Z
subject Add synthetic math module
A	docs/guide.md
A	src/math.py
```

## 3. `catchledger-ledger/1` (`--output`)

Repository-wide ledger summarizing every observed check in lexicographical check order alongside SHA-256 digests of the input documents and history window bounds.

### Per-Check `scorecard` Fields

- `fires` counts total completed runs, and `blocks` counts runs whose `conclusion` is `"failure"`.
- `judged_commits`, `flaky_commits`, and `flake_rate` measure commits where multiple completed runs of the same check disagreed on the identical `head_sha`.
- `catches`, `false_blocks`, `unattributed_blocks`, and `false_block_rate` classify every `"failure"` block conservatively using the blocked commit's changed paths from Git history as `flagged_paths`.
- `runtime_seconds`, `runtime_minutes`, `total_cost_usd`, and `cost_per_catch_usd` (`null` when `catches == 0`) price compute using exact `DecimalValue` arithmetic at `--cost-per-minute` (default `"0.008"` USD/min).

## 4. `catchledger-pack/1` (`--packs-dir`)

Self-contained single-check evidence pack written as `<NNN>-<slug>.json` inside `--packs-dir` and consumed directly by `tameshi assemble --catchledger`. Carries `format` (`"catchledger-pack/1"`), `repo`, `check`, `scorecard`, `catch_record`, `canaryfield_results` (`null` when absent), `false_block_rate`, `history_window`, `pricing`, and `source` (input SHA-256 digests).

## 5. `catchledger-token-usage/1` and `catchledger-token-cost/1`

Optional LLM check spend input (`--token-usage`, holding `format` `"catchledger-token-usage/1"` and `runs` entries with unique `run` identifier string plus non-negative integer `input_tokens` and `output_tokens`) and per-check output (`--token-output`) pricing token totals at exact decimal per-token rates (`--price-per-input-token` and `--price-per-output-token`) and appending a token-cost section to the Markdown report.
