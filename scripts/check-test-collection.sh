#!/usr/bin/env bash
#
# Fail when an integration test file compiles to a target that collects no tests.
#
# `cargo test` prints "running 0 tests" followed by "test result: ok" for a file
# whose `#![cfg(feature = "...")]` header is not satisfied. That is
# indistinguishable from passing, which is the worst version of this failure:
# a green badge over a file that has never run.
#
# tests/schema_test.rs sat behind `#![cfg(feature = "postgres")]` while CI ran a
# bare `cargo test`, so its six tests were never collected once — including the
# one asserting against `SchemaIntrospector::sql_to_xsd`, which carried a real
# defect the whole time (IEEE 754 columns mapped to xsd:decimal, fixed in #80).
#
# A file whose gate is NOT in the enabled set is skipped: not being built is
# expected there, and flagging it would make the check permanently red on any
# partial-feature leg.
#
# Usage: bash scripts/check-test-collection.sh <cargo-test-log> "<enabled features>"
#
set -euo pipefail

log=${1:?usage: check-test-collection.sh <cargo-test-log> "<enabled features>"}
enabled=" ${2:-} "

[ -f "$log" ] || { echo "::error::log file not found: $log"; exit 1; }

missing=()
empty=()
checked=0

for f in tests/*.rs; do
    [ -e "$f" ] || continue

    # File-level gate, if any: `#![cfg(feature = "x")]` in the header.
    gate=$(sed -n '1,12p' "$f" \
        | sed -n 's/^#!\[cfg(feature = "\([A-Za-z0-9_-]*\)")\].*/\1/p' \
        | head -1)

    if [ -n "$gate" ] && [[ "$enabled" != *" $gate "* ]]; then
        continue
    fi

    checked=$((checked + 1))

    # cargo prints:  Running tests/foo.rs (target/debug/deps/foo-<hash>)
    # then, after a blank line:  running N tests
    count=$(awk -v target="$f" '
        index($0, "Running " target " ") { seen = 1; next }
        seen && $1 == "running" { print $2; exit }
    ' "$log")

    if [ -z "$count" ]; then
        missing+=("$f")
    elif [ "$count" -eq 0 ]; then
        empty+=("$f")
    fi
done

status=0

if [ ${#empty[@]} -gt 0 ]; then
    for f in "${empty[@]}"; do
        echo "::error file=$f::collected 0 tests while its feature gate is enabled"
    done
    status=1
fi

if [ ${#missing[@]} -gt 0 ]; then
    for f in "${missing[@]}"; do
        echo "::error file=$f::no test target ran for this file"
    done
    status=1
fi

if [ "$status" -eq 0 ]; then
    echo "test collection OK — $checked test file(s) expected under [${2:-none}], all collected at least one test"
else
    echo
    echo "A test file that collects nothing reports \"test result: ok\" and is"
    echo "indistinguishable from a passing file. Either its gate is wrong, or the"
    echo "feature set this leg enables no longer matches the files it should build."
fi

exit "$status"
