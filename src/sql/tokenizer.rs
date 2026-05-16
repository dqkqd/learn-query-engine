use anyhow::{Context, Result};

use crate::sql::{
    token::{Keyword, Literal, Symbol, Token, TokenSpan},
    token_stream::TokenStream,
};

pub struct Tokenizer {
    bytes: Vec<u8>,
    position: usize,
}

const SYMBOL_SET: &[u8] = b"+-*,";

impl Tokenizer {
    pub fn new(sql: impl AsRef<str>) -> Tokenizer {
        let bytes = sql.as_ref().as_bytes().to_vec();
        Tokenizer { bytes, position: 0 }
    }

    pub fn tokenize(self) -> Result<Vec<TokenSpan>> {
        self.collect()
    }

    pub fn stream(self) -> Result<TokenStream> {
        let tokens = self.tokenize()?;
        Ok(TokenStream::new(tokens))
    }

    fn skip_whitespace(&mut self) {
        while self.position < self.bytes.len() && self.bytes[self.position].is_ascii_whitespace() {
            self.position += 1;
        }
    }

    fn get_string(&self, position: usize) -> Result<String> {
        let token = self.bytes[self.position..position].to_vec();
        let token = String::from_utf8(token).with_context(|| {
            format!(
                "Cannot convert to string at ({}, {})",
                self.position, position
            )
        })?;
        Ok(token)
    }

    fn get_position<F>(&self, predicate: F, from: Option<usize>) -> usize
    where
        F: Fn(&u8) -> bool,
    {
        let mut position = from.unwrap_or(self.position);
        while self.bytes.get(position).is_some_and(|b| !predicate(b)) {
            position += 1;
        }
        position
    }

    fn is_identifier_start(&self) -> bool {
        self.bytes
            .get(self.position)
            .is_some_and(|c| c.is_ascii_alphabetic())
    }

    fn is_number_start(&self) -> bool {
        self.bytes
            .get(self.position)
            .is_some_and(|c| c.is_ascii_digit() || c == &b'.')
    }

    fn is_symbol_start(&self) -> bool {
        self.bytes
            .get(self.position)
            .is_some_and(|c| SYMBOL_SET.contains(c))
    }

    /// Scan keyword or identifier
    fn scan_identifier(&mut self) -> Result<TokenSpan> {
        let position = self.get_position(|b| !b.is_ascii_alphabetic(), None);
        let token = self.get_string(position)?;
        let token = match Keyword::try_from(token.as_str()) {
            Ok(keyword) => Token::Keyword(keyword),
            Err(_) => Token::Literal(Literal::Indentifier(token)),
        };
        let token_span = TokenSpan::new(token, self.position, position);
        self.position = token_span.end;
        Ok(token_span)
    }

    /// Scan number
    fn scan_number(&mut self) -> Result<TokenSpan> {
        let mut position = self.get_position(|b| !b.is_ascii_digit(), None);
        // might be we are parsing float, check it!
        if self.bytes.get(position).is_some_and(|b| b == &b'.') {
            position = self.get_position(|b| !b.is_ascii_digit(), Some(position + 1));
        }
        let token = self.get_string(position)?;
        let token = Literal::try_from(token.as_str())?;
        let span = TokenSpan::new(Token::Literal(token), self.position, position);
        self.position = span.end;
        Ok(span)
    }

    /// Scan symbol
    fn scan_symbol(&mut self) -> Result<TokenSpan> {
        let position = self.get_position(|b| !SYMBOL_SET.contains(b), None);
        let token = self.get_string(position)?;
        let token = Symbol::try_from(token.as_str())?;
        let token = Token::Symbol(token);
        let token_span = TokenSpan::new(token, self.position, position);
        self.position = token_span.end;
        Ok(token_span)
    }
}

impl Iterator for Tokenizer {
    type Item = Result<TokenSpan>;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_whitespace();
        if self.position >= self.bytes.len() {
            return None;
        }

        if self.is_identifier_start() {
            let span = self.scan_identifier();
            return Some(span);
        }

        if self.is_number_start() {
            let span = self.scan_number();
            return Some(span);
        }

        if self.is_symbol_start() {
            let span = self.scan_symbol();
            return Some(span);
        }

        None
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use rstest::rstest;

    use crate::sql::{
        token::{Keyword, Literal, Symbol, Token},
        tokenizer::Tokenizer,
    };

    fn tokens(s: &str) -> Result<Vec<Token>> {
        let tokens = Tokenizer::new(s)
            .tokenize()?
            .into_iter()
            .map(|token_span| token_span.token)
            .collect();
        Ok(tokens)
    }

    #[rstest]
    #[case("123", Literal::Long(123))]
    #[case("1.23", Literal::Double(1.23))]
    #[case("one", Literal::Indentifier("one".to_string()))]
    fn literal(#[case] sql: &str, #[case] expected: Literal) -> Result<()> {
        assert_eq!(tokens(sql)?, [Token::Literal(expected)]);
        Ok(())
    }

    #[rstest]
    #[case("SELECT", Keyword::Select)]
    #[case("Select", Keyword::Select)]
    #[case("FROM", Keyword::From)]
    #[case("WHERE", Keyword::Where)]
    fn keyword(#[case] sql: &str, #[case] expected: Keyword) -> Result<()> {
        assert_eq!(tokens(sql)?, [Token::Keyword(expected)]);
        Ok(())
    }

    #[test]
    fn simple_select() -> Result<()> {
        assert_eq!(
            tokens("select name from employee")?,
            [
                Token::Keyword(Keyword::Select),
                Token::Literal(Literal::Indentifier("name".to_string())),
                Token::Keyword(Keyword::From),
                Token::Literal(Literal::Indentifier("employee".to_string())),
            ]
        );
        Ok(())
    }

    #[test]
    fn simple_symbol() -> Result<()> {
        assert_eq!(
            tokens("1 + 2 * 3 - 4")?,
            [
                Token::Literal(Literal::Long(1)),
                Token::Symbol(Symbol::Plus),
                Token::Literal(Literal::Long(2)),
                Token::Symbol(Symbol::Multiply),
                Token::Literal(Literal::Long(3)),
                Token::Symbol(Symbol::Minus),
                Token::Literal(Literal::Long(4)),
            ]
        );
        Ok(())
    }

    #[test]
    fn commas() -> Result<()> {
        assert_eq!(
            tokens("1 + 2, 3 + 4, 5 + 6")?,
            [
                Token::Literal(Literal::Long(1)),
                Token::Symbol(Symbol::Plus),
                Token::Literal(Literal::Long(2)),
                Token::Symbol(Symbol::Comma),
                Token::Literal(Literal::Long(3)),
                Token::Symbol(Symbol::Plus),
                Token::Literal(Literal::Long(4)),
                Token::Symbol(Symbol::Comma),
                Token::Literal(Literal::Long(5)),
                Token::Symbol(Symbol::Plus),
                Token::Literal(Literal::Long(6)),
            ]
        );
        Ok(())
    }
}
