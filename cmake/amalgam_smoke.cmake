# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# CTest driver for the single-file amalgamation.
#
# Generates the amalgamation into the build tree (never touching the committed
# dist/), compiles it together with the smoke test under the same strict
# warnings as the main build, runs it, and finally verifies the committed
# dist/bovnar.{h,c} are byte-identical to a fresh generation. So this one test
# catches both a broken amalgamation (compile/run failure) and a stale committed
# one (someone changed sources without re-running amalgamate.py).
#
# Required -D args: SRC_DIR, PYTHON, CC, BIN_DIR

if(NOT SRC_DIR OR NOT PYTHON OR NOT CC OR NOT BIN_DIR OR NOT GENERATED)
    message(FATAL_ERROR "amalgam_smoke.cmake: SRC_DIR/PYTHON/CC/BIN_DIR required")
endif()

set(gen "${BIN_DIR}/amalgam_gen")
file(MAKE_DIRECTORY "${gen}")

# 1. Generate into the build tree.
execute_process(
    COMMAND "${CMAKE_COMMAND}" -E env "BVNR_GENERATED_DIR=${GENERATED}"
            "${PYTHON}" "${SRC_DIR}/amalgamate.py" "${gen}"
    WORKING_DIRECTORY "${SRC_DIR}"
    RESULT_VARIABLE rc)
if(NOT rc EQUAL 0)
    message(FATAL_ERROR "amalgamate.py failed (${rc})")
endif()

# 2. Compile the freshly-generated amalgamation + smoke test (strict flags).
#
# NO -Wno- OF ANY KIND HERE, and that is the point of the step. This is meant to
# be an integrator's first build of dist/bovnar.c, so every flag it needs that
# the integrator would not think to pass is a warning they meet and we do not.
# The one warning this ever produced -- 546 x -Woverride-init from the lexer's
# deliberate designated-initialiser defaults -- is suppressed by a pragma in
# src/lexer/bovnar_state_table.c, which travels into the amalgamation with the
# code it is about. Adding a flag back here would hide the next such case.
set(exe "${BIN_DIR}/amalgam_smoke")
execute_process(
    COMMAND "${CC}" -std=c99 -pedantic -O2 -Wall -Wextra -Wconversion -Werror
            "-I${gen}"
            "${SRC_DIR}/tests/amalgam_smoke.c"
            "${gen}/bovnar.c"
            -o "${exe}"
    RESULT_VARIABLE rc
    OUTPUT_VARIABLE out ERROR_VARIABLE err)
if(NOT rc EQUAL 0)
    message(FATAL_ERROR "amalgamation failed to compile:\n${out}\n${err}")
endif()

# 3. Run it.
execute_process(COMMAND "${exe}" RESULT_VARIABLE rc OUTPUT_VARIABLE out)
message(STATUS "${out}")
if(NOT rc EQUAL 0)
    message(FATAL_ERROR "amalgam smoke test failed (${rc})")
endif()

# 4. If dist/ ships a committed amalgamation, it must match the fresh one (no
#    stale checkout). dist/bovnar.{c,h} are git-ignored today, so this step is
#    skipped rather than failing every run -- but it must not be DELETED: the
#    day the amalgamation is shipped again, a stale committed copy is exactly
#    the failure nobody notices. Skipping is reported, so "step 4 did nothing"
#    can never be mistaken for "step 4 passed".
if(EXISTS "${SRC_DIR}/dist/bovnar.h" AND EXISTS "${SRC_DIR}/dist/bovnar.c")
    foreach(f bovnar.h bovnar.c)
        execute_process(
            COMMAND "${CMAKE_COMMAND}" -E compare_files "${gen}/${f}" "${SRC_DIR}/dist/${f}"
            RESULT_VARIABLE diff)
        if(NOT diff EQUAL 0)
            message(FATAL_ERROR
                "dist/${f} is stale: re-run `python3 amalgamate.py` and commit dist/.")
        endif()
    endforeach()
else()
    message(STATUS "amalgam smoke: dist/ ships no amalgamation, freshness step skipped")
endif()
