// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lowercase hex encoding for hashes and keys.

pub fn encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

/// Reads hex in either case, or `None` when it is not hex.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    text.as_bytes()
        .chunks(2)
        .map(|pair| {
            let digit = |byte: u8| char::from(byte).to_digit(16);
            u8::try_from(digit(pair[0])? * 16 + digit(pair[1])?).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_lowercase_pairs() {
        assert_eq!(super::encode(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    }

    #[test]
    fn decodes_what_it_encodes() {
        assert_eq!(
            super::decode("000fA5ff"),
            Some(vec![0x00, 0x0f, 0xa5, 0xff])
        );
        assert_eq!(super::decode("abc"), None);
        assert_eq!(super::decode("zz"), None);
    }
}
