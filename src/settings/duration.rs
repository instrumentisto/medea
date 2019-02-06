use std::fmt;
use std::time::Duration;

use lazy_static::*;
use regex::Regex;
use serde::de::{self, Unexpected};
use serde::Serializer;

lazy_static! {
    static ref REGEX_DURATION: Regex =
        Regex::new(r"^(?P<minutes>\d+)m|(?P<seconds>\d+)s$").unwrap();
}

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
        let caps = REGEX_DURATION
            .captures(value)
            .ok_or(de::Error::invalid_value(Unexpected::Str(value), &self))?;
        let m = caps
            .name("minutes")
            .map_or(0, |s| s.as_str().parse::<u64>().unwrap());
        let s = caps
            .name("seconds")
            .map_or(0, |s| s.as_str().parse::<u64>().unwrap());

        Ok(Duration::from_secs(m * 60 + s))
    }
}

pub fn serialize<S>(d: &Duration, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(format!("{}s", d.as_secs()).as_str())
}
