//! What can go wrong with a settings file.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// An error reading, checking or saving settings. Its text is what people
/// see, so every message says what is wrong in plain words.
#[derive(Debug)]
pub enum Error {
    /// The file could not be read or written.
    Io(io::Error),
    /// Not TOML, or a setting that does not exist.
    Parse(String),
    /// A value out of range, or settings that do not fit together.
    Invalid(String),
    /// Something wrong with a particular file.
    InFile(PathBuf, Box<Error>),
}

impl Error {
    /// This error, about the file at `path`.
    pub(super) fn in_file(self, path: impl Into<PathBuf>) -> Error {
        Error::InFile(path.into(), Box::new(self))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Parse(m) | Error::Invalid(m) => write!(f, "{m}"),
            Error::InFile(path, e) => write!(f, "{}: {e}", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::InFile(_, e) => Some(e.as_ref()),
            Error::Parse(_) | Error::Invalid(_) => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::config::Config;

    #[test]
    fn each_problem_has_its_kind_and_names_the_file() {
        let typo = Config::parse("[[tables]]\nseatz = 4\n").unwrap_err();
        assert!(matches!(typo, Error::Parse(_)), "{typo:?}");
        let range = Config::parse("[[tables]]\nseats = 1\n").unwrap_err();
        assert!(matches!(range, Error::Invalid(_)), "{range:?}");
        let path = Path::new("no/such/holdem.toml");
        let missing = Config::read(path).unwrap_err();
        assert!(
            matches!(&missing, Error::InFile(p, e) if p == path && matches!(**e, Error::Io(_))),
            "{missing:?}"
        );
        assert!(missing.to_string().starts_with(&path.display().to_string()));
    }
}
