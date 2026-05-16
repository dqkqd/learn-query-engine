use anyhow::{Context, Result, bail};

use crate::sql::{
    token::{Keyword, Literal, Symbol, Token, TokenSpan},
    token_stream::TokenStream,
};

pub struct Tokenizer {
    bytes: Vec<u8>,
    position: usize,
}

const SYMBOL_SET: &[u8] = b"+-*/=><,()";
const ALLOW_IDENTIFIER_SET: &[u8] = b"_";
const STRING_QUOTE_SET: &[u8] = b"\'\"";

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

    fn get_string(&self, start: usize, end: usize) -> Result<String> {
        let token = self.bytes[start..end].to_vec();
        let token = String::from_utf8(token)
            .with_context(|| format!("Cannot convert to string at ({}, {})", start, end))?;
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

    fn is_string_start(&self) -> bool {
        self.bytes
            .get(self.position)
            .is_some_and(|c| STRING_QUOTE_SET.contains(c))
    }

    /// Scan keyword or identifier
    fn scan_identifier(&mut self) -> Result<TokenSpan> {
        let position = self.get_position(
            |b| !b.is_ascii_alphabetic() && !ALLOW_IDENTIFIER_SET.contains(b),
            None,
        );
        let token = self.get_string(self.position, position)?;
        if let Ok(token) = self.scan_ambiguous_identifier(token.as_str(), position) {
            return Ok(token);
        }
        let token = match Keyword::try_from(token.as_str()) {
            Ok(keyword) => Token::Keyword(keyword),
            Err(_) => Token::Literal(Literal::Identifier(token)),
        };
        let token_span = TokenSpan::new(token, self.position, position);
        self.position = token_span.end;
        Ok(token_span)
    }

    /// Scan ambiguous identifier: group by | order by
    fn scan_ambiguous_identifier(&mut self, token: &str, position: usize) -> Result<TokenSpan> {
        let token = token.to_lowercase();
        let token = token.as_str();
        if token != "group" && token != "order" {
            bail!("Not an ambiguous identifier: `{}`", token)
        }

        let by_start_position = self.get_position(|b| !b.is_ascii_whitespace(), Some(position));
        let by_end_position =
            self.get_position(|b| !b.is_ascii_alphabetic(), Some(by_start_position));
        let by_token = self.get_string(by_start_position, by_end_position)?;
        if by_token.to_lowercase().as_str() != "by" {
            bail!("Expected `by`, got `{}`", by_token)
        }

        let token = match token {
            "group" => Token::Keyword(Keyword::GroupBy),
            "order" => Token::Keyword(Keyword::OrderBy),
            _ => unreachable!(),
        };
        let token_span = TokenSpan::new(token, position, by_end_position);
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
        let token = self.get_string(self.position, position)?;
        let token = Literal::try_from(token.as_str())?;
        let span = TokenSpan::new(Token::Literal(token), self.position, position);
        self.position = span.end;
        Ok(span)
    }

    /// Scan symbol
    fn scan_symbol(&mut self) -> Result<TokenSpan> {
        let position = self.get_position(|b| !SYMBOL_SET.contains(b), None);
        let token = self.get_string(self.position, position)?;
        let token = Symbol::try_from(token.as_str())?;
        let token = Token::Symbol(token);
        let token_span = TokenSpan::new(token, self.position, position);
        self.position = token_span.end;
        Ok(token_span)
    }

    /// Scan string including quotes
    fn scan_string(&mut self) -> Result<TokenSpan> {
        // get current quote
        let quote = self.bytes.get(self.position).unwrap();
        let position = self.get_position(|b| b == quote, Some(self.position + 1));
        if position == self.bytes.len() {
            bail!(
                "Unterminated string `{}`",
                self.get_string(self.position, position)?
            );
        }
        let position = position + 1;
        let token = self.get_string(self.position, position)?;
        let token = Literal::try_from(token.as_str())?;
        let token = Token::Literal(token);
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

        if self.is_string_start() {
            let span = self.scan_string();
            return Some(span);
        }

        None
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use insta::assert_snapshot;
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

    fn tokens_to_string(tokens: Vec<Token>) -> String {
        let s: Vec<String> = tokens.into_iter().map(|t| t.to_string()).collect();
        s.join(" ")
    }

    #[rstest]
    #[case("123", Literal::Long(123))]
    #[case("1.23", Literal::Double(1.23))]
    #[case("one", Literal::Identifier("one".to_string()))]
    #[case("contains_underscore", Literal::Identifier("contains_underscore".to_string()))]
    fn literal(#[case] sql: &str, #[case] expected: Literal) -> Result<()> {
        assert_eq!(tokens(sql)?, [Token::Literal(expected)]);
        Ok(())
    }

    #[rstest]
    #[case("SELECT", Keyword::Select)]
    #[case("Select", Keyword::Select)]
    #[case("FROM", Keyword::From)]
    #[case("WHERE", Keyword::Where)]
    #[case("AS", Keyword::As)]
    fn keyword(#[case] sql: &str, #[case] expected: Keyword) -> Result<()> {
        assert_eq!(tokens(sql)?, [Token::Keyword(expected)]);
        Ok(())
    }

    #[rstest]
    #[case("+", Symbol::Plus)]
    #[case("-", Symbol::Minus)]
    #[case("*", Symbol::Multiply)]
    #[case("/", Symbol::Divide)]
    #[case("=", Symbol::Eq)]
    #[case(",", Symbol::Comma)]
    fn symbol(#[case] sql: &str, #[case] expected: Symbol) -> Result<()> {
        assert_eq!(tokens(sql)?, [Token::Symbol(expected)]);
        Ok(())
    }

    #[test]
    fn select_1() -> Result<()> {
        let tokens = tokens(
            r#"
SELECT id, first_name, salary * 1.1 AS new_salary
FROM employee
WHERE state = 'CO'
"#,
        )?;

        assert_snapshot!(
            tokens_to_string(tokens),
            @"SELECT #id , #first_name , #salary * 1.1 AS #new_salary FROM #employee WHERE #state = 'CO'",
        );
        Ok(())
    }

    #[test]
    fn select_2() -> Result<()> {
        let tokens = tokens(
            r#"
SELECT id, first_name, salary/12 AS monthly_salary
FROM employee
WHERE state = 'CO' AND monthly_salary > 1000
"#,
        )?;

        assert_snapshot!(
            tokens_to_string(tokens),
            @"SELECT #id , #first_name , #salary / 12 AS #monthly_salary FROM #employee WHERE #state = 'CO' AND #monthly_salary > 1000",
        );
        Ok(())
    }

    #[test]
    fn select_3() -> Result<()> {
        let tokens = tokens(
            r#"
SELECT department, AVG(salary) AS avg_salary
FROM employee
WHERE state = 'CO'
GROUP BY department
HAVING avg_salary > 50000
"#,
        )?;

        assert_snapshot!(
            tokens_to_string(tokens),
            @"SELECT #department , AVG ( #salary ) AS #avg_salary FROM #employee WHERE #state = 'CO' GROUP BY #department HAVING #avg_salary > 50000",
        );
        Ok(())
    }
}
