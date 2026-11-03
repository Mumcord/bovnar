# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# CMake toolchain file for cross-compiling the 64-bit MinGW-w64 Windows build
# from a Linux host (mirrors the MINGW64 leg of .github/workflows/build-and-package.yml,
# which builds natively under MSYS2). Use it for a local cross-build:
#
#   cmake -B build/mingw -DCMAKE_TOOLCHAIN_FILE=cmake/toolchain-mingw-w64-x86_64.cmake \
#         -DCMAKE_BUILD_TYPE=Release -DBVNR_BUILD_TESTS=OFF .
#   cmake --build build/mingw
#
# Produces libbvnr.a, libbvnr.dll (+ libbvnr.dll.a import lib) and bovnar.exe.
# Tests stay OFF: the harness is POSIX (fork/exec, socketpair/pthread, sigaction)
# and there is no Wine/Windows runtime on the host to execute the .exe locally;
# this leg verifies the cross-build compiles and links.

set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR x86_64)

set(TOOLCHAIN_PREFIX x86_64-w64-mingw32)
set(CMAKE_C_COMPILER   ${TOOLCHAIN_PREFIX}-gcc)
set(CMAKE_CXX_COMPILER ${TOOLCHAIN_PREFIX}-g++)
set(CMAKE_RC_COMPILER  ${TOOLCHAIN_PREFIX}-windres)

# Search the cross sysroot for libraries/headers, but use host programs (cmake,
# make) as-is.
set(CMAKE_FIND_ROOT_PATH /usr/${TOOLCHAIN_PREFIX})
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)

# Run cross-built executables (the CLI in the smoke tests, the cross-built unit
# test binaries) through Wine when it is installed. CTest honours
# CMAKE_CROSSCOMPILING_EMULATOR for add_test() targets automatically, and the
# shared cmake/*.cmake CLI test helpers pick it up via -DEMULATOR=. When Wine is
# absent the variable stays empty: the library/CLI still cross-compile, but the
# test executables cannot be run here (CTest will report them as failures, so
# build with -DBVNR_BUILD_TESTS=OFF in that case).
find_program(BVNR_WINE NAMES wine64 wine)
if(BVNR_WINE)
    set(CMAKE_CROSSCOMPILING_EMULATOR "${BVNR_WINE}" CACHE FILEPATH
        "Emulator used to run cross-compiled Windows executables under CTest")
endif()
