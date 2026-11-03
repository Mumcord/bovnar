# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# Does the stamped Content-Security-Policy still authorise the page's scripts?
#
# The policy pins every inline <script> by the SHA-256 of its bytes (there is no
# server to mint a nonce). Edit a script and its hash changes, so a stamped
# policy silently stops matching -- and the browser then BLOCKS that script,
# breaking the page for everyone, with the failure visible only in a console
# nobody watches. gen_csp.py recomputes the policy; this fails the suite when the
# committed pages have drifted from it. It shells out to the same Python so there
# is one implementation of the hashing, not a second one to keep in sync.
execute_process(
    COMMAND ${PYTHON} ${SCRIPT} --check
    WORKING_DIRECTORY ${WORKDIR}
    RESULT_VARIABLE _rc
    OUTPUT_VARIABLE _out
    ERROR_VARIABLE  _err)
if(NOT _rc EQUAL 0)
    message(FATAL_ERROR
        "Content-Security-Policy is stale.\n${_out}${_err}\n"
        "Re-run:  ./gen_csp.py")
endif()
message(STATUS "${_out}")
