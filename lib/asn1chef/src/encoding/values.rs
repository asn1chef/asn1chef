use std::io;

use num::{bigint::Sign, BigInt, Signed};
use widestring::{Utf16String, Utf32String};

use crate::types::TagType;

use super::strings;

pub fn der_encode_integer(buf: &mut Vec<u8>, num: &BigInt) {
    const SIGN_MASK: u8 = 0b1000_0000;

    if num == &BigInt::ZERO {
        // fast encode for 0
        buf.push(0x00);
    } else {
        let sign = num.sign();
        let bytes = num.to_signed_bytes_le();

        // write the bytes in little-endian order,
        // such that when the DER is reversed after encoding,
        // the bytes are in big-endian order
        buf.extend_from_slice(&bytes);

        let msb = bytes[bytes.len() - 1];
        if sign != Sign::Minus && msb & SIGN_MASK == SIGN_MASK {
            // when the sign bit is set in the msb, but the number is positive, add a padding byte without the sign bit
            buf.push(0x00);
        } else if sign == Sign::Minus && msb & SIGN_MASK != SIGN_MASK {
            // when the sign bit is not set in the msb, but the number is negative, add a padding byte containing the sign bit
            buf.push(0xff);
        }
    }
}

pub fn der_decode_integer(value: &[u8]) -> io::Result<BigInt> {
    const SIGN_MASK: u8 = 0b1000_0000;

    if value.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "INTEGER must have a value",
        ));
    }

    let sign = if value[0] & SIGN_MASK == SIGN_MASK {
        Sign::Minus
    } else {
        Sign::Plus
    };

    Ok(BigInt::from_bytes_be(sign, value))
}

pub fn der_encode_character_string(buf: &mut Vec<u8>, tag_type: TagType, str: &str) {
    match tag_type {
        TagType::NumericString
        | TagType::PrintableString
        | TagType::IA5String
        | TagType::VisibleString => {
            // NumericString, PrintableString, VisibleString, and IA5String all contain only 7-bit ASCII (T.50) characters
            let bytes = strings::T50_MAP
                .encode_str(str)
                .expect("string cannot be encoded to T.50");
            buf.extend(bytes.into_iter().rev());
        }
        TagType::TeletexString => {
            let bytes = strings::T61_MAP
                .encode_str(str)
                .expect("TeletexString cannot be encoded to T.61");
            buf.extend(bytes.into_iter().rev());
        }
        TagType::VideotexString => {
            let bytes = strings::T100_MAP
                .encode_str(str)
                .expect("VideotexString cannot be encoded to T.100");
            buf.extend(bytes.into_iter().rev());
        }
        TagType::UTF8String
        | TagType::GeneralString
        | TagType::GraphicString
        | TagType::ObjectDescriptor => {
            // Rust strings are already UTF-8, so we just write the string's internal bytes for UTF8String
            // GeneralString, GraphicString, and ObjectDescriptor are implemented as UTF-8 as well
            buf.extend(str.bytes().rev());
        }
        TagType::BMPString => {
            // BMPString is encoded as UTF-16
            let utf16str = Utf16String::from_str(str);
            for b in utf16str.into_vec().into_iter().rev() {
                buf.extend_from_slice(&b.to_le_bytes());
            }
        }
        TagType::UniversalString => {
            // UniversalString is encoded as UTF-32
            let utf32str = Utf32String::from_str(str);
            for b in utf32str.into_vec().into_iter().rev() {
                buf.extend_from_slice(&b.to_le_bytes());
            }
        }
        other => todo!("{:?}", other),
    }
}

pub fn der_encode_real(buf: &mut Vec<u8>, mut mantissa: BigInt, base: i64, mut exponent: BigInt) {
    if base == 2 {
        if mantissa != BigInt::ZERO {
            // while mantissa is even
            while !mantissa.bit(0) {
                mantissa >>= 1;
                exponent += 1;
            }

            der_encode_integer(buf, &mantissa.abs());

            let exp_len = buf.len();
            der_encode_integer(buf, &exponent);
            let exp_len = buf.len() - exp_len;

            let mut bitflags = 0x00;
            bitflags |= 0b1000_0000; // binary encoding
            if mantissa.sign() == Sign::Minus {
                bitflags |= 0b0100_0000; // sign bit
            }

            // See X.690 clause 8.5.7.4 for what is being encoded here
            bitflags |= match exp_len {
                1 => 0b00,
                2 => 0b01,
                3 => 0b10,
                _ => {
                    // X.690 clause 8.5.7.4(d) states that "the first nine bits of the transmitted exponent shall not be all zeros or all ones"
                    // figure out why this is and how to handle it
                    buf.push(
                        exp_len
                            .try_into()
                            .expect("exp_len is larger than 255 bytes"),
                    );
                    0b11
                }
            };

            buf.push(bitflags);
        }
    } else if base == 10 {
        todo!("REAL with base = 10");
    } else {
        panic!("base = {} but must be either 2 or 10", base);
    }
}

#[cfg(test)]
mod test {
    use num::BigInt;

    use super::der_encode_real;

    fn test_der_encode_real(mantissa: i64, base: i64, exponent: i64, expected_der: &[u8]) {
        let mut buf = Vec::with_capacity(expected_der.len());
        der_encode_real(
            &mut buf,
            BigInt::from(mantissa),
            base,
            BigInt::from(exponent),
        );
        let buf = buf.into_iter().rev().collect::<Vec<u8>>();
        assert_eq!(
            buf.as_slice(),
            expected_der,
            "{{ mantissa = {}, base = {}, exponent = {} }}\nexpected = {}\nfound    = {}",
            mantissa,
            base,
            exponent,
            hex::encode_upper(expected_der),
            hex::encode_upper(&buf),
        );
    }

    #[test]
    pub fn test_der_encode_real_base_2() {
        test_der_encode_real(0, 2, 0, &[]); // 0.0 with exp = 0
        test_der_encode_real(0, 2, 1, &[]); // 0.0 with exp = 1
        test_der_encode_real(0, 2, 255, &[]); // 0.0 with exp = 255

        test_der_encode_real(1, 2, 0, &[0x80, 0x00, 0x01]); // 1.0
        test_der_encode_real(1, 2, 1, &[0x80, 0x01, 0x01]); // 2.0 with exp = 1
        test_der_encode_real(2, 2, 0, &[0x80, 0x01, 0x01]); // 2.0 with exp = 0
        test_der_encode_real(8, 2, 0, &[0x80, 0x03, 0x01]); // 8.0 with exp = 0
        test_der_encode_real(4, 2, 1, &[0x80, 0x03, 0x01]); // 8.0 with exp = 1
        test_der_encode_real(2, 2, 2, &[0x80, 0x03, 0x01]); // 8.0 with exp = 2
        test_der_encode_real(1, 2, 3, &[0x80, 0x03, 0x01]); // 8.0 with exp = 3
        test_der_encode_real(1, 2, -1, &[0x80, 0xFF, 0x01]); // 0.5 with exp = -1
        test_der_encode_real(2, 2, -2, &[0x80, 0xFF, 0x01]); // 0.5 with exp = -2
        test_der_encode_real(4, 2, -3, &[0x80, 0xFF, 0x01]); // 0.5 with exp = -3
        test_der_encode_real(8, 2, -4, &[0x80, 0xFF, 0x01]); // 0.5 with exp = -4

        test_der_encode_real(-1, 2, 0, &[0xC0, 0x00, 0x01]); // -1.0
        test_der_encode_real(-1, 2, 1, &[0xC0, 0x01, 0x01]); // -2.0 with exp = 1
        test_der_encode_real(-2, 2, 0, &[0xC0, 0x01, 0x01]); // -2.0 with exp = 0
        test_der_encode_real(-8, 2, 0, &[0xC0, 0x03, 0x01]); // -8.0 with exp = 0
        test_der_encode_real(-4, 2, 1, &[0xC0, 0x03, 0x01]); // -8.0 with exp = 1
        test_der_encode_real(-2, 2, 2, &[0xC0, 0x03, 0x01]); // -8.0 with exp = 2
        test_der_encode_real(-1, 2, 3, &[0xC0, 0x03, 0x01]); // -8.0 with exp = 3
        test_der_encode_real(-1, 2, -1, &[0xC0, 0xFF, 0x01]); // -0.5 with exp = -1
        test_der_encode_real(-2, 2, -2, &[0xC0, 0xFF, 0x01]); // -0.5 with exp = -2
        test_der_encode_real(-4, 2, -3, &[0xC0, 0xFF, 0x01]); // -0.5 with exp = -3
        test_der_encode_real(-8, 2, -4, &[0xC0, 0xFF, 0x01]); // -0.5 with exp = -4
    }
}
