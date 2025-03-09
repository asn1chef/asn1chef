use std::{fmt::Write, ops::RangeBounds};

use num::{Num, NumCast};

use crate::compiler::parser::{AstElement, Error, ErrorKind, Result};

fn parse_base_10_integer<T: Num + NumCast + Copy, R: RangeBounds<u64>>(
    chars: &AstElement<&[u8]>,
    valid_range: R,
) -> Result<T> {
    let value = chars.element;
    let length = value.len();

    let mut digits = Vec::with_capacity(length);
    for i in 0..length {
        let digit = value[i];
        if !(digit as char).is_ascii_digit() {
            return Err(Error {
                kind: ErrorKind::Ast(format!("invalid base-10 digit: '{}'", digit as char)),
                loc: chars.loc,
            });
        }
        digits.push(digit);
    }

    let mut int: u64 = 0;
    for (i, digit) in digits.into_iter().rev().enumerate() {
        int += (digit - b'0') as u64 * 10u64.pow(i as u32);
    }

    if !valid_range.contains(&int) {
        return Err(Error {
            kind: ErrorKind::Ast(format!("value '{}' out of bounds", int)),
            loc: chars.loc,
        });
    }

    Ok(NumCast::from(int).expect("NumCast::from failed"))
}

#[derive(Debug, Clone, PartialEq)]
pub enum UTCTimeZoneSign {
    Plus,
    Minus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UTCTimeZone {
    Z,
    Offset {
        sign: UTCTimeZoneSign,
        hour: u8,
        minute: u8,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct UTCTime {
    pub year: u8,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: Option<u8>,
    pub tz: UTCTimeZone,
}

impl UTCTime {
    pub fn parse(str: &AstElement<&[u8]>) -> Result<UTCTime> {
        let value = str.element;

        // shorted possible value is YYMMDDhhmmZ
        if value.len() < 11 {
            return Err(Error {
                kind: ErrorKind::Ast("UTCTime value is too short".to_string()),
                loc: str.loc,
            });
        }

        let (time, tz) = if value[value.len() - 1] == b'Z' {
            (&value[..value.len() - 1], UTCTimeZone::Z)
        } else {
            let tz_sign = value[value.len() - 5];
            if tz_sign == b'+' || tz_sign == b'-' {
                (
                    &value[..value.len() - 5],
                    UTCTimeZone::Offset {
                        sign: match tz_sign {
                            b'+' => UTCTimeZoneSign::Plus,
                            b'-' => UTCTimeZoneSign::Minus,
                            _ => unreachable!(),
                        },
                        hour: parse_base_10_integer(
                            &str.as_ref()
                                .map(|_| &value[value.len() - 4..value.len() - 2]),
                            0..24,
                        )?,
                        minute: parse_base_10_integer(
                            &str.as_ref().map(|_| &value[value.len() - 2..]),
                            0..60,
                        )?,
                    },
                )
            } else {
                return Err(Error {
                    kind: ErrorKind::Ast("UTCTime time zone is malformed".to_string()),
                    loc: str.loc,
                });
            }
        };

        if time.len() != 10 && time.len() != 12 {
            return Err(Error {
                kind: ErrorKind::Ast("UTCTime time zone is malformed (invalid length)".to_string()),
                loc: str.loc,
            });
        }
        let year = parse_base_10_integer(&str.as_ref().map(|_| &time[..2]), 0..100)?;
        let month = parse_base_10_integer(&str.as_ref().map(|_| &time[2..4]), 1..=12)?;
        let day = parse_base_10_integer(&str.as_ref().map(|_| &time[4..6]), 1..=31)?;
        let hour = parse_base_10_integer(&str.as_ref().map(|_| &time[6..8]), 0..=24)?;
        let minute = parse_base_10_integer(&str.as_ref().map(|_| &time[8..10]), 0..60)?;
        let second = match time.len() {
            10 => None,
            12 => Some(parse_base_10_integer(
                &str.as_ref().map(|_| &time[10..]),
                0..60,
            )?),
            _ => unreachable!(),
        };

        Ok(UTCTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            tz,
        })
    }

    pub fn to_ber_string(&self) -> String {
        let mut str = String::with_capacity(17);
        str.write_fmt(format_args!("{:02}", self.year)).unwrap();
        str.write_fmt(format_args!("{:02}", self.month)).unwrap();
        str.write_fmt(format_args!("{:02}", self.day)).unwrap();
        str.write_fmt(format_args!("{:02}", self.hour)).unwrap();
        str.write_fmt(format_args!("{:02}", self.minute)).unwrap();
        if let Some(second) = &self.second {
            str.write_fmt(format_args!("{:02}", second)).unwrap();
        }
        match &self.tz {
            UTCTimeZone::Z => str.write_str("Z").unwrap(),
            UTCTimeZone::Offset { sign, hour, minute } => {
                str.write_str(match sign {
                    UTCTimeZoneSign::Plus => "+",
                    UTCTimeZoneSign::Minus => "-",
                })
                .unwrap();
                str.write_fmt(format_args!("{:02}", hour)).unwrap();
                str.write_fmt(format_args!("{:02}", minute)).unwrap();
            }
        }
        str
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Date {
    pub fn parse(str: &AstElement<&[u8]>) -> Result<Date> {
        let value = str.element;
        if value.len() != 10 {
            return Err(Error {
                kind: ErrorKind::Ast("DATE is malformed (invalid length)".to_string()),
                loc: str.loc,
            });
        }

        if value[4] != b'-' {
            return Err(Error {
                kind: ErrorKind::Ast("DATE is malformed (expecting 4-digit year)".to_string()),
                loc: str.loc,
            });
        }

        if value[7] != b'-' {
            return Err(Error {
                kind: ErrorKind::Ast("DATE is malformed (expecting 2-digit month)".to_string()),
                loc: str.loc,
            });
        }

        let year = parse_base_10_integer(&str.as_ref().map(|_| &value[..4]), 0..10000)?;
        let month = parse_base_10_integer(&str.as_ref().map(|_| &value[5..7]), 1..=12)?;
        let day = parse_base_10_integer(&str.as_ref().map(|_| &value[8..10]), 1..=31)?;

        Ok(Date { year, month, day })
    }

    pub fn to_ber_string(&self) -> String {
        let mut str = String::with_capacity(8);
        str.write_fmt(format_args!("{:04}", self.year)).unwrap();
        str.write_fmt(format_args!("{:02}", self.month)).unwrap();
        str.write_fmt(format_args!("{:02}", self.day)).unwrap();
        str
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeOfDay {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl TimeOfDay {
    pub fn parse(str: &AstElement<&[u8]>) -> Result<TimeOfDay> {
        let value = str.element;
        if value.len() != 8 {
            return Err(Error {
                kind: ErrorKind::Ast("TIME-OF-DAY is malformed (invalid length)".to_string()),
                loc: str.loc,
            });
        }

        if value[2] != b':' {
            return Err(Error {
                kind: ErrorKind::Ast(
                    "TIME-OF-DAY is malformed (expecting 2-digit hour)".to_string(),
                ),
                loc: str.loc,
            });
        }

        if value[5] != b':' {
            return Err(Error {
                kind: ErrorKind::Ast(
                    "TIME-OF-DAY is malformed (expecting 2-digit minute)".to_string(),
                ),
                loc: str.loc,
            });
        }

        let hour = parse_base_10_integer(&str.as_ref().map(|_| &value[..2]), 0..24)?;
        let minute = parse_base_10_integer(&str.as_ref().map(|_| &value[3..5]), 0..60)?;
        let second = parse_base_10_integer(&str.as_ref().map(|_| &value[6..8]), 0..60)?;

        Ok(TimeOfDay {
            hour,
            minute,
            second,
        })
    }

    pub fn to_ber_string(&self) -> String {
        let mut str = String::with_capacity(6);
        str.write_fmt(format_args!("{:02}", self.hour)).unwrap();
        str.write_fmt(format_args!("{:02}", self.minute)).unwrap();
        str.write_fmt(format_args!("{:02}", self.second)).unwrap();
        str
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DateTime {
    pub date: Date,
    pub time_of_day: TimeOfDay,
}

impl DateTime {
    pub fn parse(str: &AstElement<&[u8]>) -> Result<DateTime> {
        let value = str.element;
        if value.len() != 19 {
            return Err(Error {
                kind: ErrorKind::Ast("DATE-TIME is malformed (invalid length)".to_string()),
                loc: str.loc,
            });
        }

        if value[10] != b'T' {
            return Err(Error {
                kind: ErrorKind::Ast(
                    "DATE-TIME is malformed (expecting 'T' separating date and time)".to_string(),
                ),
                loc: str.loc,
            });
        }

        let date = Date::parse(&str.as_ref().map(|_| &value[..10]))?;
        let time_of_day = TimeOfDay::parse(&str.as_ref().map(|_| &value[11..]))?;

        Ok(DateTime { date, time_of_day })
    }

    pub fn to_ber_string(&self) -> String {
        let mut str = String::with_capacity(8 + 6);
        str.write_str(&self.date.to_ber_string()).unwrap();
        str.write_str(&self.time_of_day.to_ber_string()).unwrap();
        str
    }
}

#[cfg(test)]
mod test {
    use crate::{
        compiler::parser::{AstElement, Loc},
        values::{Date, DateTime, TimeOfDay, UTCTime, UTCTimeZone, UTCTimeZoneSign},
    };

    #[test]
    fn test_utc_time_parse() {
        assert_eq!(
            UTCTime::parse(&AstElement::new("8804152030Z".as_bytes(), Loc::at(0))).unwrap(),
            UTCTime {
                year: 88,
                month: 4,
                day: 15,
                hour: 20,
                minute: 30,
                second: None,
                tz: UTCTimeZone::Z,
            }
        );
        assert_eq!(
            UTCTime::parse(&AstElement::new("880415203022Z".as_bytes(), Loc::at(0))).unwrap(),
            UTCTime {
                year: 88,
                month: 4,
                day: 15,
                hour: 20,
                minute: 30,
                second: Some(22),
                tz: UTCTimeZone::Z,
            }
        );
        assert_eq!(
            UTCTime::parse(&AstElement::new("8804152030-0600".as_bytes(), Loc::at(0))).unwrap(),
            UTCTime {
                year: 88,
                month: 4,
                day: 15,
                hour: 20,
                minute: 30,
                second: None,
                tz: UTCTimeZone::Offset {
                    sign: UTCTimeZoneSign::Minus,
                    hour: 6,
                    minute: 0
                },
            }
        );
        assert_eq!(
            UTCTime::parse(&AstElement::new("8804152030+1630".as_bytes(), Loc::at(0))).unwrap(),
            UTCTime {
                year: 88,
                month: 4,
                day: 15,
                hour: 20,
                minute: 30,
                second: None,
                tz: UTCTimeZone::Offset {
                    sign: UTCTimeZoneSign::Plus,
                    hour: 16,
                    minute: 30
                },
            }
        );
    }

    #[test]
    fn test_date_parse() {
        assert_eq!(
            Date::parse(&AstElement::new("2012-12-21".as_bytes(), Loc::at(0))).unwrap(),
            Date {
                year: 2012,
                month: 12,
                day: 21,
            }
        );
        assert_eq!(
            Date::parse(&AstElement::new("0000-01-01".as_bytes(), Loc::at(0))).unwrap(),
            Date {
                year: 0,
                month: 1,
                day: 1,
            }
        );
        assert_eq!(
            Date::parse(&AstElement::new("9999-12-31".as_bytes(), Loc::at(0))).unwrap(),
            Date {
                year: 9999,
                month: 12,
                day: 31,
            }
        );
        assert!(Date::parse(&AstElement::new("2012-00-21".as_bytes(), Loc::at(0))).is_err());
        assert!(Date::parse(&AstElement::new("2012-13-21".as_bytes(), Loc::at(0))).is_err());
        assert!(Date::parse(&AstElement::new("2012-12-00".as_bytes(), Loc::at(0))).is_err());
        assert!(Date::parse(&AstElement::new("2012-12-32".as_bytes(), Loc::at(0))).is_err());
    }

    #[test]
    fn test_time_of_day_parse() {
        assert_eq!(
            TimeOfDay::parse(&AstElement::new("06:35:14".as_bytes(), Loc::at(0))).unwrap(),
            TimeOfDay {
                hour: 6,
                minute: 35,
                second: 14,
            }
        );
        assert_eq!(
            TimeOfDay::parse(&AstElement::new("00:00:00".as_bytes(), Loc::at(0))).unwrap(),
            TimeOfDay {
                hour: 0,
                minute: 0,
                second: 0,
            }
        );
        assert_eq!(
            TimeOfDay::parse(&AstElement::new("23:59:59".as_bytes(), Loc::at(0))).unwrap(),
            TimeOfDay {
                hour: 23,
                minute: 59,
                second: 59,
            }
        );
        assert!(TimeOfDay::parse(&AstElement::new("24:00:00".as_bytes(), Loc::at(0))).is_err());
        assert!(TimeOfDay::parse(&AstElement::new("00:60:00".as_bytes(), Loc::at(0))).is_err());
        assert!(TimeOfDay::parse(&AstElement::new("00:00:60".as_bytes(), Loc::at(0))).is_err());
    }

    #[test]
    fn test_date_time_parse() {
        assert_eq!(
            DateTime::parse(&AstElement::new(
                "2012-12-21T06:35:14".as_bytes(),
                Loc::at(0)
            ))
            .unwrap(),
            DateTime {
                date: Date {
                    year: 2012,
                    month: 12,
                    day: 21,
                },
                time_of_day: TimeOfDay {
                    hour: 6,
                    minute: 35,
                    second: 14,
                }
            }
        );
    }
}
