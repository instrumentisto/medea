/// Provides Serde serializer for [`std::time::Duration`] via the
/// `humantime`  crate .
use serde::Serializer;

use std::time::Duration;

/// Serializes a `Duration` via the humantime crate.
pub fn serialize<S>(d: &Duration, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&format!("{}", humantime::format_duration(*d)))
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};
    use serde_test::{assert_ser_tokens, Token};

    use std::time::Duration;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct S {
        #[serde(serialize_with = "super::serialize")]
        dur: Duration,
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
