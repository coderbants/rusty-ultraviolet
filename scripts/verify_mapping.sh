#!/usr/bin/env bash
# Verifies that UPSTREAM_MAPPING.md accounts for every file in upstream-go/.
set -u

cd "$(dirname "$0")/.."
MAPPING=UPSTREAM_MAPPING.md
fail=0

while IFS= read -r f; do
  case "$f" in
    *.go)
      if ! grep -qF "$f" "$MAPPING"; then
        # Examples may be mapped by directory without the main.go suffix.
        if [[ "$f" == examples/*/main.go ]]; then
          dir="${f%/main.go}"
          if ! grep -qF "$dir" "$MAPPING"; then
            echo "MISSING (.go): $f"
            fail=1
          fi
        else
          echo "MISSING (.go): $f"
          fail=1
        fi
      fi
      ;;
    *)
      base="$(basename "$f")"
      dir="${f%/*}"
      if ! grep -qF "$base" "$MAPPING" && ! grep -qF "$dir/" "$MAPPING"; then
        echo "MISSING (support): $f"
        fail=1
      fi
      ;;
  esac
done < <(cd upstream-go && git ls-files)

if [ "$fail" -eq 0 ]; then
  echo "OK: every upstream file is accounted for in $MAPPING"
fi
exit "$fail"
