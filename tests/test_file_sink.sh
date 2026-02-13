#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SAMPLES="$SCRIPT_DIR/samples"
OUTPUT="$SCRIPT_DIR/_output"
BINARY="$ROOT_DIR/target/debug/ssbt-tool"

PASSED=0
FAILED=0

pass() { echo "  PASS: $1"; PASSED=$((PASSED + 1)); }
fail() { echo "  FAIL: $1"; FAILED=$((FAILED + 1)); }

check_file() {
    local original="$1"
    local extracted="$2"
    local label="$3"
    if [ ! -f "$extracted" ]; then
        fail "$label — file missing"
        return
    fi
    if diff -q "$original" "$extracted" >/dev/null 2>&1; then
        pass "$label"
    else
        fail "$label — content mismatch"
    fi
}

cleanup() {
    rm -rf "$OUTPUT"
}

# ─── Build ────────────────────────────────────────────────────────────
echo "=== Building ssbt-tool ==="
cd "$ROOT_DIR"
cargo build -p ssbt-tool 2>&1
echo ""

# ─── Test 1: Basic file backup (no compression) ──────────────────────
echo "=== Test 1: Basic file backup ==="
cleanup
mkdir -p "$OUTPUT/extracted"

"$BINARY" --output "$OUTPUT/backup.zip" "$SAMPLES"

unzip -o "$OUTPUT/backup.zip" -d "$OUTPUT/extracted" >/dev/null

check_file "$SAMPLES/hello.txt"        "$OUTPUT/extracted/hello.txt"        "hello.txt present and correct"
check_file "$SAMPLES/data.csv"         "$OUTPUT/extracted/data.csv"         "data.csv present and correct"
check_file "$SAMPLES/subdir/nested.txt" "$OUTPUT/extracted/subdir/nested.txt" "subdir/nested.txt preserved"
check_file "$SAMPLES/.hidden"          "$OUTPUT/extracted/.hidden"          ".hidden file included"
echo ""

# ─── Test 2: Compressed backup ────────────────────────────────────────
echo "=== Test 2: Compressed backup (--compress) ==="
cleanup
mkdir -p "$OUTPUT/extracted"

"$BINARY" --compress --output "$OUTPUT/compressed.zip" "$SAMPLES"

unzip -o "$OUTPUT/compressed.zip" -d "$OUTPUT/extracted" >/dev/null

check_file "$SAMPLES/hello.txt"        "$OUTPUT/extracted/hello.txt"        "hello.txt (compressed)"
check_file "$SAMPLES/data.csv"         "$OUTPUT/extracted/data.csv"         "data.csv (compressed)"
check_file "$SAMPLES/subdir/nested.txt" "$OUTPUT/extracted/subdir/nested.txt" "subdir/nested.txt (compressed)"
check_file "$SAMPLES/.hidden"          "$OUTPUT/extracted/.hidden"          ".hidden (compressed)"
echo ""

# ─── Test 3: Skip pattern ────────────────────────────────────────────
echo "=== Test 3: Skip pattern (--skip '*.csv') ==="
cleanup
mkdir -p "$OUTPUT/extracted"

"$BINARY" --skip '*.csv' --output "$OUTPUT/skipped.zip" "$SAMPLES"

unzip -o "$OUTPUT/skipped.zip" -d "$OUTPUT/extracted" >/dev/null

check_file "$SAMPLES/hello.txt" "$OUTPUT/extracted/hello.txt" "hello.txt present (not skipped)"
if [ -f "$OUTPUT/extracted/data.csv" ]; then
    fail "data.csv should have been skipped"
else
    pass "data.csv correctly skipped"
fi
echo ""

# ─── Test 4: Dry run produces no file ────────────────────────────────
echo "=== Test 4: Dry run ==="
cleanup

"$BINARY" --dry --output "$OUTPUT/should_not_exist.zip" "$SAMPLES" >/dev/null

if [ ! -f "$OUTPUT/should_not_exist.zip" ]; then
    pass "dry run did not create archive"
else
    fail "dry run created an archive"
fi
echo ""

# ─── Summary ──────────────────────────────────────────────────────────
cleanup
TOTAL=$((PASSED + FAILED))
echo "=== Results: $PASSED/$TOTAL passed ==="
if [ "$FAILED" -gt 0 ]; then
    echo "$FAILED test(s) FAILED"
    exit 1
fi
echo "All tests passed."
exit 0
