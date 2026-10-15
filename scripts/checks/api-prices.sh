#!/usr/bin/env bash
# API price watch: compares the model table of Anthropic's pricing page with
# scripts/checks/api-prices.txt, the snapshot the price table in
# src/stats/price.rs is tested against (no specification governs this: our
# own design). A changed price or a new model shows as a difference; the
# snapshot and the Rust table are then updated together, and the unit tests
# in src/stats/price.rs check that they agree.
#
# Usage:
#   scripts/checks/api-prices.sh              compare the live page with the
#                                            snapshot; print the difference
#   scripts/checks/api-prices.sh --print      print the live table as snapshot
#                                            lines
#   scripts/checks/api-prices.sh --self-test  prove the parser on a fixture
# Each snapshot line is: name|input|5m write|1h write|cache read|output, in
# dollars per million tokens.
# Exit 1 when the page differs from the snapshot, 2 when the page cannot be
# fetched or holds no model table; 0 otherwise.
set -euo pipefail
cd "$(dirname "$0")/../.."

URL="https://platform.claude.com/docs/en/about-claude/pricing.md"
SNAPSHOT=scripts/checks/api-prices.txt

# table: reads the pricing page as Markdown on stdin and prints the rows of
# its "Model pricing" table as snapshot lines.
table() {
  awk '
    /^## / { inside = ($0 ~ /^## Model pricing/) ; next }
    inside && /^\| *Claude / {
      line = $0
      gsub(/<sup>[^<]*<\/sup>/, "", line)
      while (match(line, /\[[^]]*\]\([^)]*\)/)) {
        link = substr(line, RSTART, RLENGTH)
        text = link
        sub(/^\[/, "", text)
        sub(/\]\(.*$/, "", text)
        line = substr(line, 1, RSTART - 1) text substr(line, RSTART + RLENGTH)
      }
      n = split(line, cell, "|")
      out = ""
      for (i = 2; i < n; i++) {
        c = cell[i]
        gsub(/\$|\/ *MTok/, "", c)
        gsub(/^ +| +$/, "", c)
        out = out (i == 2 ? "" : "|") c
      }
      print out
    }
  '
}

self_test() {
  local fixture expected got
  # shellcheck disable=SC2016 # the fixture holds literal dollar prices
  fixture='# Pricing

## Model pricing

| Model | Base input tokens | 5m cache writes | 1h cache writes | Cache hits and refreshes | Output tokens |
| :---- | :---- | :---- | :---- | :---- | :---- |
| Claude Opus 5.5 | $4 / MTok | $5 / MTok | $8 / MTok | $0.20 / MTok<sup>2</sup> | $20 / MTok |
| Claude Opus 4 ([retired, except on Google Cloud](https://example.invalid/x)) | $15 / MTok | $18.75 / MTok | $30 / MTok | $1.50 / MTok | $75 / MTok |

## Batch processing

| Claude Opus 5.5 | $2 / MTok | $1 / MTok |'
  expected='Claude Opus 5.5|4|5|8|0.20|20
Claude Opus 4 (retired, except on Google Cloud)|15|18.75|30|1.50|75'
  got="$(printf '%s\n' "$fixture" | table)"
  if [ "$got" != "$expected" ]; then
    printf 'self-test FAILED\nexpected:\n%s\ngot:\n%s\n' "$expected" "$got" >&2
    exit 1
  fi
  printf 'self-test passed\n'
}

live() {
  local page rows
  if ! page="$(curl -fsSL --max-time 30 "$URL")"; then
    printf 'cannot fetch %s\n' "$URL" >&2
    exit 2
  fi
  rows="$(printf '%s\n' "$page" | table)"
  if [ -z "$rows" ]; then
    printf 'no model table found on %s\n' "$URL" >&2
    exit 2
  fi
  printf '%s\n' "$rows"
}

case "${1:-}" in
  --self-test) self_test ;;
  --print) live ;;
  "")
    current="$(live)"
    if ! diff -u "$SNAPSHOT" <(printf '%s\n' "$current"); then
      printf '\nThe pricing page differs from %s.\n' "$SNAPSHOT"
      exit 1
    fi
    printf 'The pricing page matches %s.\n' "$SNAPSHOT"
    ;;
  *)
    printf 'usage: %s [--print | --self-test]\n' "$0" >&2
    exit 2
    ;;
esac
