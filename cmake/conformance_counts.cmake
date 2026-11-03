# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# Do the conformance case counts in doc/13 still match the suite?
#
# The documented total said 306 in six places while the suite ran 319, and a
# whole group ("limits") was missing from the table. Numbers copied into prose
# drift; this one had drifted already, so check it rather than fix it again.
#
# The suite prints a per-group TAP plan and a total, both of which are compared
# against the table in doc/13_bovnar_conformance.md.
# The TAP stream goes to stdout, the summary line to stderr; look at both.
#
# Launch through CMAKE_CROSSCOMPILING_EMULATOR when there is one, the way
# query_expect.cmake and events_ok.cmake already do. Without it the MinGW cross
# build ran the .exe directly and the shell tried to execute a PE binary as a
# script -- "MZ...: not found", "Syntax error: \"(\" unexpected" -- so this gate
# was permanently red on the Windows target, and red in a way that says nothing
# about whether the counts actually drifted.
set(_cmd ${CONFORMANCE})
if(EMULATOR)
    set(_cmd ${EMULATOR} ${CONFORMANCE})
endif()
execute_process(COMMAND ${_cmd} OUTPUT_VARIABLE _out
                RESULT_VARIABLE _rc ERROR_VARIABLE _err)
if(NOT _rc EQUAL 0)
    message(FATAL_ERROR "conformance suite failed to run: ${_rc}\n${_err}")
endif()
set(_tap "${_out}${_err}")
if(NOT _tap MATCHES "All ([0-9]+) conformance tests passed")
    message(FATAL_ERROR "could not read the case total from the suite output")
endif()
set(_actual ${CMAKE_MATCH_1})

file(READ "${DOCFILE}" _doc)
if(NOT _doc MATCHES "\\| \\*\\*Total\\*\\* \\| \\*\\*([0-9]+)\\*\\* \\|")
    message(FATAL_ERROR "no Total row in ${DOCFILE}")
endif()
set(_documented ${CMAKE_MATCH_1})

if(NOT _actual EQUAL _documented)
    message(FATAL_ERROR
        "conformance case count drifted: the suite runs ${_actual}, "
        "doc/13_bovnar_conformance.md says ${_documented}.\n"
        "Update the per-group table and its Total row (and the counts quoted in "
        "README.md and web/index.html).")
endif()

# doc/13 calls a case's id its unique identifier, and a report names a case by
# nothing else. Five were not: a block of new encoding cases was numbered from
# ENC-005 beside the ENC-005..009 already there, so "not ok - [ENC-007]" named
# two cases with opposite expectations. The TAP lines carry every id.
string(REGEX MATCHALL "ok [0-9]+ - \\[[A-Z0-9]+-[0-9]+[a-z]?\\]" _idlines "${_tap}")
set(_ids "")
foreach(_l IN LISTS _idlines)
    string(REGEX REPLACE ".*- \\[(.*)\\]" "\\1" _id "${_l}")
    list(APPEND _ids "${_id}")
endforeach()
list(LENGTH _ids _nids)
if(NOT _nids EQUAL _actual)
    message(FATAL_ERROR
        "read ${_nids} case ids from a suite of ${_actual} cases; the TAP line "
        "format has changed and the uniqueness check below is blind")
endif()
set(_uniq ${_ids})
list(REMOVE_DUPLICATES _uniq)
list(LENGTH _uniq _nuniq)
if(NOT _nuniq EQUAL _nids)
    set(_dups "")
    foreach(_id IN LISTS _uniq)
        string(REGEX MATCHALL "\\[${_id}\\]" _hits "${_tap}")
        list(LENGTH _hits _n)
        if(_n GREATER 1)
            list(APPEND _dups "${_id}")
        endif()
    endforeach()
    message(FATAL_ERROR "conformance case ids used more than once: ${_dups}")
endif()

# Every group the suite reports must appear in the table, with the right count.
string(REGEX MATCHALL "# Subtest: [a-z_]+" _groups "${_tap}")
string(REGEX MATCHALL "    1\\.\\.[0-9]+" _plans "${_tap}")
list(LENGTH _groups _ngroups)
list(LENGTH _plans _nplans)
if(NOT _ngroups EQUAL _nplans)
    message(FATAL_ERROR "malformed TAP: ${_ngroups} groups, ${_nplans} plans")
endif()
math(EXPR _last "${_ngroups} - 1")
foreach(i RANGE ${_last})
    list(GET _groups ${i} _g)
    list(GET _plans  ${i} _p)
    string(REPLACE "# Subtest: " "" _g "${_g}")
    string(REPLACE "    1.." "" _p "${_p}")
    if(NOT _doc MATCHES "\\| \\`${_g}\\` \\| ${_p} \\|")
        message(FATAL_ERROR
            "group '${_g}' runs ${_p} cases; doc/13's table does not say so "
            "(row missing, or a different count)")
    endif()
endforeach()

# The illustrative TAP block in §9 carries the same numbers a second time -- the
# parent plan, the prose group count beside it, and the child plan of the group
# it walks through. Being an example rather than a table, it escaped the checks
# above and drifted: it still showed "1..21" and an encoding plan of "1..9" long
# after the suite had grown to 23 groups and 14 encoding cases. A reader
# implementing against §9 counts those, so check them too.
# The sample is a fenced block and contains no backtick, so a backtick-free run
# after the TAP header is bounded by the closing fence. CMake's regex engine has
# no lazy quantifier, and a greedy ".*" here ran on to the LAST child plan in the
# file rather than the sample's first.
if(NOT _doc MATCHES "TAP version 14\n([^`]*)")
    message(FATAL_ERROR "no illustrative TAP block in ${DOCFILE} §9")
endif()
set(_sample "${CMAKE_MATCH_1}")
if(NOT _sample MATCHES "^1\\.\\.([0-9]+)\n# Subtest: ([a-z_]+)\n")
    message(FATAL_ERROR "${DOCFILE} §9's sample TAP block has no parent plan and first subtest")
endif()
set(_sample_plan ${CMAKE_MATCH_1})
set(_sample_group ${CMAKE_MATCH_2})
if(NOT _sample_plan EQUAL _ngroups)
    message(FATAL_ERROR
        "doc/13 §9's sample TAP parent plan says 1..${_sample_plan}; the suite "
        "reports ${_ngroups} groups")
endif()
if(NOT _doc MATCHES "parent plan therefore counts the groups \\(currently ([0-9]+)\\)")
    message(FATAL_ERROR "doc/13 §9 no longer states the group count in prose")
endif()
if(NOT CMAKE_MATCH_1 EQUAL _ngroups)
    message(FATAL_ERROR
        "doc/13 §9 says there are currently ${CMAKE_MATCH_1} groups; the suite "
        "reports ${_ngroups}")
endif()
# The child plan closing the sample's first group, against that group's real
# size. Take the FIRST child plan inside the sample -- the block goes on to show
# a second group, whose plan must not be read as this one's.
string(REGEX MATCHALL "    1\\.\\.[0-9]+" _sample_plans "${_sample}")
if(NOT _sample_plans)
    message(FATAL_ERROR "doc/13 §9's sample has no child plan for '${_sample_group}'")
endif()
list(GET _sample_plans 0 _sample_child)
string(REPLACE "    1.." "" _sample_child "${_sample_child}")
list(FIND _groups "# Subtest: ${_sample_group}" _idx)
if(_idx EQUAL -1)
    message(FATAL_ERROR "doc/13 §9 walks through group '${_sample_group}', which the suite does not run")
endif()
list(GET _plans ${_idx} _real)
string(REPLACE "    1.." "" _real "${_real}")
if(NOT _sample_child EQUAL _real)
    message(FATAL_ERROR
        "doc/13 §9's sample shows group '${_sample_group}' closing at "
        "1..${_sample_child}; the suite runs ${_real} cases there")
endif()

message(STATUS "conformance counts match doc/13 (${_actual} cases, ${_ngroups} groups, "
               "including §9's sample TAP block)")
