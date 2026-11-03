# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# Do the ?v= cache-busting stamps in web/index.html still match the files they label?
#
# build_wasm.sh derives each stamp from its file's own bytes, so the stamp IS the
# content hash. That only holds if every edit to those files goes through the
# build. Editing web/bovnar_parser_wasm.js by hand and not re-running the build
# leaves the page requesting ?v=<old hash> for new content -- so any client that
# fetched the old bytes under that URL keeps serving them from cache forever,
# which is precisely what the stamp exists to prevent. That happened once.
#
# Recomputing here needs neither emscripten nor node.
foreach(_pair ${PAIRS})
    string(REPLACE ":" ";" _p "${_pair}")
    list(GET _p 0 _file)
    list(GET _p 1 _name)
    if(NOT EXISTS "${_file}")
        message(FATAL_ERROR "cache-stamp input missing: ${_file}")
    endif()
    file(SHA256 "${_file}" _full)
    string(SUBSTRING "${_full}" 0 12 _want)

    file(READ "${PAGE}" _page)
    if(NOT _page MATCHES "${_name}\\?v=([0-9a-f]+)")
        message(FATAL_ERROR "no ?v= stamp for ${_name} in ${PAGE}")
    endif()
    set(_have "${CMAKE_MATCH_1}")
    if(NOT _have STREQUAL _want)
        message(FATAL_ERROR
            "cache-bust stamp for ${_name} is STALE.\n"
            "  page says: ${_have}\n"
            "  file is:   ${_want}\n"
            "The file was edited without re-running wasm/build_wasm.sh, so the "
            "page requests the new content under the old URL and caches will "
            "keep serving the old bytes. Re-run:\n"
            "    source ~/emsdk/emsdk_env.sh && wasm/build_wasm.sh")
    endif()
endforeach()
message(STATUS "cache-bust stamps match their files")
