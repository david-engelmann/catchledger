# Catchledger Operator Guide

Catchledger is an offline, zero-dependency Rust CLI and library that computes a conservative per-check scorecard from local CI workflow-run history and Git commit history. It attributes true catches from what happened after each failure, separates bare-retry flakes, prices compute and optional LLM token spend, and emits validated `catchledger-pack/1` files for `tameshi`.

## Installation

### Build from Git with Cargo

Requires Rust `1.70` or newer on Linux or macOS.

```sh
cargo install --git https://github.com/david-engelmann/catchledger --locked
```

### Prebuilt Release Archives

On Linux and macOS (`x86_64` and `aarch64`), install verified prebuilt archives via `install.sh` or `cargo-binstall` once release assets are attached (`v0.37.0` and `v0.38.0` onward).

```sh
curl -fsSL https://raw.githubusercontent.com/david-engelmann/catchledger/main/install.sh | sh
export PATH="$HOME/.local/bin:$PATH"
```

```sh
cargo binstall --git https://github.com/david-engelmann/catchledger catchledger
```

## CLI Flags and Verification Modes

Catchledger runs either in **write mode** (generating `--output`, `--report`, `--packs-dir`, and optional `--token-output`) or in **read-only check mode** (supplying exactly one of `--check-ledger`, `--check-report`, `--check-packs`, or `--check-token-cost` without `--output`, `--report`, `--packs-dir`, or `--token-output`).

```text
# Write mode
catchledger \
  --ci-history PATH \
  (--git-log PATH | --git-repo PATH) \
  [--canaryfield-results PATH] \
  --output PATH \
  --report PATH \
  --packs-dir DIR \
  [--repo-label TEXT] \
  [--cost-per-minute TEXT] \
  [--token-usage PATH --token-output PATH] \
  [--price-per-input-token TEXT] \
  [--price-per-output-token TEXT]

# Read-only verification modes (mutually exclusive, write nothing)
catchledger --ci-history PATH (--git-log PATH | --git-repo PATH) [--canaryfield-results PATH] --check-ledger PATH
catchledger --ci-history PATH (--git-log PATH | --git-repo PATH) [--canaryfield-results PATH] --check-report PATH
catchledger --ci-history PATH (--git-log PATH | --git-repo PATH) [--canaryfield-results PATH] --check-packs DIR
catchledger --ci-history PATH (--git-log PATH | --git-repo PATH) --token-usage PATH --check-token-cost PATH
```

Every successful run exits `0` in silence. Usage errors, malformed JSON, duplicate object keys, or check-mode mismatches exit `2` with a diagnostic message on standard error.

## Quickstart Over Synthetic Examples

Run `catchledger` against the synthetic fixtures under `examples/`.

```sh
mkdir -p /tmp/cl-demo
catchledger \
  --ci-history examples/synthetic-runs.json \
  --git-log examples/synthetic-git-log.txt \
  --canaryfield-results examples/synthetic-canaryfield-results.json \
  --output /tmp/cl-demo/ledger.json \
  --report /tmp/cl-demo/report.md \
  --packs-dir /tmp/cl-demo/packs
```

Verify each generated artifact in read-only check mode without writing new files.

```sh
catchledger \
  --ci-history examples/synthetic-runs.json \
  --git-log examples/synthetic-git-log.txt \
  --canaryfield-results examples/synthetic-canaryfield-results.json \
  --check-ledger /tmp/cl-demo/ledger.json

catchledger \
  --ci-history examples/synthetic-runs.json \
  --git-log examples/synthetic-git-log.txt \
  --canaryfield-results examples/synthetic-canaryfield-results.json \
  --check-report /tmp/cl-demo/report.md

catchledger \
  --ci-history examples/synthetic-runs.json \
  --git-log examples/synthetic-git-log.txt \
  --canaryfield-results examples/synthetic-canaryfield-results.json \
  --check-packs /tmp/cl-demo/packs
```

## Exporting Real GitHub Actions and Git History

Catchledger never calls the GitHub API over the network. Export workflow runs with `gh api --paginate` and `jq -s`, export Git commit history with `git log` matching `--git-repo`'s committer-date (`%cI`) and rename-detection (`-M`) format (passing `--no-show-signature` so signed commits never break stanza parsing), and pass `--repo-label "$REPO"` explicitly because GitHub's REST response nests `repository.full_name` inside each run object rather than at the top-level `repo` key.

```sh
(
  set -eu
  REPO="owner/repo"
  CLONE_DIR="."

  gh api --paginate "repos/$REPO/actions/runs?per_page=100" \
    | jq -s --arg repo "$REPO" '{repo: $repo, workflow_runs: map(.workflow_runs[])}' \
    > runs.json

  git -C "$CLONE_DIR" --no-pager log --reverse --topo-order --no-show-signature -M \
    --pretty='format:commit %H%nparents %P%ndate %cI%nsubject %s' \
    --name-status > git-log.txt

  catchledger \
    --ci-history runs.json \
    --git-log git-log.txt \
    --repo-label "$REPO" \
    --cost-per-minute 0.008 \
    --output ledger.json \
    --report report.md \
    --packs-dir packs
)
```

You can also pass `--git-repo "$CLONE_DIR"` instead of `--git-log git-log.txt` when running against a local repository without GPG commit-signature display enabled (`log.showSignature=false`) and outside a Git hook (`env -u GIT_DIR -u GIT_WORK_TREE`).

## Bridging `flighttape` Agent Runs to Token Spend

When CI checks invoke AI review or verification agents recorded with `flighttape` (`format = "1.0"`), aggregate per-run token counts into `catchledger-token-usage/1` (`format` and unique `runs` entries keyed by `run`) with `jq`, then pass `--token-usage` and `--token-output`.

```sh
jq '{
  format: "catchledger-token-usage/1",
  runs: (
    [ .events[] | select(.usage != null and .metadata.run_id != null) ]
    | group_by(.metadata.run_id | tostring)
    | map({
        run: (.[0].metadata.run_id | tostring),
        input_tokens: (map(.usage.input_tokens // 0) | add),
        output_tokens: (map(.usage.output_tokens // 0) | add)
      })
  )
}' tape.json > token-usage.json

catchledger \
  --ci-history runs.json \
  --git-log git-log.txt \
  --repo-label "owner/repo" \
  --token-usage token-usage.json \
  --token-output token-cost.json \
  --price-per-input-token 0.000003 \
  --price-per-output-token 0.000015 \
  --output ledger.json \
  --report report.md \
  --packs-dir packs
```

## Feeding Evidence Packs Into `tameshi`

Catchledger numbers per-check files in `--packs-dir` in sorted check order (`001-style-checks.json`, `002-unit-checks.json`). Hand either `ledger.json` or an individual pack file directly to `tameshi assemble --catchledger`.

```sh
tameshi assemble \
  --catchledger ledger.json \
  --canaryfield canaryfield-results.json \
  --packs-dir tameshi-packs
```
