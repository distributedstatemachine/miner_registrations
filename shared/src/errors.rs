use std::fmt;

#[derive(Debug)]
pub enum Error {
    BlockHeaderNotFound,
    ExceededMaxWaitTime,
    SubxtError(subxt::Error),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BlockHeaderNotFound => write!(f, "Block header not found"),
            Error::ExceededMaxWaitTime => {
                write!(f, "Exceeded maximum wait time for block sampling")
            }
            Error::SubxtError(e) => write!(f, "Subxt error: {}", e),
            Error::Other(s) => write!(f, "Other error: {}", s),
        }
    }
}

impl std::error::Error for Error {}

impl From<subxt::Error> for Error {
    fn from(error: subxt::Error) -> Self {
        Error::SubxtError(error)
    }
}
