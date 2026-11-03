#!/bin/sh
# Builds the Bovnar C reference implementation that serves as the conformance
# oracle for this crate. The UCUM and CF unit profiles are switched off because
# bovnar-rs does not ship their tables; the reference must reject those
# namespaces exactly like we do.
#
# Usage: scripts/build-cref.sh <upstream-source-dir> <build-dir>
# Produces <build-dir>/tests/bvnr_conformance and
#          <build-dir>/tests/bvnr_conformance_iut (and <build-dir>/libbvnr.a).
set -eu

if [ "$#" -ne 2 ]; then
	echo "usage: $0 <upstream-source-dir> <build-dir>" >&2
	exit 2
fi

upstream=$1
out=$2
jobs=${CMAKE_BUILD_PARALLEL_LEVEL:-$(nproc 2>/dev/null || echo 4)}

cmake -S "$upstream" -B "$out" \
	-DCMAKE_BUILD_TYPE=Release \
	-DBVNR_BUILD_TESTS=ON \
	-DBVNR_WITH_UCUM_PROFILE=OFF \
	-DBVNR_WITH_CF_PROFILE=OFF
cmake --build "$out" --parallel "$jobs" \
	--target bvnr_static bvnr_conformance bvnr_conformance_iut
