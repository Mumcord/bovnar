//! Hand-written edge inputs checked against the C reference.
//!
//! Every expected output below is the stdout of the C reference IUT adapter
//! (`bvnr_conformance_iut`, upstream commit `1bcf03b`, UCUM and CF off) for
//! that input: the event log on success, `ERROR <code>` on failure.

use bovnar::{conformance, Event, Limits, Reader};

/// Inputs that need nothing beyond the reader itself.
const CASES: &[(&[u8], &[u8])] = &[
    (b"", b"STREAM_START\n"),
    (b" ", b"STREAM_START\n"),
    (b"\xef\xbb\xbf.a = 1;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 1\n"),
    (b".a = 1", b"ERROR incomplete_bvnr_stream\n"),
    (b".a = ;", b"STREAM_START\nASSIGNMENT_START a\nDATA null \n"),
    (b".a = <uint:8> 255;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8\nTYPE_FAMILY uint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END uint:8\nDATA number 255\n"),
    (b".a = <uint:8> 256;", b"ERROR value_out_of_range\n"),
    (b".a = <sint:8> -128;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START sint:8\nTYPE_FAMILY sint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END sint:8\nDATA number -128\n"),
    (b".a = <sint:8> -129;", b"ERROR value_out_of_range\n"),
    (b".a = <uint:64> 18446744073709551615;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:64\nTYPE_FAMILY uint:64\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END uint:64\nDATA number 18446744073709551615\n"),
    (b".a = <uint:64> 18446744073709551616;", b"ERROR value_out_of_range\n"),
    (b".a = <sint:64> -9223372036854775808;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START sint:64\nTYPE_FAMILY sint:64\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END sint:64\nDATA number -9223372036854775808\n"),
    (b".a = <uint:65> 36893488147419103231;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:65\nTYPE_FAMILY uint:65\nTYPE_PARAM_WIDTH 65\nTYPE_ANN_END uint:65\nDATA number 36893488147419103231\n"),
    (b".a = <uint:65> 36893488147419103232;", b"ERROR value_out_of_range\n"),
    (b".a = <uint:8,_16> \"ff\";", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8,_16\nTYPE_FAMILY uint:8,_16\nTYPE_PARAM_WIDTH 8\nTYPE_PARAM_BASE 16\nTYPE_ANN_END uint:8,_16\nDATA string ff\n"),
    (b".a = <uint:8,_16> \"100\";", b"ERROR value_out_of_range\n"),
    (b".a = <uint:8,_16> \"fg\";", b"ERROR digit_not_in_base\n"),
    (b".a = <uint:8,_2> \"11111111\";", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8,_2\nTYPE_FAMILY uint:8,_2\nTYPE_PARAM_WIDTH 8\nTYPE_PARAM_BASE 2\nTYPE_ANN_END uint:8,_2\nDATA string 11111111\n"),
    (b".a = <uint:32,_64> \"+-\";", b"ERROR digit_not_in_base\n"),
    (b".a = <sint:32,_64> \"+-\";", b"ERROR illegal_value_type\n"),
    (b".a = <uint:8,_36> \"inf\";", b"ERROR value_out_of_range\n"),
    (b".a = <uint:8> inf;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8\nTYPE_FAMILY uint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END uint:8\nDATA number inf\n"),
    (b".a = <uint> nan;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_ANN_END uint\nDATA number nan\n"),
    (b".a = <float_fix:16,q8> 127.99609375;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_fix:16,q8\nTYPE_FAMILY float_fix:16,q8\nTYPE_PARAM_WIDTH 16\nTYPE_PARAM_Q 8\nTYPE_ANN_END float_fix:16,q8\nDATA number 127.99609375\n"),
    (b".a = <float_fix:16,q8> 128;", b"ERROR value_out_of_range\n"),
    (b".a = <float_fix:16,q8> -128;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_fix:16,q8\nTYPE_FAMILY float_fix:16,q8\nTYPE_PARAM_WIDTH 16\nTYPE_PARAM_Q 8\nTYPE_ANN_END float_fix:16,q8\nDATA number -128\n"),
    (b".a = <float_fix:16,q8> 127.998046875;", b"ERROR value_out_of_range\n"),
    (b".a = <float_fix:16,q16> 0;", b"ERROR illegal_value_type\n"),
    (b".a = <float:16> 1e400;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float:16\nTYPE_FAMILY float:16\nTYPE_PARAM_WIDTH 16\nTYPE_ANN_END float:16\nDATA number 1e400\n"),
    (b".a = <float:64,_16> 0x1.8p3;", b"ERROR unexpected_input_byte\n"),
    (b".a = <float:64,_16> \"1.8p3\";", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float:64,_16\nTYPE_FAMILY float:64,_16\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 16\nTYPE_ANN_END float:64,_16\nDATA string 1.8p3\n"),
    (b".a = <float:33> 1;", b"ERROR illegal_value_type\n"),
    (b".a = <float:64,_8> 1;", b"ERROR illegal_value_type\n"),
    (b".a = <utf8:0> \"x\";", b"ERROR illegal_value_type\n"),
    (b".a = <bool> on;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START bool\nTYPE_FAMILY bool\nTYPE_ANN_END bool\nDATA bool true\n"),
    (b".a = <bool> 1;", b"ERROR type_value_mismatch\n"),
    (b".a = off;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START bool\nTYPE_FAMILY bool\nTYPE_ANN_END bool\nDATA bool false\n"),
    (b".a = <uint> 1.0;", b"ERROR type_value_mismatch\n"),
    (b".a = <uint> 1e3;", b"ERROR type_value_mismatch\n"),
    (b".a = 1e3;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float\nTYPE_FAMILY float\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END float\nDATA number 1e3\n"),
    (b".a = -0;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START sint\nTYPE_FAMILY sint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END sint\nDATA number -0\n"),
    (b".a = 007;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 007\n"),
    (b".a = [1, 2, 3];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 1\nDATA array_number 2\nDATA array_number 3\nARRAY_ROW_END\n"),
    (b".a = [1, -2, 3.5, \"x\", true, null];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 1\nTYPE_ANN_START sint\nTYPE_FAMILY sint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END sint\nDATA array_number -2\nTYPE_ANN_START float\nTYPE_FAMILY float\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END float\nDATA array_number 3.5\nTYPE_ANN_START utf8\nTYPE_FAMILY utf8\nTYPE_ANN_END utf8\nDATA array_string x\nTYPE_ANN_START bool\nTYPE_FAMILY bool\nTYPE_ANN_END bool\nDATA bool true\nDATA null \nARRAY_ROW_END\n"),
    (b".a = [[1,2],[3,4]];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nARRAY_ROW_START\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 1\nDATA array_number 2\nARRAY_ROW_END\nARRAY_ROW_START\nDATA array_number 3\nDATA array_number 4\nARRAY_ROW_END\nARRAY_ROW_END\n"),
    (b".a = [[1,2],[3]];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nARRAY_ROW_START\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 1\nDATA array_number 2\nARRAY_ROW_END\nARRAY_ROW_START\nDATA array_number 3\nARRAY_ROW_END\nARRAY_ROW_END\n"),
    (b".a = [1,2 / 3,4];", b"ERROR unexpected_input_byte\n"),
    (b".a = [];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nARRAY_ROW_END\n"),
    (b".a = [[]];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_END\nARRAY_ROW_END\n"),
    (b".a = <uint:8> [1, 2, 300];", b"ERROR value_out_of_range\n"),
    (b".a = [<uint:8> 1, 2, <sint:8> -3, 4];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nTYPE_ANN_START uint:8\nTYPE_FAMILY uint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END uint:8\nDATA array_number 1\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 2\nTYPE_ANN_START sint:8\nTYPE_FAMILY sint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END sint:8\nDATA array_number -3\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 4\nARRAY_ROW_END\n"),
    (b".a = {.b = 1; .c = {.d = \"x\";};};", b"STREAM_START\nASSIGNMENT_START a\nSTRUCT_START\nASSIGNMENT_START b\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 1\nASSIGNMENT_START c\nSTRUCT_START\nASSIGNMENT_START d\nTYPE_ANN_START utf8\nTYPE_FAMILY utf8\nTYPE_ANN_END utf8\nDATA string x\nSTRUCT_END\nSTRUCT_END\n"),
    (b".a = {.b = 1;}", b"ERROR incomplete_bvnr_stream\n"),
    (b".a = {};", b"STREAM_START\nASSIGNMENT_START a\nSTRUCT_START\nSTRUCT_END\n"),
    (b".a = };", b"ERROR unexpected_input_byte\n"),
    (b".a = \"\\x41\\u{1F600}\";", b"ERROR illegal_escape_sequence\n"),
    (b"#!bovnar 1.1\n.a = \"\\x41\\u{1F600}\";", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START utf8\nTYPE_FAMILY utf8\nTYPE_ANN_END utf8\nDATA string A\\xf0\\x9f\\x98\\x80\n"),
    (b"#!bovnar 1.1\n.a = \"\\u{D800}\";", b"ERROR invalid_codepoint\n"),
    (b"#!bovnar 1.1\n.a = \"\\x00\";", b"ERROR unexpected_input_byte\n"),
    (b"#!bovnar 1.1\n.a = <uint:8> \"\\x00\";", b"ERROR unexpected_input_byte\n"),
    (b"#!bovnar 1.1\n.a = 2026-06-15T12:00:00Z;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START datetime\nTYPE_FAMILY datetime\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END datetime\nDATA number 1781524800\n"),
    (b"#!bovnar 1.1\n.a = 2026-06-15T12:00:00.123+02:00;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START datetime\nTYPE_FAMILY datetime\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END datetime\nDATA number 1781517600\n"),
    (b"#!bovnar 1.1\n.a = 2016-12-31T23:59:60Z;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START datetime\nTYPE_FAMILY datetime\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END datetime\nDATA number 1483228800\n"),
    (b"#!bovnar 1.1\n.a = 2017-12-31T23:59:60Z;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START datetime\nTYPE_FAMILY datetime\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END datetime\nDATA number 1514764800\n"),
    (b"#!bovnar 1.1\n.a = 2024-02-30T00:00:00Z;", b"ERROR invalid_datetime_literal\n"),
    (b"#!bovnar 1.1\n.a = <datetime:64,tai> 2026-06-15T12:00:00Z;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START datetime:64,tai\nTYPE_FAMILY datetime:64,tai\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_UNIT tai\nTYPE_ANN_END datetime:64,tai\nDATA number 2160216037\n"),
    (b"#!bovnar 1.1\n.a = <datetime:32> 2106-06-15T12:00:00Z;", b"ERROR value_out_of_range\n"),
    (b"#!bovnar 1.1\n.a = [2026-06-15T12:00:00Z, 5, -7];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nTYPE_ANN_START datetime\nTYPE_FAMILY datetime\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END datetime\nDATA array_number 1781524800\nDATA array_number 5\nDATA array_number -7\nARRAY_ROW_END\n"),
    (b"#!bovnar 1.1\n.a = <datetime:64,gps> [1, 2026-06-15T12:00:00Z];", b"ERROR datetime_literal_unsupported_epoch\n"),
    (b"#!bovnar 1.1\n.a = <datetime:64,bogus> 1;", b"ERROR illegal_value_type\n"),
    (b"#!bovnar 1.1\n.a = <datetime:64,_10> 1;", b"ERROR illegal_value_type\n"),
    (b".a = 2026-06-15T12:00:00Z;", b"ERROR illegal_value_type\n"),
    (b"#!bovnar 1.3\n.a = 1;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 1\n"),
    (b"#!bovnar 2.0\n.a = 1;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 1\n"),
    (b"#!bovnar 1.x\n.a = 1;", b"ERROR invalid_spec_version\n"),
    (b"# comment\n#!bovnar 1.1\n.a = 1;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 1\n"),
    (b".a = 'sym;", b"ERROR unexpected_input_byte\n"),
    (b".a = @b.c[0];", b"ERROR unexpected_input_byte\n"),
    (b".a = @b..c;", b"ERROR unexpected_input_byte\n"),
    (b".a = 1; .a = 2;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 1\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 2\n"),
    (b".\xc3\xa4 = 1;", b"STREAM_START\nASSIGNMENT_START \\xc3\\xa4\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 1\n"),
    (b".a\xff = 1;", b"ERROR invalid_utf8_byte\n"),
    (b".a = \"\xff\";", b"ERROR invalid_utf8_byte\n"),
    (b".a = \"\xc3\";", b"ERROR invalid_utf8_byte\n"),
    (b".a = <uint : 8> 1;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8\nTYPE_FAMILY uint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END uint:8\nDATA number 1\n"),
    (b".a = <uint:8 ,_16> \"f\";", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8,_16\nTYPE_FAMILY uint:8,_16\nTYPE_PARAM_WIDTH 8\nTYPE_PARAM_BASE 16\nTYPE_ANN_END uint:8,_16\nDATA string f\n"),
    (b".a = <uint: 8> 1;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8\nTYPE_FAMILY uint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END uint:8\nDATA number 1\n"),
    (b".a = <uint:8,> 1;", b"ERROR illegal_value_type\n"),
    (b".a = <uint:8,_16,_16> \"f\";", b"ERROR illegal_value_type\n"),
    (b".a = <float_fix:16,q16> 0;", b"ERROR illegal_value_type\n"),
    (b".a = <float_dec:64> 1.25;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_dec:64\nTYPE_FAMILY float_dec:64\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END float_dec:64\nDATA number 1.25\n"),
    (b".a = <float_dec:64,_10> 1.25;", b"ERROR illegal_value_type\n"),
    (b".a = [1 EUR, 2 EUR];", b"ERROR unexpected_input_byte\n"),
    (b".a = <utf8,m> \"x\";", b"ERROR illegal_value_type\n"),
    (b".a = <float:64,no_unit> 1;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float:64,no_unit\nTYPE_FAMILY float:64,no_unit\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END float:64,no_unit\nDATA number 1\n"),
    (b".a = \x00\x01\x05\x00hel", b"ERROR read_complete_chunk_failed\n"),
    (b".a = \x00\x01\x00\x00\x00;", b"ERROR read_complete_chunk_failed\n"),
    (b".a = [\x00\x00];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nOCTET_STREAM_START\nOCTET_STREAM_END\nARRAY_ROW_END\n"),
    (b".a = <uint:8> \x00\x00;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8\nTYPE_FAMILY uint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END uint:8\nOCTET_STREAM_START\nOCTET_STREAM_END\n"),
    (b".a = <uint:32768> 1;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:32768\nTYPE_FAMILY uint:32768\nTYPE_PARAM_WIDTH 32768\nTYPE_ANN_END uint:32768\nDATA number 1\n"),
    (b".a = <uint:32769> 1;", b"ERROR illegal_value_type\n"),
    (b".a = <sint:32768> -1;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START sint:32768\nTYPE_FAMILY sint:32768\nTYPE_PARAM_WIDTH 32768\nTYPE_ANN_END sint:32768\nDATA number -1\n"),
    (b".a = <uint:4294967296> 1;", b"ERROR illegal_value_type\n"),
    (b".a = [1,[2]];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 1\nARRAY_ROW_START\nDATA array_number 2\nARRAY_ROW_END\nARRAY_ROW_END\n"),
    (b".a = [[1],2];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nARRAY_ROW_START\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 1\nARRAY_ROW_END\nDATA array_number 2\nARRAY_ROW_END\n"),
    (b".a = [1,2/];", b"ERROR unexpected_input_byte\n"),
    (b".a = [/1];", b"ERROR unexpected_input_byte\n"),
    (b".a = <uint:8> null;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8\nTYPE_FAMILY uint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END uint:8\nDATA null \n"),
    (b".a = <uint:8> [null, 1];", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8\nTYPE_FAMILY uint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END uint:8\nARRAY_ROW_START\nDATA null \nDATA array_number 1\nARRAY_ROW_END\n"),
    (b".a = nan;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float\nTYPE_FAMILY float\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END float\nDATA number nan\n"),
    (b".a = -inf;", b"ERROR unexpected_input_byte\n"),
    (b".a = ninf;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float\nTYPE_FAMILY float\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END float\nDATA number ninf\n"),
    (b".a = <float_fix:16,q8> ninf;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_fix:16,q8\nTYPE_FAMILY float_fix:16,q8\nTYPE_PARAM_WIDTH 16\nTYPE_PARAM_Q 8\nTYPE_ANN_END float_fix:16,q8\nDATA number ninf\n"),
    (b".a = <sint:8> ninf;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START sint:8\nTYPE_FAMILY sint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END sint:8\nDATA number ninf\n"),
    (b".a = [1 $EUR, 2 $EUR];", b"ERROR unexpected_input_byte\n"),
    (b".a = [1 $EUR, 2.5 $EUR];", b"ERROR unexpected_input_byte\n"),
    (b".a = nan $EUR;", b"ERROR unexpected_input_byte\n"),
    (b".a = [1, 2 $EUR];", b"ERROR unexpected_input_byte\n"),
    (b".a = [[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[1]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nARRAY_ROW_START\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 1\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\nARRAY_ROW_END\n"),
    (b".a = [[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[1]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]];", b"ERROR array_nesting_too_high\n"),
    (b".kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk = 1;", b"STREAM_START\nASSIGNMENT_START kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 1\n"),
    (b".kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk = 1;", b"ERROR identifier_too_long\n"),
    (b".a = <uint:128,_62> \"zzzzzzzzzzzzzzzzzzzzzzzzzzzzzz\";", b"ERROR value_out_of_range\n"),
    (b".a = <uint:128,_62> \"zzzzzzzzzzzzzzzzzzzzz\";", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:128,_62\nTYPE_FAMILY uint:128,_62\nTYPE_PARAM_WIDTH 128\nTYPE_PARAM_BASE 62\nTYPE_ANN_END uint:128,_62\nDATA string zzzzzzzzzzzzzzzzzzzzz\n"),
    (b".a = <uint:128,_85> \"~~~~~~~~~~~~~~~~~~~~\";", b"ERROR digit_not_in_base\n"),
    (b".a = <sint:200> -803469022129495137770981046170581301261101496891396417650688;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START sint:200\nTYPE_FAMILY sint:200\nTYPE_PARAM_WIDTH 200\nTYPE_ANN_END sint:200\nDATA number -803469022129495137770981046170581301261101496891396417650688\n"),
    (b".a = <sint:200> -803469022129495137770981046170581301261101496891396417650689;", b"ERROR value_out_of_range\n"),
    (b".a = <sint:200> 803469022129495137770981046170581301261101496891396417650688;", b"ERROR value_out_of_range\n"),
    (b".a = <float:64> 1e99999;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float:64\nTYPE_FAMILY float:64\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END float:64\nDATA number 1e99999\n"),
    (b".a = <float_fix:256,q128> 170141183460469231731687303715884105728;", b"ERROR value_out_of_range\n"),
    (b".a = <float_fix:256,q128> 170141183460469231731687303715884105727.9999;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_fix:256,q128\nTYPE_FAMILY float_fix:256,q128\nTYPE_PARAM_WIDTH 256\nTYPE_PARAM_Q 128\nTYPE_ANN_END float_fix:256,q128\nDATA number 170141183460469231731687303715884105727.9999\n"),
    (b".a = 1;\x0d\n.b = 2;\x0d\n", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 1\nASSIGNMENT_START b\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 2\n"),
    (b".a\x09=\x091\x09;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 1\n"),
    (b".a = 1 ; # c\n.b = 2; #", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 1\nASSIGNMENT_START b\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA number 2\n"),
    (b".a = \"x\"\n\"y\";", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START utf8\nTYPE_FAMILY utf8\nTYPE_ANN_END utf8\nDATA string xy\n"),
    (b".a = \"\\q\";", b"ERROR illegal_escape_sequence\n"),
    (b".a = \"\\n\\t\\\\\\\"\";", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START utf8\nTYPE_FAMILY utf8\nTYPE_ANN_END utf8\nDATA string \\x0a\\x09\\x5c\"\n"),
    (b".a = [1,\n2,\n3\n];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 1\nDATA array_number 2\nDATA array_number 3\nARRAY_ROW_END\n"),
    (b".a = [1 m, 2 m];", b"ERROR unexpected_input_byte\n"),
    (b".a = [1 m, 2 km];", b"ERROR unexpected_input_byte\n"),
    (b".a = [1 m, 2];", b"ERROR unexpected_input_byte\n"),
    (b".a = <uint:8> [[1,2],[3,256]];", b"ERROR value_out_of_range\n"),
    (b".a = [1,2/3,4/5,6];", b"ERROR unexpected_input_byte\n"),
    (b".a = [[1,2/3,4],[5,6/7,8]];", b"ERROR unexpected_input_byte\n"),
    (b".a = [1,2/3];", b"ERROR unexpected_input_byte\n"),
    (b".a = [<uint:8> 1 / <sint:8> 2];", b"ERROR unexpected_input_byte\n"),
    (b".a = <uint:8> {.b = 1;};", b"ERROR type_value_mismatch\n"),
    (b".a = <uint:8> 'x;", b"ERROR unexpected_input_byte\n"),
    (b".a = <uint:8> @x;", b"ERROR unexpected_input_byte\n"),
    (b".a = <uint:8> \"1\";", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8\nTYPE_FAMILY uint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END uint:8\nDATA string 1\n"),
    (b".a = <utf8> 1;", b"ERROR type_value_mismatch\n"),
    (b".a = <float:64> \"1.5\";", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float:64\nTYPE_FAMILY float:64\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END float:64\nDATA string 1.5\n"),
    (b".a = <float:64,_16> \"1.8\";", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float:64,_16\nTYPE_FAMILY float:64,_16\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 16\nTYPE_ANN_END float:64,_16\nDATA string 1.8\n"),
    (b".a = <float:16,_16> 0x1p-30;", b"ERROR unexpected_input_byte\n"),
    (b".a = 0x10;", b"ERROR unexpected_input_byte\n"),
    (b".a = 1.;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float\nTYPE_FAMILY float\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END float\nDATA number 1.\n"),
    (b".a = .5;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float\nTYPE_FAMILY float\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END float\nDATA number .5\n"),
    (b".a = 1e+;", b"ERROR unexpected_input_byte\n"),
    (b".a = -;", b"ERROR unexpected_input_byte\n"),
    (b".a = --1;", b"ERROR unexpected_input_byte\n"),
    (b".a = 1.2.3;", b"ERROR unexpected_input_byte\n"),
    (b".a = 1e5e5;", b"ERROR unexpected_input_byte\n"),
    (b".a = TRUE;", b"STREAM_START\nASSIGNMENT_START a\nDATA symbol TRUE\n"),
    (b".a = <bool> null;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START bool\nTYPE_FAMILY bool\nTYPE_ANN_END bool\nDATA null \n"),
    (b".a = [true, false, on, off];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nTYPE_ANN_START bool\nTYPE_FAMILY bool\nTYPE_ANN_END bool\nDATA bool true\nDATA bool false\nDATA bool true\nDATA bool false\nARRAY_ROW_END\n"),
    (b".a = [true, 1];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nTYPE_ANN_START bool\nTYPE_FAMILY bool\nTYPE_ANN_END bool\nDATA bool true\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 1\nARRAY_ROW_END\n"),
    (b".a = [\"a\", 1];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nTYPE_ANN_START utf8\nTYPE_FAMILY utf8\nTYPE_ANN_END utf8\nDATA array_string a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 1\nARRAY_ROW_END\n"),
    (b"#!bovnar 1.1\n.a = [1, 2026-01-01T00:00:00Z];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT no_unit\nTYPE_ANN_END uint\nDATA array_number 1\nTYPE_ANN_START datetime\nTYPE_FAMILY datetime\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END datetime\nDATA array_number 1767225600\nARRAY_ROW_END\n"),
    (b"#!bovnar 1.1\n.a = <datetime> -1;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START datetime\nTYPE_FAMILY datetime\nTYPE_ANN_END datetime\nDATA number -1\n"),
    (b"#!bovnar 1.1\n.a = <datetime:8> 200;", b"ERROR value_out_of_range\n"),
    (b"#!bovnar 1.1\n.a = <datetime:64> 1.5;", b"ERROR type_value_mismatch\n"),
    (b"#!bovnar 1.1\n.a = <datetime:64> \"1\";", b"ERROR type_value_mismatch\n"),
    (b"#!bovnar 1.1\n.a = 2026-06-15T12:00:00;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START datetime\nTYPE_FAMILY datetime\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END datetime\nDATA number 1781524800\n"),
    (b"#!bovnar 1.1\n.a = 2026-06-15;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START datetime\nTYPE_FAMILY datetime\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END datetime\nDATA number 1781481600\n"),
    (b"#!bovnar 1.1\n.a = 2026-06-15T24:00:00Z;", b"ERROR invalid_datetime_literal\n"),
    (b"#!bovnar 1.1\n.a = 0000-01-01T00:00:00Z;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START datetime\nTYPE_FAMILY datetime\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END datetime\nDATA number -62167219200\n"),
    (b"#!bovnar 1.1\n.a = 9999-12-31T23:59:59.999999999-14:00;", b"ERROR invalid_datetime_literal\n"),
    (b"#!bovnar 1.1\n.a = 2026-06-15T12:00:00Z m;", b"ERROR unit_illegal\n"),
    (b"#!bovnar 1.1\n.a = <datetime:64,tai> [2016-12-31T23:59:60Z, 2017-01-01T00:00:00Z];", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START datetime:64,tai\nTYPE_FAMILY datetime:64,tai\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_UNIT tai\nTYPE_ANN_END datetime:64,tai\nARRAY_ROW_START\nDATA array_number 1861920036\nDATA array_number 1861920037\nARRAY_ROW_END\n"),
    (b"#!bovnar 1.1\n.a = <datetime:64,mjd> 2026-06-15T12:00:00Z;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START datetime:64,mjd\nTYPE_FAMILY datetime:64,mjd\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_UNIT mjd\nTYPE_ANN_END datetime:64,mjd\nDATA number 5288241600\n"),
    (b"#!bovnar 1.1\n.a = <datetime:64,unix> 1 m;", b"ERROR unit_illegal\n"),
    (b"#!bovnar 1.1\n.a = [<datetime:64,tai> 1, 2026-01-01T00:00:00Z];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nTYPE_ANN_START datetime:64,tai\nTYPE_FAMILY datetime:64,tai\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_UNIT tai\nTYPE_ANN_END datetime:64,tai\nDATA array_number 1\nTYPE_ANN_START datetime\nTYPE_FAMILY datetime\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END datetime\nDATA array_number 1767225600\nARRAY_ROW_END\n"),
    (b".a = 1 ucum:m;", b"ERROR unit_illegal\n"),
    (b"#!bovnar 1.2\n.a = <utf8,ucum:m> \"x\";", b"ERROR illegal_value_type\n"),
    (b".a = \x00\x01\x80\x00xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\x00;", b"STREAM_START\nASSIGNMENT_START a\nOCTET_STREAM_START\nDATA octets 128 bytes\nOCTET_STREAM_END\n"),
    (b".a = \x00\x01\x00\x01xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\x00;", b"STREAM_START\nASSIGNMENT_START a\nOCTET_STREAM_START\nDATA octets 256 bytes\nOCTET_STREAM_END\n"),
    (b".a = [\x00\x01\x01\x00x\x00];", b"STREAM_START\nASSIGNMENT_START a\nARRAY_ROW_START\nOCTET_STREAM_START\nDATA octets 1 bytes\nOCTET_STREAM_END\nARRAY_ROW_END\n"),
    (b".a = {.b = \x00\x01\x01\x00x\x00;};", b"STREAM_START\nASSIGNMENT_START a\nSTRUCT_START\nASSIGNMENT_START b\nOCTET_STREAM_START\nDATA octets 1 bytes\nOCTET_STREAM_END\nSTRUCT_END\n"),
    (b".a = \x00\x01\x01\x00x\x01\x00\x00\x00;", b"ERROR read_complete_chunk_failed\n"),
    (b".a = \x00\x01\x01\x00x\x00 ;", b"STREAM_START\nASSIGNMENT_START a\nOCTET_STREAM_START\nDATA octets 1 bytes\nOCTET_STREAM_END\n"),
    (b".a = \x00\x01\x01\x00x\x00", b"ERROR incomplete_bvnr_stream\n"),
    (
        b".a = <float:64,{{{{{{{{{{{{{{{{{m}}}}}}}}}}}}}}}}},_99> 1;",
        b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float:64,{{{{{{{{{{{{{{{{{m}}}}}}}}}}}}}}}}},_99\nTYPE_FAMILY float:64,{{{{{{{{{{{{{{{{{m}}}}}}}}}}}}}}}}},_99\nTYPE_PARAM_WIDTH 64\nTYPE_ANN_END float:64,{{{{{{{{{{{{{{{{{m}}}}}}}}}}}}}}}}},_99\nDATA number 1\n",
    ),
    (
        b".a = <float_fix:16,q8> 1e10000;",
        b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_fix:16,q8\nTYPE_FAMILY float_fix:16,q8\nTYPE_PARAM_WIDTH 16\nTYPE_PARAM_Q 8\nTYPE_ANN_END float_fix:16,q8\nDATA number 1e10000\n",
    ),
    (b".a = <float_fix:16,q8> 1e9000;", b"ERROR value_out_of_range\n"),
];

/// Inputs whose outcome depends on the unit system (inline units, unit
/// parameters, currencies).
const UNIT_CASES: &[(&[u8], &[u8])] = &[
    (b".a = <float_fix:32,q8,m> 1.5;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_fix:32,q8,m\nTYPE_FAMILY float_fix:32,q8,m\nTYPE_PARAM_WIDTH 32\nTYPE_PARAM_Q 8\nTYPE_PARAM_UNIT m\nTYPE_ANN_END float_fix:32,q8,m\nDATA number 1.5\n"),
    (b".a = 1 m;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint\nTYPE_FAMILY uint\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT m\nTYPE_ANN_END uint\nDATA number 1\n"),
    (b".a = 1.5 km/h;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float\nTYPE_FAMILY float\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_BASE 10\nTYPE_PARAM_UNIT km/h\nTYPE_ANN_END float\nDATA number 1.5\n"),
    (b".a = <float:64,m> 1.5 km;", b"ERROR unit_mismatch\n"),
    (b".a = <float:64,m> 1.5 m;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float:64,m\nTYPE_FAMILY float:64,m\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_UNIT m\nTYPE_ANN_END float:64,m\nDATA number 1.5\n"),
    (b".a = 12.50 EUR;", b"ERROR unit_illegal\n"),
    (b".a = 12 EUR;", b"ERROR unit_illegal\n"),
    (b".a = <float:64,no_unit> 1 m;", b"ERROR unit_mismatch\n"),
    (b".a = \"x\" m;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START utf8\nTYPE_FAMILY utf8\nTYPE_ANN_END utf8\nDATA string x\n"),
    (b".a = 12 $EUR;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_dec\nTYPE_FAMILY float_dec\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_UNIT $EUR\nTYPE_ANN_END float_dec\nDATA number 12\n"),
    (b".a = 12.50 $USD;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_dec\nTYPE_FAMILY float_dec\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_UNIT $USD\nTYPE_ANN_END float_dec\nDATA number 12.50\n"),
    (b".a = <float_dec:64,$USD> 1.0;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_dec:64,$USD\nTYPE_FAMILY float_dec:64,$USD\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_UNIT $USD\nTYPE_ANN_END float_dec:64,$USD\nDATA number 1.0\n"),
    (b".a = 1 k$USD;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_dec\nTYPE_FAMILY float_dec\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_UNIT k$USD\nTYPE_ANN_END float_dec\nDATA number 1\n"),
    (b".a = -3 $EUR;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START float_dec\nTYPE_FAMILY float_dec\nTYPE_PARAM_WIDTH 64\nTYPE_PARAM_UNIT $EUR\nTYPE_ANN_END float_dec\nDATA number -3\n"),
    (b".a = <float:64,m> [1, 2 m];", b"ERROR unexpected_input_byte\n"),
    (b".a = <float:64,m> [1, 2 s];", b"ERROR unexpected_input_byte\n"),
    (b".a = [<float:64,m> 1, 2 s];", b"ERROR unexpected_input_byte\n"),
    (b"#!bovnar 1.2\n.a = 1 ucum:m;", b"ERROR unit_profile_unknown\n"),
    (b"#!bovnar 1.2\n.a = <float:64,cf:m> 1;", b"ERROR unit_profile_unknown\n"),
    (b".a = <uint:8> 1 m;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8\nTYPE_FAMILY uint:8\nTYPE_PARAM_WIDTH 8\nTYPE_ANN_END uint:8\nDATA number 1\n"),
    (b".a = <uint:8,m> 1 m;", b"STREAM_START\nASSIGNMENT_START a\nTYPE_ANN_START uint:8,m\nTYPE_FAMILY uint:8,m\nTYPE_PARAM_WIDTH 8\nTYPE_PARAM_UNIT m\nTYPE_ANN_END uint:8,m\nDATA number 1\n"),
    (b".a = <uint:8,m> 1 km;", b"ERROR unit_mismatch\n"),
];

fn log_or_error(input: &[u8]) -> Vec<u8> {
    match conformance::event_log(input, &Limits::default()) {
        Ok(log) => log.into_bytes(),
        Err(err) => format!("ERROR {}\n", err.kind().as_str()).into_bytes(),
    }
}

fn check(cases: &[(&[u8], &[u8])]) {
    let mut failures = Vec::new();
    for &(input, expected) in cases {
        let got = log_or_error(input);
        if got != expected {
            failures.push(format!(
                "{}\n  expected: {}\n  got:      {}",
                String::from_utf8_lossy(input).escape_debug(),
                String::from_utf8_lossy(expected).escape_debug(),
                String::from_utf8_lossy(&got).escape_debug(),
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn matches_c_reference() {
    check(CASES);
}

#[test]
fn matches_c_reference_with_units() {
    check(UNIT_CASES);
}

/// Feeds `input` in chunks of `size` bytes and records every event and the
/// final outcome.
fn events_in_chunks(input: &[u8], size: usize) -> Vec<String> {
    let mut reader = Reader::new(&Limits::default());
    let mut seen = Vec::new();
    let mut record = |reader: &mut Reader| {
        while let Some(event) = reader.next_event() {
            seen.push(format!("{event:?}"));
        }
    };
    for chunk in input.chunks(size) {
        let result = reader.feed(chunk);
        record(&mut reader);
        if let Err(err) = result {
            seen.push(format!("error {}", err.kind().as_str()));
            return seen;
        }
    }
    let result = reader.finish();
    record(&mut reader);
    if let Err(err) = result {
        seen.push(format!("error {}", err.kind().as_str()));
    }
    seen
}

#[test]
fn chunking_does_not_change_the_events() {
    for &(input, _) in CASES.iter().chain(UNIT_CASES) {
        let whole = events_in_chunks(input, input.len().max(1));
        for size in [1, 2, 3, 7] {
            assert_eq!(
                events_in_chunks(input, size),
                whole,
                "chunk size {size} for {}",
                String::from_utf8_lossy(input).escape_debug()
            );
        }
    }
}

#[test]
fn structured_annotation_and_octets() {
    let input = b"#!bovnar 1.1\n.t = <datetime:64,tai> 2026-06-15T12:00:00.5Z; .b = \x00\x01\x03\x00abc\x00;";
    let mut reader = Reader::new(&Limits::default());
    reader.feed(input).unwrap();
    reader.finish().unwrap();
    assert_eq!(reader.declared_version(), Some(bovnar::SpecVersion::V1_1));
    let mut annotation = None;
    let mut frac = Vec::new();
    let mut chunk = Vec::new();
    while let Some(event) = reader.next_event() {
        match event {
            Event::TypeAnnotation(ann) => {
                annotation = Some((ann.family, ann.width, ann.epoch, ann.synthesized))
            }
            Event::Data { frac: f, .. } if !f.is_empty() => frac = f.to_vec(),
            Event::OctetStreamChunk(data) => chunk = data.to_vec(),
            _ => {}
        }
    }
    assert_eq!(
        annotation,
        Some((bovnar::TypeFamily::Datetime, Some(64), Some("tai"), false))
    );
    assert_eq!(frac, b"5");
    assert_eq!(chunk, b"abc");
}

#[test]
fn recovery_mode_skips_a_broken_assignment() {
    let mut reader = Reader::with_recovery(&Limits::default());
    reader.feed(b".a = 1; .b = ?; .c = 2;").unwrap();
    reader.finish().unwrap();
    let mut keys = Vec::new();
    let mut recovered = Vec::new();
    while let Some(event) = reader.next_event() {
        match event {
            Event::AssignmentStart { key } => keys.push(key.to_vec()),
            Event::Recovered(err) => recovered.push(err.kind()),
            _ => {}
        }
    }
    assert_eq!(keys, [b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    assert_eq!(recovered, [bovnar::ErrorKind::UnexpectedInputByte]);
    assert_eq!(reader.recovery_count(), 1);
}
