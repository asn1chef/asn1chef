use std::io;

use num::{bigint::Sign, BigInt};
use widestring::{Utf16String, Utf32String};

use crate::types::TagType;

use super::strings;

pub fn der_encode_integer(buf: &mut Vec<u8>, mut num: BigInt) {
    const SIGN_MASK: u8 = 0b1000_0000;

    let sign = num.sign();
    if sign == Sign::Minus {
        // add one to the value for two's complement negative representation
        num += 1;
    }

    if num == BigInt::ZERO {
        if sign == Sign::Minus {
            // fast encode for -1
            buf.push(0xff);
        } else {
            // fast encode for 0
            buf.push(0x00);
        }
    } else {
        let (_, mut bytes) = num.to_bytes_le();

        // invert all bits when the number is negative
        if sign == Sign::Minus {
            for item in &mut bytes {
                *item = !*item;
            }
        }

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
        TagType::UTF8String | TagType::GeneralString | TagType::GraphicString | TagType::ObjectDescriptor => {
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
