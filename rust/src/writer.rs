//! The streaming writer, mirroring `python/bovnar/writer.py`.

use crate::enums::{Event, TokenType, ValueTypeFamily};
use crate::error::{Error, Result};
use crate::ffi::{self, BvnrWriteFlags, ValueTypeSpec};

/// Default size of the memory sink (Python's `DEFAULT_MEM_CAP`).
pub const DEFAULT_MEM_CAP: usize = 4 * 1024 * 1024;

/// Largest octet-stream chunk: the most one chunk header can count.
const OCTET_CHUNK: usize = 65536;

/// A streaming BVNR writer into memory.
pub struct Writer {
    handle: ffi::WriterHandle,
    struct_depth: u32,
    finished: bool,
}

impl Writer {
    /// A writer into a memory buffer of `cap` bytes; `pretty` indents the
    /// output. Writing past `cap` fails with `sink_buffer_exhausted`.
    pub fn to_mem(cap: usize, pretty: bool) -> Result<Self> {
        let handle = ffi::WriterHandle::new()
            .ok_or_else(|| Error::Argument("bvnr_writer_create() returned NULL".into()))?;
        let mut w = Writer {
            handle,
            struct_depth: 0,
            finished: false,
        };
        let mut flags = BvnrWriteFlags::zeroed();
        // The writer matches what the reader reads: the 255-level cap in both
        // directions, where zero would select the C default of 64.
        flags.max_struct_nesting = 255;
        flags.max_array_nesting = 255;
        if !w.handle.open_mem(cap, pretty, flags) {
            return Err(w.error());
        }
        Ok(w)
    }

    /// Sends one raw event, like Python's `Writer.emit`: `key` for an
    /// assignment, `value` (a literal) with its type `vt` for data.
    pub fn emit(
        &mut self,
        event: Event,
        key: Option<&str>,
        value: Option<&str>,
        vt: Option<ValueTypeSpec>,
    ) -> Result<()> {
        let vt = vt.unwrap_or_default();
        let (token, data) = match (value, key) {
            (Some(v), _) => {
                let family = ValueTypeFamily::from_raw(vt.family);
                let decimal_float = matches!(
                    family,
                    Some(ValueTypeFamily::FloatFix | ValueTypeFamily::FloatDec)
                );
                let token = if family == Some(ValueTypeFamily::Utf8)
                    || (vt.base != 0 && vt.base != 10 && !decimal_float)
                {
                    TokenType::String
                } else {
                    TokenType::Number
                };
                (token, v.as_bytes())
            }
            (None, Some(k)) => (TokenType::Identifier, k.as_bytes()),
            (None, None) => (TokenType::Identifier, &[][..]),
        };
        self.event(event, token, vt, data)
    }

    /// Emits a leading `#!bovnar <major>.<minor>` directive; must come before
    /// any value.
    pub fn write_version(&mut self, major: u16, minor: u16) -> Result<()> {
        self.check(|h| h.write_version(major, minor))
    }

    /// `.key = <uint:width[,_base]> value;`
    pub fn write_uint(&mut self, key: &str, value: u64, width: u32, base: u32) -> Result<()> {
        let vt = type_spec(ValueTypeFamily::Uint, width, base);
        let text = ffi::format_u64(value, base)
            .ok_or_else(|| Error::Argument(format!("cannot format {value} in base {base}")))?;
        self.write_scalar(key, vt, &text)
    }

    /// `.key = <sint:width[,_base]> value;`
    pub fn write_sint(&mut self, key: &str, value: i64, width: u32, base: u32) -> Result<()> {
        if base == 64 || base == 85 {
            return Err(Error::Argument(format!(
                "base {base} is unsigned: its alphabet uses '+' and '-' as digits, \
                 so a signed integer cannot be written in it"
            )));
        }
        let vt = type_spec(ValueTypeFamily::Sint, width, base);
        let text = ffi::format_i64(value, base)
            .ok_or_else(|| Error::Argument(format!("cannot format {value} in base {base}")))?;
        self.write_scalar(key, vt, &text)
    }

    /// `.key = <float:width> value;`
    pub fn write_float(&mut self, key: &str, value: f64, width: u32) -> Result<()> {
        let vt = type_spec(ValueTypeFamily::Float, width, 0);
        let text = ffi::format_f64(value, vt)
            .ok_or_else(|| Error::Argument(format!("cannot format {value}")))?;
        self.write_scalar(key, vt, &text)
    }

    /// `.key = "value";`
    pub fn write_string(&mut self, key: &str, value: &str) -> Result<()> {
        self.assignment(key)?;
        self.event(
            Event::Data,
            TokenType::String,
            ValueTypeSpec::default(),
            value.as_bytes(),
        )
    }

    /// `.key = true;` or `.key = false;`
    pub fn write_bool(&mut self, key: &str, value: bool) -> Result<()> {
        self.assignment(key)?;
        let text: &[u8] = if value { b"true" } else { b"false" };
        self.event(Event::Data, TokenType::Bool, ValueTypeSpec::default(), text)
    }

    /// `.key = null;`
    pub fn write_null(&mut self, key: &str) -> Result<()> {
        self.assignment(key)?;
        self.event(
            Event::Data,
            TokenType::NullValue,
            ValueTypeSpec::default(),
            &[],
        )
    }

    /// `.key = <octet stream>;`
    pub fn write_octets(&mut self, key: &str, value: &[u8]) -> Result<()> {
        self.assignment(key)?;
        let none = ValueTypeSpec::default();
        self.event(Event::OctetStreamStart, TokenType::Identifier, none, &[])?;
        for chunk in value.chunks(OCTET_CHUNK) {
            self.event(Event::Data, TokenType::OctetStream, none, chunk)?;
        }
        self.event(Event::OctetStreamEnd, TokenType::Identifier, none, &[])
    }

    /// `.key = {` — close with [`Writer::end_struct`].
    pub fn begin_struct(&mut self, key: &str) -> Result<()> {
        self.assignment(key)?;
        self.emit(Event::StructStart, None, None, None)?;
        self.struct_depth += 1;
        Ok(())
    }

    /// `};`
    pub fn end_struct(&mut self) -> Result<()> {
        self.emit(Event::StructEnd, None, None, None)?;
        self.struct_depth = self.struct_depth.saturating_sub(1);
        Ok(())
    }

    /// `[` of an array row.
    pub fn begin_array_row(&mut self) -> Result<()> {
        self.emit(Event::ArrayRowStart, None, None, None)
    }

    /// `]` of an array row.
    pub fn end_array_row(&mut self) -> Result<()> {
        self.emit(Event::ArrayRowEnd, None, None, None)
    }

    /// `/` between array rows.
    pub fn new_array_dim(&mut self) -> Result<()> {
        self.emit(Event::ArrayDimStart, None, None, None)
    }

    /// Completes the document. Fails while a struct is still open.
    pub fn finish(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        if self.struct_depth != 0 {
            return Err(Error::Argument(format!(
                "finish() called with {} unclosed struct(s)",
                self.struct_depth
            )));
        }
        self.finished = true;
        self.check(ffi::WriterHandle::finish)
    }

    /// Bytes written so far.
    pub fn bytes_written(&self) -> u64 {
        self.handle.bytes_written()
    }

    /// The document written so far.
    pub fn get_output(&self) -> &[u8] {
        self.handle.output()
    }

    fn assignment(&mut self, key: &str) -> Result<()> {
        self.emit(Event::AssignmentStart, Some(key), None, None)
    }

    /// An assignment of one annotated scalar literal.
    fn write_scalar(&mut self, key: &str, vt: ValueTypeSpec, text: &str) -> Result<()> {
        self.assignment(key)?;
        self.annotation(vt)?;
        let special = matches!(text, "nan" | "inf" | "ninf");
        let token = if vt.base != 0 && vt.base != 10 && !special {
            TokenType::String
        } else {
            TokenType::Number
        };
        self.event(Event::Data, token, vt, text.as_bytes())
    }

    /// `<family:width[,_base]>` for the uint, sint and float families.
    fn annotation(&mut self, vt: ValueTypeSpec) -> Result<()> {
        self.event(Event::TypeAnnotationStart, TokenType::Type, vt, &[])?;
        self.event(Event::TypeAnnotationTypeFamily, TokenType::Type, vt, &[])?;
        if vt.width != 0 {
            self.event(
                Event::TypeAnnotationTypeFamilyParam,
                TokenType::TypeWidth,
                vt,
                &[],
            )?;
        }
        if vt.base != 0 && vt.base != 10 {
            self.event(
                Event::TypeAnnotationTypeFamilyParam,
                TokenType::TypeBase,
                vt,
                &[],
            )?;
        }
        self.event(
            Event::TypeAnnotationEnd,
            TokenType::Type,
            ValueTypeSpec::default(),
            &[],
        )
    }

    fn event(
        &mut self,
        event: Event,
        token: TokenType,
        vt: ValueTypeSpec,
        data: &[u8],
    ) -> Result<()> {
        if u32::try_from(data.len()).is_err() {
            return Err(Error::Argument("a single value cannot exceed 4 GiB".into()));
        }
        self.check(|h| h.write_event(event as i32, token as i32, vt, data))
    }

    fn check(&mut self, call: impl FnOnce(&mut ffi::WriterHandle) -> bool) -> Result<()> {
        if call(&mut self.handle) {
            Ok(())
        } else {
            Err(self.error())
        }
    }

    fn error(&self) -> Error {
        let (code, offset) = self.handle.error();
        Error::Write { code, offset }
    }
}

fn type_spec(family: ValueTypeFamily, width: u32, base: u32) -> ValueTypeSpec {
    ValueTypeSpec {
        family: family as i32,
        width,
        base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(f: impl FnOnce(&mut Writer) -> Result<()>) -> String {
        let mut w = Writer::to_mem(4096, false).unwrap();
        f(&mut w).unwrap();
        w.finish().unwrap();
        String::from_utf8(w.get_output().to_vec()).unwrap()
    }

    #[test]
    fn writes_scalars_and_structs() {
        let out = written(|w| {
            w.write_uint("u", 255, 64, 16)?;
            w.write_sint("i", -3, 64, 10)?;
            w.write_float("f", 1.5, 64)?;
            w.begin_struct("s")?;
            w.write_string("t", "x")?;
            w.write_null("n")?;
            w.end_struct()
        });
        assert!(out.contains("\"ff\""), "{out}");
        assert!(out.contains("-3") && out.contains("1.5"), "{out}");
        assert!(out.contains("\"x\""), "{out}");
    }

    #[test]
    fn full_buffer_reports_sink_buffer_exhausted() {
        let mut w = Writer::to_mem(8, false).unwrap();
        let err = w
            .write_string("key", "a value longer than eight bytes")
            .and_then(|_| w.finish());
        assert_eq!(err.unwrap_err().name(), Some("sink_buffer_exhausted"));
    }
}
