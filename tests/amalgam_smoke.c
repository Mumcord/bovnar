/*
 * SPDX-License-Identifier: MIT
 *
 * Copyright (c) 2026 Janos Sonntag
 *
 * Smoke test for the single-file amalgamation (dist/bovnar.h + dist/bovnar.c).
 *
 * Compiled and run by cmake/amalgam_smoke.cmake (CTest target bvnr_amalgam),
 * which first regenerates dist/ via amalgamate.py. It exercises a value across
 * several subsystems (lexer, validator, DOM, units, big-int, special floats,
 * the streaming writer + sink, and the bovnar_stream framing/mux layer) so a
 * broken amalgamation fails loudly rather than silently rotting.
 *
 * It is compiled at -O2: the source/sink opaque-blob type-punning is only
 * miscompiled by TBAA when the optimiser can see the whole flow in one TU, so a
 * non-optimised smoke build would miss exactly that class of amalgamation bug.
 */
#include "bovnar.h"   /* amalgamated single header — includes the stream API */
#include <stdio.h>
#include <string.h>

#define CHECK(cond, msg) do { if (!(cond)) { \
	printf("amalgam_smoke FAIL: %s\n", (msg)); return 1; } } while (0)

int main(void)
{
	const char *src =
		".speed = <float:64,m/s> 9.81;\n"
		".accel = 9.81 (m/s)/s;\n"            /* inline leading-paren unit */
		".big   = <uint:128> 340282366920938463463374607431768211455;\n"
		".inf   = inf;\n"
		".ninf  = ninf;\n"
		".price = <float_dec:64,$USD> 19.99;\n";

	uint32_t n = (uint32_t)strlen(src);
	bvn_dom_doc_t *d = bvn_dom_parse(src, n);
	CHECK(d != NULL, "bvn_dom_parse returned NULL");
	CHECK(bvn_dom_doc_get_parse_error(d) == error_none, "unexpected parse error");

	bvn_dom_node_t *speed = bvn_dom_lookup(d, ".speed");
	double f = 0.0;
	CHECK(speed && bvn_dom_get_float(speed, &f) && f > 9.8 && f < 9.82, ".speed value");

	bvn_dom_node_t *inf = bvn_dom_lookup(d, ".inf");
	double iv = 0.0;
	CHECK(inf && bvn_dom_get_float(inf, &iv) && iv > 1e308, ".inf value");

	bvn_dom_node_t *big = bvn_dom_lookup(d, ".big");
	CHECK(big != NULL, ".big present");

	/* A document the validator must reject (mixed-kind array) still parses to a
	 * doc but records the error, confirming the validator is wired in. */
	const char *bad = ".x = [1, \"two\"];\n";
	bvn_dom_doc_t *bd = bvn_dom_parse(bad, (uint32_t)strlen(bad));
	CHECK(bd != NULL, "bad-doc parse NULL");
	CHECK(bvn_dom_doc_get_parse_error(bd) == error_array_element_type_mismatch,
	      "expected array_element_type_mismatch");

	bvn_dom_doc_destroy(d);
	bvn_dom_doc_destroy(bd);

	/* Streaming writer + memory sink round-trip. This is the path that the
	 * source/sink TBAA bug miscompiled at -O2 in the amalgam: a memory sink
	 * whose is_mem/mem_left stores were dropped reports error_writing_to_sink
	 * and emits nothing. */
	{
		unsigned char buf[256];
		bvnr_writer_t *w = bvnr_writer_create();
		bvnr_data_t hdr = {0};
		bool ok = w
			&& bvnr_open_write_mem(w, buf, sizeof buf, false, NULL)
			&& bvnr_write_event(w, ev_stream_start, &hdr)
			&& bvnr_write_uint(w, "x", 32, 42)
			&& bvnr_write_finish(w);
		uint64_t n = ok ? bvnr_writer_bytes_written(w) : 0;
		CHECK(ok, "streaming writer round-trip failed");
		CHECK(n > 0 && n < sizeof buf, "writer produced no output");
		buf[n] = '\0';
		CHECK(strstr((char *)buf, ".x=<uint:32>42;") != NULL,
		      "writer output mismatch");
		bvnr_writer_destroy(w);
	}

	/* bovnar_stream layer: octet multiplexing round-trip through the amalgam. */
	{
		unsigned char buf[256];
		bvnr_writer_t *w = bvnr_writer_create();
		bvnr_data_t hdr = {0};
		bool wok = w
			&& bvnr_open_write_mem(w, buf, sizeof buf, false, NULL)
			&& bvnr_write_event(w, ev_stream_start, &hdr)
			&& bvnr_mux_begin(w, "m")
			&& bvnr_mux_send(w, 3, "hello", 5)
			&& bvnr_mux_end(w)
			&& bvnr_write_finish(w);
		uint64_t n = wok ? bvnr_writer_bytes_written(w) : 0;
		CHECK(wok, "mux producer failed");
		bvnr_writer_destroy(w);

		/* Validate-only demux (NULL callback): reassembles and checks framing
		 * without delivering, which is enough to confirm the round-trip and the
		 * cross-chunk reassembly path link and run in the amalgam. */
		bvnr_demux_t *dm = bvnr_demux_create(NULL, NULL, 0);
		CHECK(dm != NULL, "demux create failed");
		bvnr_read_flags_t fl; memset(&fl, 0, sizeof fl);
		fl.on_verified = bvnr_demux_on_event;
		fl.userdata    = dm;
		bvnr_reader_t *r = bvnr_reader_create();
		bool rok = r && bvnr_open_read_mem(r, buf, n, NULL, 0, &fl) && bvnr_read(r);
		CHECK(rok, "mux consumer failed");
		CHECK(bvnr_demux_error(dm) == error_none, "demux desync");
		bvnr_reader_destroy(r);
		bvnr_demux_destroy(dm);
	}

	printf("amalgam_smoke OK\n");
	return 0;
}
