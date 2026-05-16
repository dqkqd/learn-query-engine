use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("invalid keyword {0}")]
    InvalidKeyword(String),
    #[error("{0}")]
    InvalidLiteralString(String),
    #[error("{0}")]
    InvalidLiteralNumber(String),
    #[error("invalid symbol {0}")]
    InvalidSymbol(String),
}
