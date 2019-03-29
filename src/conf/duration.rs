/// Provides deserialize [`time::Duration`] from string.
use std::fmt;
use std::time::Duration;

use serde::de::{self, Unexpected};
use serde::Serializer;

struct DurationFromStringVisitor;

pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
where
    D: de::Deserializer<'de>,
{
    Ok(d.deserialize_str(DurationFromStringVisitor)?)
}

impl<'de> de::Visitor<'de> for DurationFromStringVisitor {
    type Value = Duration;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a string representation of duration")
    }

    fn visit_str<E>(self, value: &str) -> Result<Duration, E>
    where
        E: de::Error,
    {
        let dur = humantime::parse_duration(value).map_err(|_| {
            de::Error::invalid_value(Unexpected::Str(value), &self)
        })?;
        Ok(dur)
    }
}

pub fn serialize<S>(d: &Duration, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&format!("{}", humantime::format_duration(*d)))
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};
    use serde_test::{assert_de_tokens, assert_ser_tokens, Token};
    use std::time::Duration;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct S {
        #[serde(serialize_with = "super::serialize")]
        #[serde(deserialize_with = "super::deserialize")]
        dur: Duration,
    }

    #[test]
    fn test_de() {
        let valid = vec![
            ("17nsec", Duration::new(0, 17)),
            ("17nanos", Duration::new(0, 17)),
            ("33ns", Duration::new(0, 33)),
            ("3usec", Duration::new(0, 3000)),
            ("78us", Duration::new(0, 78000)),
            ("31msec", Duration::new(0, 31000000)),
            ("31millis", Duration::new(0, 31000000)),
            ("6ms", Duration::new(0, 6000000)),
            ("3000s", Duration::new(3000, 0)),
            ("300sec", Duration::new(300, 0)),
            ("300secs", Duration::new(300, 0)),
            ("50seconds", Duration::new(50, 0)),
            ("1second", Duration::new(1, 0)),
            ("100m", Duration::new(6000, 0)),
            ("12min", Duration::new(720, 0)),
            ("12mins", Duration::new(720, 0)),
            ("1minute", Duration::new(60, 0)),
            ("7minutes", Duration::new(420, 0)),
            ("2h", Duration::new(7200, 0)),
            ("7hr", Duration::new(25200, 0)),
            ("7hrs", Duration::new(25200, 0)),
            ("1hour", Duration::new(3600, 0)),
            ("24hours", Duration::new(86400, 0)),
            ("1day", Duration::new(86400, 0)),
            ("2days", Duration::new(172800, 0)),
            ("365d", Duration::new(31536000, 0)),
            ("1week", Duration::new(604800, 0)),
            ("7weeks", Duration::new(4233600, 0)),
            ("52w", Duration::new(31449600, 0)),
            ("1month", Duration::new(2630016, 0)),
            ("3months", Duration::new(3 * 2630016, 0)),
            ("12M", Duration::new(31560192, 0)),
            ("1year", Duration::new(31557600, 0)),
            ("7years", Duration::new(7 * 31557600, 0)),
            ("17y", Duration::new(536479200, 0)),
        ];

        valid.into_iter().for_each(|(formatted, dur)| {
            let s = S { dur };

            assert_de_tokens(
                &s,
                &[
                    Token::Struct { name: "S", len: 1 },
                    Token::Str("dur"),
                    Token::Str(formatted),
                    Token::StructEnd,
                ],
            );
        });
    }

    #[test]
    fn test_ser() {
        let s = S {
            dur: Duration::new(236179457, 45500),
        };

        assert_ser_tokens(
            &s,
            &[
                Token::Struct { name: "S", len: 1 },
                Token::Str("dur"),
                Token::Str("7years 5months 24days 14h 36m 17s 45us 500ns"),
                Token::StructEnd,
            ],
        );
    }
}
