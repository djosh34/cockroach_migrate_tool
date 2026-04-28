#![allow(dead_code)]

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const UPPER_HEX: &[u8; 16] = b"0123456789ABCDEF";

pub(crate) fn encode_ca_cert_fixture_value(bytes: &[u8]) -> String {
    let base64 = encode_base64(bytes);
    percent_encode(&base64, |byte| !byte.is_ascii_alphanumeric())
}

pub(crate) fn encode_ca_cert_query_value(bytes: &[u8]) -> String {
    let base64 = encode_base64(bytes);
    percent_encode(&base64, |byte| {
        byte.is_ascii_control() || matches!(byte, b'+' | b'/' | b'=')
    })
}

fn encode_base64(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied();
        let third = chunk.get(2).copied();

        encoded.push(BASE64_ALPHABET[(first >> 2) as usize] as char);
        encoded.push(
            BASE64_ALPHABET[(((first & 0b0000_0011) << 4) | second.unwrap_or(0) >> 4) as usize]
                as char,
        );

        match (second, third) {
            (Some(second), Some(third)) => {
                encoded.push(
                    BASE64_ALPHABET[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize]
                        as char,
                );
                encoded.push(BASE64_ALPHABET[(third & 0b0011_1111) as usize] as char);
            }
            (Some(second), None) => {
                encoded.push(BASE64_ALPHABET[((second & 0b0000_1111) << 2) as usize] as char);
                encoded.push('=');
            }
            (None, None) => {
                encoded.push('=');
                encoded.push('=');
            }
            (None, Some(_)) => unreachable!("three-byte chunk cannot miss its second byte"),
        }
    }

    encoded
}

fn percent_encode(input: &str, should_escape: impl Fn(u8) -> bool) -> String {
    let mut encoded = String::with_capacity(input.len());

    for byte in input.bytes() {
        if should_escape(byte) {
            encoded.push('%');
            encoded.push(UPPER_HEX[(byte >> 4) as usize] as char);
            encoded.push(UPPER_HEX[(byte & 0x0f) as usize] as char);
        } else {
            encoded.push(byte as char);
        }
    }

    encoded
}
