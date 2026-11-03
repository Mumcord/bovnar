#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# run_tests.sh — build, then run EVERY registered test, then sweep the fuzzers.
#
# THIS SCRIPT DOES NOT KEEP A LIST OF TESTS, and that is the point. It used to:
# a hand-written sequence of run_test/run_cli_test/run_cmake_check calls that was
# a second copy of the CTest registry, maintained by remembering to. It drifted,
# the way a second copy of anything drifts — 81 line items here against 157
# registered tests, and the whole of it invisible from the inside, because a
# runner that never knew about a test cannot report it missing. Everything the
# seven unit-profile suites check, every documentation and web gate, the ABI
# dump, the amalgamation checks, the WASM freshness stamp and two thirds of the
# CLI corpus ran only under `ctest` and were reported as "All tests passed" here.
#
# So the corpus is now CTest's, run whole and unfiltered, and this script is what
# CTest is not:
#
#   * it BUILDS first, so "the tests pass" cannot mean "the tests as they were";
#   * it sweeps the fuzz harnesses at an iteration count you choose, where the
#     registered fuzz tests are pinned to fixed ones;
#   * it drives a cross-built (MinGW) tree through Wine;
#   * it ASSERTS that the number of tests run equals the number registered, so a
#     filter — including this script's own --no-fuzz — cannot quietly shrink the
#     run.
#
# Usage:
#   ./run_tests.sh [--build-dir DIR] [--fuzz-iter N] [--no-fuzz] [-j N]

set -euo pipefail

BUILD_DIR="./build"
TESTS_DIR="${BUILD_DIR}/tests"
FUZZ_ITER=5000
RUN_FUZZ=1
JOBS=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

while [[ $# -gt 0 ]]; do
    case "$1" in
        --build-dir) BUILD_DIR="$2"; TESTS_DIR="${BUILD_DIR}/tests"; shift 2 ;;
        --fuzz-iter) FUZZ_ITER="$2"; shift 2 ;;
        --no-fuzz)   RUN_FUZZ=0;     shift   ;;
        -j)          JOBS="$2";       shift 2 ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

# Cross (MinGW) build detection. CTest launches the registered tests through the
# toolchain's CMAKE_CROSSCOMPILING_EMULATOR by itself; what needs Wine here is
# the fuzz sweep below, which runs the executables directly.
EXE_SUFFIX=""
RUNNER=()
if [[ -e "${BUILD_DIR}/bovnar.exe" ]]; then
    EXE_SUFFIX=".exe"
    if command -v wine64 > /dev/null 2>&1; then
        RUNNER=(wine64)
    elif command -v wine > /dev/null 2>&1; then
        RUNNER=(wine)
    else
        echo "Windows build in ${BUILD_DIR} but Wine is not installed;" \
             "cannot run the .exe tests." >&2
        exit 1
    fi
fi

PASS=0
FAIL=0
SKIP=0
FAILED_TESTS=()

# Single scratch dir for all temp artifacts (the fuzz seed, the CTest log);
# cleaned up on exit.
SCRATCH_DIR="$(mktemp -d "${TMPDIR:-/tmp}/bvnr_run_tests.XXXXXX")"
trap 'rm -rf "$SCRATCH_DIR"' EXIT

CTEST_BIN="${CTEST:-ctest}"

_green()  { printf '\033[0;32m%s\033[0m\n' "$*"; }
_red()    { printf '\033[0;31m%s\033[0m\n' "$*"; }
_yellow() { printf '\033[0;33m%s\033[0m\n' "$*"; }
_bold()   { printf '\033[1m%s\033[0m\n'    "$*"; }

run_test() {

    local label="$1"; shift
    local exe="$1${EXE_SUFFIX}"; shift

    if [[ ! -e "$exe" ]]; then
        _yellow "  SKIP  $label  (not built: $exe)"
        (( SKIP++ )) || true
        return
    fi

    printf '  %-52s ' "$label"
    if ${RUNNER[@]+"${RUNNER[@]}"} "$exe" "$@"; then
        _green "PASS"
        (( PASS++ )) || true
    else
        _red "FAIL"
        (( FAIL++ )) || true
        FAILED_TESTS+=("$label")
    fi
}

_bold "=== Building ==="
cmake --build "$BUILD_DIR" --parallel "$JOBS"
echo

# ── The registered corpus, whole ────────────────────────────────────────────

_bold "=== CTest (every registered test) ==="

CTEST_FILTER=()
if [[ $RUN_FUZZ -eq 0 ]]; then
    # The one filter this script may apply, and it is reported below rather than
    # left to be inferred from a smaller number.
    CTEST_FILTER=(-LE fuzz)
fi

if ! "${CTEST_BIN}" --test-dir "${BUILD_DIR}" -N > /dev/null 2>&1; then
    _red "  FAIL  ctest cannot read ${BUILD_DIR} — configure it first" \
         "(cmake -S . -B ${BUILD_DIR})"
    (( FAIL++ )) || true
    FAILED_TESTS+=("ctest --test-dir ${BUILD_DIR}")
    REGISTERED=0
    RAN=0
else
    # ${arr[@]+...} rather than a bare "${arr[@]}": an empty array under
    # `set -u` is an unbound variable on bash 3.2, which is what macOS ships.
    #
    # A test marked DISABLED is listed and counted by -N and is then not run, so
    # it is subtracted here rather than being allowed to read as a hole in the
    # coverage below. Disabling one is a decision recorded in CMakeLists; a test
    # that silently did not run is not.
    LISTING=$("${CTEST_BIN}" --test-dir "${BUILD_DIR}" -N \
        ${CTEST_FILTER[@]+"${CTEST_FILTER[@]}"})
    REGISTERED=$(printf '%s\n' "${LISTING}" | sed -n 's/^Total Tests: //p' | tail -1)
    DISABLED=$(printf '%s\n' "${LISTING}" | grep -c "(Disabled)" || true)
    REGISTERED=$(( REGISTERED - DISABLED ))
    ALL_REGISTERED=$("${CTEST_BIN}" --test-dir "${BUILD_DIR}" -N \
        | sed -n 's/^Total Tests: //p' | tail -1)
    if [[ $RUN_FUZZ -eq 0 ]]; then
        _yellow "  --no-fuzz: $(( ALL_REGISTERED - REGISTERED )) fuzz test(s) excluded" \
                "of ${ALL_REGISTERED}"
    fi

    CTEST_LOG="${SCRATCH_DIR}/ctest.log"
    set +e
    "${CTEST_BIN}" --test-dir "${BUILD_DIR}" -j "${JOBS}" --output-on-failure \
        ${CTEST_FILTER[@]+"${CTEST_FILTER[@]}"} 2>&1 | tee "${CTEST_LOG}"
    CTEST_RC=${PIPESTATUS[0]}
    set -e

    # "N% tests passed, F tests failed out of T" is the line that says what
    # actually ran, and it is the only place CTest states T after filtering.
    RAN=$(sed -n 's/.*tests failed out of \([0-9][0-9]*\).*/\1/p' "${CTEST_LOG}" \
        | tail -1)
    CTEST_FAILED=$(sed -n 's/.*, \([0-9][0-9]*\) tests failed out of.*/\1/p' \
        "${CTEST_LOG}" | tail -1)
    RAN=${RAN:-0}
    CTEST_FAILED=${CTEST_FAILED:-0}

    echo
    if [[ "${CTEST_RC}" -eq 0 && "${CTEST_FAILED}" -eq 0 ]]; then
        printf '  %-52s ' "ctest: ${RAN} test(s)"
        _green "PASS"
        PASS=$(( PASS + RAN ))
    else
        printf '  %-52s ' "ctest: ${CTEST_FAILED} of ${RAN} test(s) failed"
        _red "FAIL"
        PASS=$(( PASS + RAN - CTEST_FAILED ))
        FAIL=$(( FAIL + CTEST_FAILED ))
        while read -r t; do
            [[ -n "$t" ]] && FAILED_TESTS+=("ctest: $t")
        done < <(sed -n '/The following tests FAILED:/,$p' "${CTEST_LOG}" \
                 | sed -n 's/^[[:space:]]*[0-9][0-9]*[[:space:]]*-[[:space:]]*\([^ ]*\).*/\1/p')
    fi

    # The registry is the corpus, so anything short of it is a hole in this run
    # — a stray filter, a test CTest declined to schedule, an interrupted run.
    # It is the check whose absence let 81 stand in for 157.
    if [[ "${RAN}" -ne "${REGISTERED}" ]]; then
        printf '  %-52s ' "coverage: ran ${RAN} of ${REGISTERED} enabled"
        _red "FAIL"
        (( FAIL++ )) || true
        FAILED_TESTS+=("coverage: ${RAN} of ${REGISTERED} enabled tests ran")
    fi
fi

echo

# ── Beyond the registry: the fuzz harnesses, at your iteration count ────────
#
# CTest registers these at fixed iteration counts (bvnr_fuzz_reader and its
# _deep/_thorough siblings), which is right for a gate and useless for a sweep.
# The run above already covered those; this is the same harnesses driven as far
# as --fuzz-iter asks.

if [[ $RUN_FUZZ -eq 1 ]]; then
    _bold "=== Fuzz sweep (--fuzz-iter $FUZZ_ITER) ==="

    run_test "bvnr_fuzz_test --harness reader" \
        "$TESTS_DIR/bvnr_fuzz_test" \
        --harness reader --iterations "$FUZZ_ITER" --seed 42

    run_test "bvnr_fuzz_test --harness dom" \
        "$TESTS_DIR/bvnr_fuzz_test" \
        --harness dom --iterations "$FUZZ_ITER" --seed 42

    run_test "bvnr_fuzz_test --harness utils" \
        "$TESTS_DIR/bvnr_fuzz_test" \
        --harness utils --iterations "$FUZZ_ITER" --seed 42

    SEED_FILE="${SCRATCH_DIR}/bvnr_fuzz_writer_seed.bin"
    printf 'X' > "$SEED_FILE"

    run_test "bvnr_fuzz_writer_test (smoke)" \
        "$TESTS_DIR/bvnr_fuzz_writer_test" "$SEED_FILE"

    echo
else
    _yellow "=== Fuzz sweep skipped (--no-fuzz) ==="
    echo
fi

_bold "=== Results ==="
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo "  Skipped: $SKIP"

if [[ $FAIL -gt 0 ]]; then
    echo
    _red "Failed tests:"
    for t in "${FAILED_TESTS[@]}"; do
        _red "  • $t"
    done
    exit 1
fi

echo
_green "All tests passed."
