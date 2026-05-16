use std::fmt::Display;

use crate::sql::error::ParseError;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Token {
    Keyword(Keyword),
    Literal(Literal),
    Symbol(Symbol),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Keyword {
    Select,
    From,
    Where,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Literal {
    Long(i64),
    Double(f64),
    String(String),
    Indentifier(String),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Symbol {
    Plus,
    Minus,
    Multiply,
}

impl Eq for Literal {}

impl TryFrom<&str> for Keyword {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let keyword = match value.to_lowercase().as_str() {
            "select" => Keyword::Select,
            "from" => Keyword::From,
            "where" => Keyword::Where,
            s => return Err(ParseError::InvalidKeyword(s.to_string())),
        };
        Ok(keyword)
    }
}

impl TryFrom<&str> for Literal {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Ok(value) = value.parse::<i64>() {
            return Ok(Literal::Long(value));
        };
        if let Ok(value) = value.parse::<f64>() {
            return Ok(Literal::Double(value));
        };

        // string
        if value.starts_with("'") {
            match value.strip_prefix("'").and_then(|v| v.strip_suffix("'")) {
                Some(value) => Ok(Literal::String(value.to_string())),
                None => Err(ParseError::InvalidLiteralString(
                    "unterminated literal string".to_string(),
                )),
            }
        } else {
            Ok(Literal::Indentifier(value.to_string()))
        }
    }
}

impl TryFrom<&str> for Symbol {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let symbol = match value {
            "+" => Symbol::Plus,
            "-" => Symbol::Minus,
            "*" => Symbol::Multiply,
            c => return Err(ParseError::InvalidSymbol(c.to_string())),
        };
        Ok(symbol)
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Keyword(keyword) => write!(f, "{}", keyword),
            Token::Literal(literal) => write!(f, "{}", literal),
            Token::Symbol(symbol) => write!(f, "{}", symbol),
        }
    }
}

impl Display for Keyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Keyword::Select => write!(f, "SELECT"),
            Keyword::From => write!(f, "FROM"),
            Keyword::Where => write!(f, "WHERE"),
        }
    }
}

impl Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Long(v) => write!(f, "{}", v),
            Literal::Double(v) => write!(f, "{}", v),
            Literal::String(v) => write!(f, "{}", v),
            Literal::Indentifier(v) => write!(f, "{}", v),
        }
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Symbol::Plus => write!(f, "+"),
            Symbol::Minus => write!(f, "-"),
            Symbol::Multiply => write!(f, "*"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TokenSpan {
    pub token: Token,
    pub start: usize,
    pub end: usize,
}

impl TokenSpan {
    pub fn new(token: Token, start: usize, end: usize) -> TokenSpan {
        TokenSpan { token, start, end }
    }
}
