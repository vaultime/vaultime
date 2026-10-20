// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Just enough of the protocol buffer wire format for the caches Battle.net
//! and Ubisoft Connect keep: fields by number, as varints or bytes.

/// A field's value. Fixed width numbers are skipped, no launcher field read
/// here uses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Value<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
}

/// The fields of a message in order. `None` when the bytes are no message.
pub(crate) fn fields(data: &[u8]) -> Option<Vec<(u64, Value<'_>)>> {
    let mut found = Vec::new();
    let mut rest = data;
    while !rest.is_empty() {
        let key = varint(&mut rest)?;
        let number = key >> 3;
        if number == 0 {
            return None;
        }
        match key & 0b111 {
            0 => found.push((number, Value::Varint(varint(&mut rest)?))),
            1 => rest = rest.get(8..)?,
            2 => {
                let length = usize::try_from(varint(&mut rest)?).ok()?;
                let (bytes, after) = rest.split_at_checked(length)?;
                found.push((number, Value::Bytes(bytes)));
                rest = after;
            }
            5 => rest = rest.get(4..)?,
            _ => return None,
        }
    }
    Some(found)
}

/// The first bytes field `number` of `fields`.
pub(crate) fn bytes<'a>(fields: &[(u64, Value<'a>)], number: u64) -> Option<&'a [u8]> {
    fields.iter().find_map(|(field, value)| match value {
        Value::Bytes(bytes) if *field == number => Some(*bytes),
        _ => None,
    })
}

/// The first field `number` of `fields` as UTF-8 text.
pub(crate) fn text<'a>(fields: &[(u64, Value<'a>)], number: u64) -> Option<&'a str> {
    std::str::from_utf8(bytes(fields, number)?).ok()
}

/// The first varint field `number` of `fields`.
pub(crate) fn number(fields: &[(u64, Value<'_>)], number: u64) -> Option<u64> {
    fields.iter().find_map(|(field, value)| match value {
        Value::Varint(value) if *field == number => Some(*value),
        _ => None,
    })
}

/// Reads a varint off the front of `data`.
fn varint(data: &mut &[u8]) -> Option<u64> {
    let mut value = 0_u64;
    for shift in (0..64).step_by(7) {
        let (&byte, rest) = data.split_first()?;
        *data = rest;
        value |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

/// Writes messages for tests, the way launchers store them.
#[cfg(test)]
pub(crate) mod write {
    pub(crate) fn varint(mut value: u64, out: &mut Vec<u8>) {
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }

    pub(crate) fn number(field: u64, value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        varint(field << 3, &mut out);
        varint(value, &mut out);
        out
    }

    pub(crate) fn bytes(field: u64, value: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        varint((field << 3) | 2, &mut out);
        varint(value.len() as u64, &mut out);
        out.extend_from_slice(value);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_numbers_and_nested_messages() {
        let inner = [write::bytes(1, b"Diablo IV"), write::number(2, 300)].concat();
        let message = [
            write::number(1, 1),
            write::bytes(3, &inner),
            vec![0x0D, 1, 2, 3, 4],
        ]
        .concat();
        let outer = fields(&message).unwrap();
        assert_eq!(number(&outer, 1), Some(1));
        let nested = fields(bytes(&outer, 3).unwrap()).unwrap();
        assert_eq!(text(&nested, 1), Some("Diablo IV"));
        assert_eq!(number(&nested, 2), Some(300));
        assert_eq!(text(&nested, 9), None);
    }

    #[test]
    fn refuses_what_is_no_message() {
        assert_eq!(fields(&[0x0A, 0x05, b'a']), None);
        assert_eq!(fields(&[0x80]), None);
        assert_eq!(fields(&[0x00, 0x01]), None);
        assert_eq!(fields(&[]), Some(Vec::new()));
    }
}
