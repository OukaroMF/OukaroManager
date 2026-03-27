use std::{fmt::Write as _, fs, path::Path};

use anyhow::{Context, Result, anyhow, bail};
use cesu8::from_java_cesu8;

const ABX_MAGIC: [u8; 4] = [0x41, 0x42, 0x58, 0x00];

const START_DOCUMENT: u8 = 0;
const END_DOCUMENT: u8 = 1;
const START_TAG: u8 = 2;
const END_TAG: u8 = 3;
const TEXT: u8 = 4;
const CDSECT: u8 = 5;
const ENTITY_REF: u8 = 6;
const IGNORABLE_WHITESPACE: u8 = 7;
const PROCESSING_INSTRUCTION: u8 = 8;
const COMMENT: u8 = 9;
const DOCDECL: u8 = 10;
const ATTRIBUTE: u8 = 15;

const TYPE_NULL: u8 = 1 << 4;
const TYPE_STRING: u8 = 2 << 4;
const TYPE_STRING_INTERNED: u8 = 3 << 4;
const TYPE_BYTES_HEX: u8 = 4 << 4;
const TYPE_BYTES_BASE64: u8 = 5 << 4;
const TYPE_INT: u8 = 6 << 4;
const TYPE_INT_HEX: u8 = 7 << 4;
const TYPE_LONG: u8 = 8 << 4;
const TYPE_LONG_HEX: u8 = 9 << 4;
const TYPE_FLOAT: u8 = 10 << 4;
const TYPE_DOUBLE: u8 = 11 << 4;
const TYPE_BOOLEAN_TRUE: u8 = 12 << 4;
const TYPE_BOOLEAN_FALSE: u8 = 13 << 4;

const INTERNED_STRING_NEW_MARKER: u16 = 0xFFFF;

pub fn read_xmlish_text(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    decode_xmlish_bytes(&bytes).with_context(|| format!("decode {}", path.display()))
}

pub fn decode_xmlish_bytes(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(&ABX_MAGIC) {
        return AbxDecoder::new(bytes)?.decode();
    }

    decode_text_xml(bytes)
}

pub fn parse_boolish(value: &str) -> Option<bool> {
    match value.trim() {
        "1" => Some(true),
        "0" => Some(false),
        value
            if value.eq_ignore_ascii_case("true")
                || value.eq_ignore_ascii_case("yes")
                || value.eq_ignore_ascii_case("on") =>
        {
            Some(true)
        }
        value
            if value.eq_ignore_ascii_case("false")
                || value.eq_ignore_ascii_case("no")
                || value.eq_ignore_ascii_case("off") =>
        {
            Some(false)
        }
        _ => None,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_i64ish(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (negative, digits) = if let Some(rest) = trimmed.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (false, rest)
    } else {
        (false, trimmed)
    };

    let parsed = if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        i64::from_str_radix(hex, 16).ok()
    } else if digits
        .bytes()
        .any(|byte| matches!(byte, b'a'..=b'f' | b'A'..=b'F'))
    {
        i64::from_str_radix(digits, 16).ok()
    } else {
        digits.parse::<i64>().ok()
    }?;

    Some(if negative { -parsed } else { parsed })
}

fn decode_text_xml(bytes: &[u8]) -> Result<String> {
    if let Some(bytes) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes.to_vec()).context("decode UTF-8 XML with BOM");
    }

    if let Some(bytes) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16_xml(bytes, true);
    }

    if let Some(bytes) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16_xml(bytes, false);
    }

    if let Some(little_endian) = sniff_utf16_xml_without_bom(bytes) {
        return decode_utf16_xml(bytes, little_endian);
    }

    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => Ok(text),
        Err(_) => decode_modified_utf8(bytes),
    }
}

fn sniff_utf16_xml_without_bom(bytes: &[u8]) -> Option<bool> {
    let prefix = bytes.get(..4)?;

    match prefix {
        [b'<', 0x00, _, 0x00] => Some(true),
        [0x00, b'<', 0x00, _] => Some(false),
        _ => None,
    }
}

fn decode_utf16_xml(bytes: &[u8], little_endian: bool) -> Result<String> {
    if bytes.len() % 2 != 0 {
        bail!("UTF-16 XML byte length is not even");
    }

    let code_units = bytes
        .chunks_exact(2)
        .map(|chunk| {
            if little_endian {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect::<Vec<_>>();

    String::from_utf16(&code_units).context("decode UTF-16 XML")
}

fn decode_modified_utf8(bytes: &[u8]) -> Result<String> {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.to_owned());
    }

    from_java_cesu8(bytes)
        .map(|value| value.into_owned())
        .map_err(|_| anyhow!("decode modified UTF-8 XML"))
}

struct AbxDecoder<'a> {
    bytes: &'a [u8],
    pos: usize,
    interned_strings: Vec<String>,
}

impl<'a> AbxDecoder<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self> {
        if !bytes.starts_with(&ABX_MAGIC) {
            bail!("missing Android binary XML magic");
        }

        Ok(Self {
            bytes,
            pos: ABX_MAGIC.len(),
            interned_strings: Vec::new(),
        })
    }

    fn decode(&mut self) -> Result<String> {
        let mut xml = String::new();

        while self.pos < self.bytes.len() {
            if !self.process_token(&mut xml)? {
                break;
            }
        }

        Ok(xml)
    }

    fn process_token(&mut self, xml: &mut String) -> Result<bool> {
        let token = self.read_u8()?;
        let command = token & 0x0F;
        let type_info = token & 0xF0;

        match command {
            START_DOCUMENT => Ok(true),
            END_DOCUMENT => Ok(false),
            START_TAG => {
                let tag_name = self.read_interned_string()?;
                xml.push('<');
                xml.push_str(&tag_name);

                while self.peek_command() == Some(ATTRIBUTE) {
                    let attribute_token = self.read_u8()?;
                    self.write_attribute(xml, attribute_token)?;
                }

                xml.push('>');
                Ok(true)
            }
            END_TAG => {
                let tag_name = self.read_interned_string()?;
                xml.push_str("</");
                xml.push_str(&tag_name);
                xml.push('>');
                Ok(true)
            }
            TEXT | IGNORABLE_WHITESPACE => {
                let value = self.read_typed_value(type_info)?;
                push_escaped_text(xml, &value);
                Ok(true)
            }
            CDSECT => {
                let value = self.read_typed_value(type_info)?;
                push_cdata_or_escaped_text(xml, &value);
                Ok(true)
            }
            ENTITY_REF => {
                let entity = self.read_typed_value(type_info)?;
                let value = resolve_entity_ref(&entity)?;
                push_escaped_text(xml, &value);
                Ok(true)
            }
            PROCESSING_INSTRUCTION | COMMENT | DOCDECL => {
                let _ = self.read_typed_value(type_info)?;
                Ok(true)
            }
            other => bail!("unsupported Android binary XML token {other}"),
        }
    }

    fn write_attribute(&mut self, xml: &mut String, token: u8) -> Result<()> {
        let type_info = token & 0xF0;
        let name = self.read_interned_string()?;
        let value = self.read_typed_value(type_info)?;

        xml.push(' ');
        xml.push_str(&name);
        xml.push_str("=\"");
        push_escaped_attribute(xml, &value);
        xml.push('"');
        Ok(())
    }

    fn read_typed_value(&mut self, type_info: u8) -> Result<String> {
        match type_info {
            TYPE_NULL => Ok(String::new()),
            TYPE_STRING => self.read_utf_string(),
            TYPE_STRING_INTERNED => self.read_interned_string(),
            TYPE_BYTES_HEX => {
                let length = usize::from(self.read_u16()?);
                let bytes = self.read_slice(length)?;
                Ok(bytes_to_hex(bytes))
            }
            TYPE_BYTES_BASE64 => {
                let length = usize::from(self.read_u16()?);
                let bytes = self.read_slice(length)?;
                Ok(bytes_to_base64(bytes))
            }
            TYPE_INT => Ok(self.read_i32()?.to_string()),
            TYPE_INT_HEX => Ok(format!("0x{:x}", self.read_i32()? as u32)),
            TYPE_LONG => Ok(self.read_i64()?.to_string()),
            TYPE_LONG_HEX => Ok(format!("0x{:x}", self.read_i64()? as u64)),
            TYPE_FLOAT => Ok(self.read_f32()?.to_string()),
            TYPE_DOUBLE => Ok(self.read_f64()?.to_string()),
            TYPE_BOOLEAN_TRUE => Ok(String::from("true")),
            TYPE_BOOLEAN_FALSE => Ok(String::from("false")),
            _ => bail!("unsupported Android binary XML type 0x{type_info:02x}"),
        }
    }

    fn read_interned_string(&mut self) -> Result<String> {
        let index = self.read_u16()?;
        if index == INTERNED_STRING_NEW_MARKER {
            let value = self.read_utf_string()?;
            self.interned_strings.push(value.clone());
            return Ok(value);
        }

        self.interned_strings
            .get(usize::from(index))
            .cloned()
            .ok_or_else(|| anyhow!("invalid Android binary XML string pool index {index}"))
    }

    fn read_utf_string(&mut self) -> Result<String> {
        let length = usize::from(self.read_u16()?);
        let bytes = self.read_slice(length)?;
        decode_modified_utf8(bytes)
    }

    fn peek_command(&self) -> Option<u8> {
        self.bytes.get(self.pos).map(|token| token & 0x0F)
    }

    fn read_u8(&mut self) -> Result<u8> {
        let byte = self
            .bytes
            .get(self.pos)
            .copied()
            .ok_or_else(|| anyhow!("unexpected end of Android binary XML"))?;
        self.pos += 1;
        Ok(byte)
    }

    fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_slice(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_i32(&mut self) -> Result<i32> {
        let bytes = self.read_slice(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i64(&mut self) -> Result<i64> {
        let bytes = self.read_slice(8)?;
        Ok(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.read_i32()? as u32))
    }

    fn read_f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.read_i64()? as u64))
    }

    fn read_slice(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| anyhow!("Android binary XML offset overflow"))?;

        if end > self.bytes.len() {
            bail!("unexpected end of Android binary XML");
        }

        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }
}

fn push_escaped_text(xml: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => xml.push_str("&amp;"),
            '<' => xml.push_str("&lt;"),
            '>' => xml.push_str("&gt;"),
            _ => xml.push(ch),
        }
    }
}

fn push_cdata_or_escaped_text(xml: &mut String, value: &str) {
    if value.contains("]]>") {
        push_escaped_text(xml, value);
        return;
    }

    xml.push_str("<![CDATA[");
    xml.push_str(value);
    xml.push_str("]]>");
}

fn push_escaped_attribute(xml: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => xml.push_str("&amp;"),
            '<' => xml.push_str("&lt;"),
            '>' => xml.push_str("&gt;"),
            '"' => xml.push_str("&quot;"),
            '\'' => xml.push_str("&apos;"),
            _ => xml.push(ch),
        }
    }
}

fn resolve_entity_ref(entity: &str) -> Result<String> {
    match entity {
        "" => Ok(String::new()),
        "lt" => Ok(String::from("<")),
        "gt" => Ok(String::from(">")),
        "amp" => Ok(String::from("&")),
        "apos" => Ok(String::from("'")),
        "quot" => Ok(String::from("\"")),
        _ => {
            let code_point = if let Some(hex) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                u32::from_str_radix(hex, 16)
                    .with_context(|| format!("decode hex XML entity reference `{entity}`"))?
            } else if let Some(decimal) = entity.strip_prefix('#') {
                decimal
                    .parse::<u32>()
                    .with_context(|| format!("decode decimal XML entity reference `{entity}`"))?
            } else {
                bail!("unknown XML entity reference `{entity}`");
            };

            let ch = char::from_u32(code_point)
                .ok_or_else(|| anyhow!("invalid XML entity code point U+{code_point:04X}"))?;
            Ok(ch.to_string())
        }
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn bytes_to_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let triple = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);

        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }

        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }

    out
}
