# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# Does the amalgamation depend on WHERE it was built?
#
# It must not. bvnr_wasm_freshness compares a hash of the amalgamated sources
# against a stamp committed by wasm/build_wasm.sh, so the moment the output
# varies with the build directory, that gate reports a stale artifact on a tree
# where nothing is stale -- and would report a fresh one only for whoever built
# in a directory named the same as the recorder's.
#
# This has been wrong twice. The banner comments that head each inlined fragment
# carry the fragment's path, and the generated ones live under <build>/generated;
# the first fix normalised a build tree OUTSIDE the source tree and one named
# exactly "build", which left `cmake -B build-asan` -- i.e. the sanitizer job --
# falling through with its directory name baked into the output.
#
# Build the amalgamation twice from byte-identical inputs, differing only in the
# name of the directory the generated fragments sit in, and require the two to
# match exactly.

foreach(_v PYTHON SCRIPT GENERATED WORKDIR)
    if(NOT ${_v})
        message(FATAL_ERROR "amalgam_reproducible: ${_v} not set")
    endif()
endforeach()

# Two names sharing no common prefix, so a substring or startswith() test on
# either cannot accidentally succeed. "build" is deliberately one of them: it is
# the name the stamp was recorded under and the one case that already worked.
set(_dir_a "${WORKDIR}/build/generated")
set(_dir_b "${WORKDIR}/zzz-other-name/generated")

file(REMOVE_RECURSE "${WORKDIR}")
file(MAKE_DIRECTORY "${_dir_a}" "${_dir_b}")
file(GLOB _frags "${GENERATED}/*")
if(NOT _frags)
    message(FATAL_ERROR
        "amalgam_reproducible: no generated fragments in ${GENERATED}")
endif()
file(COPY ${_frags} DESTINATION "${_dir_a}")
file(COPY ${_frags} DESTINATION "${_dir_b}")

function(_amalgamate _gendir _out)
    file(MAKE_DIRECTORY "${_out}")
    execute_process(
        COMMAND "${CMAKE_COMMAND}" -E env "BVNR_GENERATED_DIR=${_gendir}"
                "${PYTHON}" "${SCRIPT}" "${_out}"
        RESULT_VARIABLE _rc OUTPUT_QUIET ERROR_VARIABLE _err)
    if(NOT _rc EQUAL 0)
        message(FATAL_ERROR "amalgamate.py failed for ${_gendir}:\n${_err}")
    endif()
endfunction()

_amalgamate("${_dir_a}" "${WORKDIR}/out_a")
_amalgamate("${_dir_b}" "${WORKDIR}/out_b")

# The banner lines are where a path can leak, so on a mismatch report the ones
# that differ: the hashes alone say something changed, these say what.
set(_banner "^/\\* =+ .+ =+ \\*/$")

foreach(_f bovnar.c bovnar.h)
    file(SHA256 "${WORKDIR}/out_a/${_f}" _ha)
    file(SHA256 "${WORKDIR}/out_b/${_f}" _hb)
    if(NOT _ha STREQUAL _hb)
        file(STRINGS "${WORKDIR}/out_a/${_f}" _la REGEX "${_banner}")
        file(STRINGS "${WORKDIR}/out_b/${_f}" _lb REGEX "${_banner}")
        set(_diff "")
        list(LENGTH _la _na)
        list(LENGTH _lb _nb)
        if(_na EQUAL _nb AND _na GREATER 0)
            math(EXPR _last "${_na} - 1")
            foreach(_i RANGE ${_last})
                list(GET _la ${_i} _x)
                list(GET _lb ${_i} _y)
                if(NOT _x STREQUAL _y)
                    string(APPEND _diff "\n      ${_x}\n  vs  ${_y}")
                endif()
            endforeach()
        else()
            set(_diff "\n      (banner counts differ: ${_na} vs ${_nb})")
        endif()
        message(FATAL_ERROR
            "${_f} depends on the build directory it was generated in.\n"
            "  build/generated          -> ${_ha}\n"
            "  zzz-other-name/generated -> ${_hb}\n"
            "Differing lines:${_diff}\n"
            "Every build tree must produce byte-identical output, or "
            "bvnr_wasm_freshness reports a stale artifact on a fresh tree.")
    endif()
endforeach()

message(STATUS "amalgamation is independent of the build directory")
