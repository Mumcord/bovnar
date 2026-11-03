# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# Run `bovnar events -d <FILE>` and require a clean, complete run.
# Used by bvnr_cli_events_long_unit: the point is that the debug annotation
# rebuilder survives a unit at the format's maximum length, which it did not --
# it read past a 128-byte stack buffer because bvn_unit_to_string writes no NUL
# when the text does not fit.
set(_cmd ${BOVNAR})
if(EMULATOR)
    set(_cmd ${EMULATOR} ${BOVNAR})
endif()
execute_process(COMMAND ${_cmd} events -d ${FILE}
                OUTPUT_VARIABLE _out ERROR_VARIABLE _err RESULT_VARIABLE _rc)
if(NOT _rc EQUAL 0)
    message(FATAL_ERROR "events -d exited ${_rc} on ${FILE}\n${_out}\n${_err}")
endif()
if(NOT _out MATCHES "All tokens validated successfully")
    message(FATAL_ERROR "events -d did not validate ${FILE}\n${_out}\n${_err}")
endif()
if(_out MATCHES "unprintable unit")
    message(FATAL_ERROR "the annotation rebuilder could not render the unit; "
                        "BVNR_UNIT_STRING_MAX may be too small\n${_out}")
endif()
