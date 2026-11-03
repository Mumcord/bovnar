/*
 * Replays textual event sequences through the C reference writer
 * (bvnr_write_event) and prints, per sequence, the output or the first
 * error. Its twin is bovnar-writer-diff, which drives bovnar::Writer with the
 * same sequences; scripts/writer-diff.sh builds this file against the C
 * reference library and compares the two.
 *
 * One sequence per input line, events separated by single spaces, fields of
 * an event separated by ':' and percent-encoded (%XX) where they contain
 * '%', ':', a space or a non-printable byte. An empty field means "absent".
 *
 *   v:M.N                     bvnr_write_version
 *   ss  se                    stream start / end
 *   k:KEY                     assignment start
 *   a:FAM:W:B:Q:EPOCH:UNIT    a type annotation, sent as the C reader sends
 *                             one: start, family, one parameter event per
 *                             field present (in this order), end
 *   d:KIND:TEXT:FRAC:UNIT     a value; KIND is num, anum, str, astr, sym, ref,
 *                             bool or null; UNIT is its inline unit
 *   {  }  [  ]  /             struct start / end, row start / end, dimension
 *   o(  o:BYTES  o)           octet stream start, chunk, end
 *
 * Every sequence ends with bvnr_write_finish. Output, one line each:
 *   ok <hex of the bytes written>
 *   err <error name> <index of the failing event; the event count for finish>
 */
#include <bovnar.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAX_FIELDS 8
#define OUT_CAP (16u << 20)

typedef struct field_s {
	unsigned char *p;
	uint32_t len;
	bool present;
} field_t;

static int hexval(int c)
{
	if (c >= '0' && c <= '9') return c - '0';
	if (c >= 'a' && c <= 'f') return c - 'a' + 10;
	if (c >= 'A' && c <= 'F') return c - 'A' + 10;
	return -1;
}

/* Decodes %XX in place; returns the decoded length. */
static uint32_t decode(unsigned char *s, uint32_t len)
{
	uint32_t o = 0;
	for (uint32_t i = 0; i < len; i++) {
		if (s[i] == '%' && i + 2 < len &&
		    hexval(s[i + 1]) >= 0 && hexval(s[i + 2]) >= 0) {
			s[o++] = (unsigned char)(hexval(s[i + 1]) * 16 + hexval(s[i + 2]));
			i += 2;
		} else {
			s[o++] = s[i];
		}
	}
	return o;
}

static int split(unsigned char *ev, uint32_t len, field_t *f)
{
	int n = 0;
	uint32_t start = 0;
	for (uint32_t i = 0; i <= len && n < MAX_FIELDS; i++) {
		if (i == len || ev[i] == ':') {
			f[n].p = ev + start;
			f[n].len = decode(ev + start, i - start);
			f[n].present = f[n].len > 0;
			n++;
			start = i + 1;
		}
	}
	return n;
}

static bool eq(const field_t *f, const char *s)
{
	return f->len == strlen(s) && memcmp(f->p, s, f->len) == 0;
}

static uint32_t num(const field_t *f)
{
	char buf[16] = {0};
	memcpy(buf, f->p, f->len < 15 ? f->len : 15);
	return (uint32_t)strtoul(buf, NULL, 10);
}

static value_type_family_t family(const field_t *f)
{
	static const char *names[] = {"plain", "utf8", "sint", "uint", "float",
		"float_fix", "float_dec", "bool", "datetime"};
	for (int i = 0; i < 9; i++)
		if (eq(f, names[i]))
			return (value_type_family_t)i;
	return vt_illegal;
}

static token_type_t kind(const field_t *f)
{
	if (eq(f, "num"))  return token_is_number;
	if (eq(f, "anum")) return token_is_array_number;
	if (eq(f, "str"))  return token_is_string;
	if (eq(f, "astr")) return token_is_array_string;
	if (eq(f, "sym"))  return token_is_symbol;
	if (eq(f, "ref"))  return token_is_reference;
	if (eq(f, "bool")) return token_is_bool;
	if (eq(f, "null")) return token_is_null_value;
	return token_is_unknown;
}

/* The unit a spelling names, as the reader would fill value_unit. A spelling
 * that does not parse gets a stand-in unit so the writer still checks the
 * spelling (bvnr_data_t.unit_source) and reports why it is wrong. */
static value_unit_t unit_of(const field_t *f)
{
	if (!f->present)
		return BVN_UNIT_NONE;
	bool ok = false;
	value_unit_t u = bvn_parse_unit_n(f->p, f->len, &ok);
	if (!ok)
		return BVN_UNIT_NO_PREFIX(BVN_UNIT_ID_FIRST);
	return u;
}

static bool annotation(bvnr_writer_t *w, field_t *f, int n)
{
	field_t none = {0};
	field_t *fam = n > 1 ? &f[1] : &none, *wid = n > 2 ? &f[2] : &none,
		*base = n > 3 ? &f[3] : &none, *q = n > 4 ? &f[4] : &none,
		*epoch = n > 5 ? &f[5] : &none, *unit = n > 6 ? &f[6] : &none;
	bvnr_data_t d = {0};
	d.type = token_is_type;
	d.value_type.family = family(fam);
	d.value_type.width = wid->present ? num(wid) : 0;
	if (base->present) d.value_type.base = num(base);
	if (q->present) d.value_type.base = num(q);
	if (epoch->present) {
		char name[64] = {0};
		memcpy(name, epoch->p, epoch->len < 63 ? epoch->len : 63);
		int32_t idx = bvnr_datetime_epoch_index(name);
		d.value_type.base = idx < 0 ? 0u : (uint32_t)idx;
	}
	d.value_unit = unit_of(unit);
	if (!bvnr_write_event(w, ev_type_annotation_start, &d)) return false;
	if (!bvnr_write_event(w, ev_type_annotation_type_family, &d)) return false;
	if (wid->present) {
		bvnr_data_t p = d;
		p.type = token_is_type_width;
		if (!bvnr_write_event(w, ev_type_annotation_type_family_parameter, &p))
			return false;
	}
	if (base->present) {
		bvnr_data_t p = d;
		p.type = token_is_type_base;
		if (!bvnr_write_event(w, ev_type_annotation_type_family_parameter, &p))
			return false;
	}
	if (q->present) {
		bvnr_data_t p = d;
		p.type = token_is_type_q;
		if (!bvnr_write_event(w, ev_type_annotation_type_family_parameter, &p))
			return false;
	}
	if (epoch->present) {
		bvnr_data_t p = d;
		p.type = token_is_unit;
		p.value_unit = BVN_UNIT_NONE;
		p.data = epoch->p;
		p.length = epoch->len;
		if (!bvnr_write_event(w, ev_type_annotation_type_family_parameter, &p))
			return false;
	}
	if (unit->present) {
		bvnr_data_t p = d;
		p.type = token_is_unit;
		p.data = unit->p;
		p.length = unit->len;
		p.unit_source = unit->p;
		p.unit_source_length = unit->len;
		if (!bvnr_write_event(w, ev_type_annotation_type_family_parameter, &p))
			return false;
	}
	return bvnr_write_event(w, ev_type_annotation_end, &d);
}

static bool data(bvnr_writer_t *w, field_t *f, int n)
{
	field_t none = {0};
	field_t *k = n > 1 ? &f[1] : &none, *text = n > 2 ? &f[2] : &none,
		*frac = n > 3 ? &f[3] : &none, *unit = n > 4 ? &f[4] : &none;
	bvnr_data_t d = {0};
	d.type = kind(k);
	d.value_type = BVN_TYPE_PLAIN;
	d.data = text->len ? (const void *)text->p : (const void *)"";
	d.length = text->len;
	if (frac->present) {
		d.frac_data = frac->p;
		d.frac_length = frac->len;
	}
	if (unit->present && !eq(unit, "no_unit")) {
		d.value_unit = unit_of(unit);
		d.unit_source = unit->p;
		d.unit_source_length = unit->len;
	}
	return bvnr_write_event(w, ev_data, &d);
}

static bool simple(bvnr_writer_t *w, bvnr_event_t ev)
{
	bvnr_data_t d = {0};
	return bvnr_write_event(w, ev, &d);
}

static bool event(bvnr_writer_t *w, unsigned char *ev, uint32_t len)
{
	field_t f[MAX_FIELDS];
	int n = split(ev, len, f);
	if (eq(&f[0], "v")) {
		unsigned maj = 0, min = 0;
		char buf[32] = {0};
		memcpy(buf, f[1].p, f[1].len < 31 ? f[1].len : 31);
		sscanf(buf, "%u.%u", &maj, &min);
		return bvnr_write_version(w, (uint16_t)maj, (uint16_t)min);
	}
	if (eq(&f[0], "ss")) return simple(w, ev_stream_start);
	if (eq(&f[0], "se")) return simple(w, ev_stream_end);
	if (eq(&f[0], "{"))  return simple(w, ev_struct_start);
	if (eq(&f[0], "}"))  return simple(w, ev_struct_end);
	if (eq(&f[0], "["))  return simple(w, ev_array_row_start);
	if (eq(&f[0], "]"))  return simple(w, ev_array_row_end);
	if (eq(&f[0], "/"))  return simple(w, ev_array_dim_start);
	if (eq(&f[0], "o(")) return simple(w, ev_octet_stream_start);
	if (eq(&f[0], "o)")) return simple(w, ev_octet_stream_end);
	if (eq(&f[0], "k")) {
		bvnr_data_t d = {0};
		d.type = token_is_identifier;
		d.data = n > 1 && f[1].len ? (const void *)f[1].p : (const void *)"";
		d.length = n > 1 ? f[1].len : 0;
		return bvnr_write_event(w, ev_assignment_start, &d);
	}
	if (eq(&f[0], "o")) {
		bvnr_data_t d = {0};
		d.type = token_is_octet_stream;
		d.data = n > 1 && f[1].len ? (const void *)f[1].p : (const void *)"";
		d.length = n > 1 ? f[1].len : 0;
		return bvnr_write_event(w, ev_data, &d);
	}
	if (eq(&f[0], "a")) return annotation(w, f, n);
	if (eq(&f[0], "d")) return data(w, f, n);
	fprintf(stderr, "unknown event %.*s\n", (int)f[0].len, f[0].p);
	exit(2);
}

int main(void)
{
	static unsigned char out[OUT_CAP];
	char *line = NULL;
	size_t cap = 0;
	ssize_t got;
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) return 1;
	while ((got = getline(&line, &cap, stdin)) >= 0) {
		if (got > 0 && line[got - 1] == '\n') line[--got] = '\0';
		if (!bvnr_open_write_mem(w, out, OUT_CAP, true, NULL)) return 1;
		unsigned idx = 0;
		bool ok = true;
		char *save = NULL;
		for (char *ev = strtok_r(line, " ", &save); ev;
		     ev = strtok_r(NULL, " ", &save), idx++) {
			if (!event(w, (unsigned char *)ev, (uint32_t)strlen(ev))) {
				ok = false;
				break;
			}
		}
		if (ok && !bvnr_write_finish(w))
			ok = false;
		if (ok) {
			uint64_t n = bvnr_writer_bytes_written(w);
			fputs("ok ", stdout);
			for (uint64_t i = 0; i < n; i++)
				printf("%02x", out[i]);
			putchar('\n');
		} else {
			printf("err %s %u\n",
				bvn_error_to_string(bvnr_writer_get_error(w)), idx);
		}
	}
	free(line);
	bvnr_writer_destroy(w);
	return 0;
}
