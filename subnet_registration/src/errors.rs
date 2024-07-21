use std::fmt;

#[derive(Debug)]
pub enum Error {
    ConnectionError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ConnectionError(e) => write!(f, "Connection error: {}", e),
        }
    }
}

impl std::error::Error for Error {}
