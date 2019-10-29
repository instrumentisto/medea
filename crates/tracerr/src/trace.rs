//! Error trace.

use std::{
    fmt,
    ops::{Deref, DerefMut},
};

use derive_more::Display;

/// Captured frame of [`Trace`].
#[derive(Clone, Copy, Debug, Display)]
#[display(fmt = "{}\n  at {}:{}", module, file, line)]
pub struct Frame {
    /// Name of source file where [`Frame`] is captured.
    pub file: &'static str,
    /// Line number in source file where [`Frame`] is captured.
    pub line: u32,
    /// Absolute name of module where [`Frame`] is captured.
    pub module: &'static str,
}

/// Captures and returns new [`Frame`] in the macro invocation place.
#[macro_export]
macro_rules! new_frame {
    () => {
        $crate::Frame {
            file: file!(),
            line: line!(),
            module: module_path!(),
        }
    };
}

/// Trace composed from captured [`Frame`]s.
#[derive(Debug)]
pub struct Trace(Vec<Frame>);

impl Trace {
    /// Creates and returns new [`Trace`] from given [`Frame`]s.
    #[inline]
    pub fn new(frames: Vec<Frame>) -> Self {
        Self(frames)
    }
}

impl Deref for Trace {
    type Target = Vec<Frame>;

    #[inline]
    fn deref(&self) -> &Vec<Frame> {
        &self.0
    }
}

impl DerefMut for Trace {
    #[inline]
    fn deref_mut(&mut self) -> &mut Vec<Frame> {
        &mut self.0
    }
}

impl fmt::Display for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error trace:")?;
        for frame in &self.0 {
            write!(f, "\n{}", frame)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod frame_spec {
    use super::*;

    #[test]
    fn displays_module_followed_by_file_and_line() {
        let frame = Frame {
            file: "my_file.rs",
            line: 32,
            module: "main::sub",
        };
        assert_eq!(format!("{}", frame), "main::sub\n  at my_file.rs:32");
    }
}

#[cfg(test)]
mod trace_spec {
    use super::*;

    #[test]
    fn displays_frames_separated_by_blank_line() {
        let stack = Trace(vec![
            Frame {
                file: "src/my_file.rs",
                line: 32,
                module: "main::sub1",
            },
            Frame {
                file: "anywhere/my_file.rs",
                line: 54,
                module: "main::sub2",
            },
            Frame {
                file: "file.rs",
                line: 232,
                module: "main::sub3",
            },
        ]);
        assert_eq!(
            format!("{}\n            ", stack),
            r#"error trace:
main::sub1
  at src/my_file.rs:32
main::sub2
  at anywhere/my_file.rs:54
main::sub3
  at file.rs:232
            "#
        );
    }
}
