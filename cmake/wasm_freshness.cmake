# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# Is the committed WebAssembly artifact built from the current sources?
#
# The web playground is presented as "the C reference implementation, via WASM".
# It drifted 35 commits behind without anything noticing: the shipped module
# still answered the pre-fix value for a tai leap second long after the library
# had been corrected. The cache-busting ?v= hashes in build_wasm.sh key on the
# ARTIFACT's bytes, so they roll when it is rebuilt and say nothing about whether
# it should be.
#
# build_wasm.sh records the hash of everything that goes into the module; this
# recomputes it and compares. It needs neither emscripten nor node, so it can run
# in any build.
if(NOT EXISTS "${STAMP}")
    message(FATAL_ERROR
        "no WASM build stamp at ${STAMP} -- run wasm/build_wasm.sh")
endif()
file(READ "${STAMP}" _recorded)
string(STRIP "${_recorded}" _recorded)

set(_h "")
foreach(f ${SOURCES})
    if(NOT EXISTS "${f}")
        message(FATAL_ERROR "WASM input missing: ${f}")
    endif()
    file(SHA256 "${f}" _one)
    string(APPEND _h "${_one}")
endforeach()
string(SHA256 _actual "${_h}")

if(NOT _actual STREQUAL _recorded)
    message(FATAL_ERROR
        "the committed WebAssembly artifact is STALE.\n"
        "  recorded: ${_recorded}\n"
        "  current:  ${_actual}\n"
        "The library sources changed since web/bovnar_wasm_core.js was built, so "
        "the playground is no longer the reference implementation it claims to "
        "be. Rebuild it:\n"
        "    source ~/emsdk/emsdk_env.sh && wasm/build_wasm.sh")
endif()
message(STATUS "WASM artifact is current (${_actual})")
