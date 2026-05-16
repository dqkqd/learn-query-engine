use std::fmt::Display;

use anyhow::{Context, Result, bail};

use crate::sql::token::{Token, TokenSpan};

#[derive(Debug)]
pub struct TokenStream {
    tokens: Vec<TokenSpan>,
    position: usize,
}

impl TokenStream {
    pub fn new(tokens: Vec<TokenSpan>) -> TokenStream {
        TokenStream {
            tokens,
            position: 0,
        }
    }

    pub fn peek_token(&self) -> Result<Token> {
        let span = self
            .tokens
            .get(self.position)
            .with_context(|| format!("No token left\n{}", self.context()))?;
        Ok(span.token.clone())
    }

    #[must_use]
    pub fn consume(&mut self, tokens: &[Token]) -> bool {
        let ok = tokens.iter().enumerate().all(|(i, token)| {
            self.tokens.get(self.position + i).map(|span| &span.token) == Some(token)
        });
        if ok {
            self.position += tokens.len();
        }
        ok
    }

    pub fn expect(&mut self, expect: Token) -> Result<()> {
        let token = self.next_token()?;
        if token != expect {
            bail!(
                "Unexpected token: expected {}, got {}\n{}",
                expect,
                token,
                self.context()
            );
        }
        Ok(())
    }

    pub fn next_token(&mut self) -> Result<Token> {
        let token = self.peek_token()?;
        self.position += 1;
        Ok(token)
    }

    pub fn context(&self) -> String {
        let strings: Vec<String> = self.tokens.iter().map(|s| s.token.to_string()).collect();
        let original = strings.join(" ");
        let consumed = strings[..self.position].join(" ");
        format!(
            "Context\n\toriginal: `{}`\n\tconsumed: `{}`",
            original, consumed
        )
    }
}

impl Display for TokenStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.context())
    }
}
