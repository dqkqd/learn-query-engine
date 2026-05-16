use crate::sql::{
    error::ParseError,
    token::{Keyword, Literal, Symbol, Token, TokenSpan},
    token_stream::TokenStream,
};

pub struct Tokenizer {
    bytes: Vec<u8>,
    position: usize,
}

const SYMBOL_SET: &[u8] = b"+-*";

impl Tokenizer {
    pub fn new(sql: impl AsRef<str>) -> Tokenizer {
        let bytes = sql.as_ref().as_bytes().to_vec();
        Tokenizer { bytes, position: 0 }
    }

    pub fn tokenize(self) -> Result<Vec<TokenSpan>, ParseError> {
        self.collect()
    }

    pub fn stream(self) -> Result<TokenStream, ParseError> {
        let tokens = self.tokenize()?;
        Ok(TokenStream::new(tokens))
    }

    fn skip_whitespace(&mut self) {
        while self.position < self.bytes.len() && self.bytes[self.position].is_ascii_whitespace() {
            self.position += 1;
        }
    }

    /// Scan until we see a white space or a terminate character
    /// Return the position at the terminate character.
    fn get_next_terminate_position(&self) -> usize {
        let mut position = self.position;
        while self
            .bytes
            .get(position)
            .is_some_and(|c| !c.is_ascii_whitespace())
        {
            position += 1;
        }
        position
    }

    /// Get token string, return a string and position
    fn get_token_string(&self) -> (String, usize) {
        let position = self.get_next_terminate_position();
        let token = self.bytes[self.position..position].to_vec();
        let token = String::from_utf8(token)
            .unwrap_or_else(|_| self.panic_error("Invalid utf8 string", self.position, position));
        (token, position)
    }

    fn panic_error(&self, message: &str, start: usize, end: usize) -> ! {
        panic!("{}, start: {}, end: {}", message, start, end)
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
    fn scan_identifier(&mut self) -> Result<TokenSpan, ParseError> {
        let (token, position) = self.get_token_string();
        let token = match Keyword::try_from(token.as_str()) {
            Ok(keyword) => Token::Keyword(keyword),
            Err(_) => Token::Literal(Literal::Indentifier(token)),
        };
        let token_span = TokenSpan::new(token, self.position, position);
        self.position = token_span.end;
        Ok(token_span)
    }

    /// Scan number
    fn scan_number(&mut self) -> Result<TokenSpan, ParseError> {
        let (token, position) = self.get_token_string();
        let token = Literal::try_from(token.as_str())?;
        if !matches!(token, Literal::Long(_)) && !matches!(token, Literal::Double(_)) {
            return Err(ParseError::InvalidLiteralString(
                "token is not a number".to_string(),
            ));
        }
        let token = Token::Literal(token);
        let token_span = TokenSpan::new(token, self.position, position);
        self.position = token_span.end;
        Ok(token_span)
    }

    /// Scan symbol
    fn scan_symbol(&mut self) -> Result<TokenSpan, ParseError> {
        let (token, position) = self.get_token_string();
        let token = Symbol::try_from(token.as_str())?;
        let token = Token::Symbol(token);
        let token_span = TokenSpan::new(token, self.position, position);
        self.position = token_span.end;
        Ok(token_span)
    }
}

impl Iterator for Tokenizer {
    type Item = Result<TokenSpan, ParseError>;

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
        error::ParseError,
        token::{Keyword, Literal, Symbol, Token},
        tokenizer::Tokenizer,
    };

    fn tokens(s: &str) -> Result<Vec<Token>, ParseError> {
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
    fn literal(#[case] sql: &str, #[case] expected: Literal) -> Result<(), ParseError> {
        assert_eq!(tokens(sql)?, [Token::Literal(expected)]);
        Ok(())
    }

    #[rstest]
    #[case("SELECT", Keyword::Select)]
    #[case("Select", Keyword::Select)]
    #[case("FROM", Keyword::From)]
    #[case("WHERE", Keyword::Where)]
    fn keyword(#[case] sql: &str, #[case] expected: Keyword) -> Result<(), ParseError> {
        assert_eq!(tokens(sql)?, [Token::Keyword(expected)]);
        Ok(())
    }

    #[test]
    fn simple_select() -> Result<(), ParseError> {
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
    fn simple_symbol() -> Result<(), ParseError> {
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
}
