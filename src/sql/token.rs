use anyhow::{Result, bail};
use std::fmt::Display;

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
    As,
    And,
    Avg,
    Cast,
    GroupBy,
    OrderBy,
    Having,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Literal {
    Long(i64),
    Double(f64),
    String(String),
    Identifier(String),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Symbol {
    Plus,
    Minus,
    Multiply,
    Divide,
    Eq,
    Le,
    Ge,
    Comma,
    LParen,
    RParen,
}

impl Eq for Literal {}

impl TryFrom<&str> for Keyword {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let keyword = match value.to_lowercase().as_str() {
            "select" => Keyword::Select,
            "cast" => Keyword::Cast,
            "from" => Keyword::From,
            "where" => Keyword::Where,
            "as" => Keyword::As,
            "and" => Keyword::And,
            "avg" => Keyword::Avg,
            "having" => Keyword::Having,
            s => bail!("invalid keyword {}", s),
        };
        Ok(keyword)
    }
}

impl TryFrom<&str> for Literal {
    type Error = anyhow::Error;

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
                None => bail!("unterminated literal string `{}`", value),
            }
        } else {
            Ok(Literal::Identifier(value.to_string()))
        }
    }
}

impl TryFrom<&str> for Symbol {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let symbol = match value {
            "+" => Symbol::Plus,
            "-" => Symbol::Minus,
            "*" => Symbol::Multiply,
            "/" => Symbol::Divide,
            "=" => Symbol::Eq,
            "<" => Symbol::Le,
            ">" => Symbol::Ge,
            "," => Symbol::Comma,
            "(" => Symbol::LParen,
            ")" => Symbol::RParen,
            c => bail!("invalid symbol `{}`", c),
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
            Keyword::Cast => write!(f, "CAST"),
            Keyword::From => write!(f, "FROM"),
            Keyword::Where => write!(f, "WHERE"),
            Keyword::As => write!(f, "AS"),
            Keyword::And => write!(f, "AND"),
            Keyword::Avg => write!(f, "AVG"),
            Keyword::GroupBy => write!(f, "GROUP BY"),
            Keyword::OrderBy => write!(f, "ORDER BY"),
            Keyword::Having => write!(f, "HAVING"),
        }
    }
}

impl Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Long(v) => write!(f, "{}", v),
            Literal::Double(v) => write!(f, "{}", v),
            Literal::String(v) => write!(f, "'{}'", v),
            Literal::Identifier(v) => write!(f, "#{}", v),
        }
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Symbol::Plus => write!(f, "+"),
            Symbol::Minus => write!(f, "-"),
            Symbol::Multiply => write!(f, "*"),
            Symbol::Divide => write!(f, "/"),
            Symbol::Eq => write!(f, "="),
            Symbol::Le => write!(f, "<"),
            Symbol::Ge => write!(f, ">"),
            Symbol::Comma => write!(f, ","),
            Symbol::LParen => write!(f, "("),
            Symbol::RParen => write!(f, ")"),
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
