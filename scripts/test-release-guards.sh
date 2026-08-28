#!/usr/bin/env bash

# Regression checks for the release and CI trust boundaries. These checks are
# intentionally static and fast so every gate can prove that workflow changes
# did not reintroduce mutable dependencies or accidental write access. Keep
# the implementation on GitHub-hosted runner core tools; ripgrep is not
# guaranteed to be installed there.

set -euo pipefail

cd "$(dirname "$0")/.."

fail=0

report() {
  printf 'ERROR: %s\n' "$1" >&2
  fail=1
}

check_pinned_action() {
  local action="$1"
  local line
  local ref

  while IFS= read -r line; do
    ref="${line##*@}"
    if [[ ! "${ref}" =~ ^[0-9a-f]{40}$ ]]; then
      report "${action} must use a full immutable commit SHA: ${line}"
    fi
  done < <(grep -nE "uses: ${action}@" .github/workflows/*.yml)
}

check_pinned_action "actions/checkout"
check_pinned_action "actions/setup-go"
check_pinned_action "taiki-e/install-action"

if grep -n 'workflow_dispatch' .github/workflows/publish.yml >/dev/null; then
  report "publish workflow must not expose a manual dispatch path"
fi

if awk '
  /repository: coderbants\/rusty-/ { sibling=1; next }
  sibling && /ref: dev/ { bad=1 }
  sibling && /^      - name:/ { sibling=0 }
  END { exit bad ? 0 : 1 }
' .github/workflows/ci.yml .github/workflows/publish.yml; then
  report "sibling dependency checkouts must use immutable commit refs"
fi

coverage_job="$(awk '
  /^  coverage:/ { in_coverage=1 }
  in_coverage && /^  [A-Za-z0-9_-]+:/ && $0 !~ /^  coverage:/ { exit }
  in_coverage { print }
' .github/workflows/ci.yml)"

case "${coverage_job}" in
  *"contents: write"*)
    report "coverage must not have repository write permission while running tests"
    ;;
esac

if ! grep -n 'needs: coverage' .github/workflows/ci.yml >/dev/null; then
  report "coverage badge publication must depend on the read-only coverage job"
fi

if ! grep -nE 'uses: actions/(upload|download)-artifact@[0-9a-f]{40}' .github/workflows/ci.yml >/dev/null; then
  report "coverage must exchange its report through immutable artifact actions"
fi

if grep -nE 'x-access-token:|git (remote set-url|push).*(GH_TOKEN|\$\{GH_TOKEN\})|cargo publish.*--token' .github/workflows/ci.yml .github/workflows/publish.yml >/dev/null; then
  report "workflow credentials must not be embedded in URLs or command-line arguments"
fi

if ! grep -n 'gh api --method PUT' .github/workflows/ci.yml >/dev/null; then
  report "coverage badge updates must use the GitHub API credential channel"
fi

if ! scripts/verify_upstream_version.sh >/dev/null; then
  report "the tracked upstream version must pass the release-version guard"
fi

if scripts/verify_upstream_version.sh not-a-release-tag >/dev/null 2>&1; then
  report "the release-version guard must reject non-v tags"
fi

if [ "${fail}" -ne 0 ]; then
  exit 1
fi

echo "OK: release and CI trust-boundary guards pass"
