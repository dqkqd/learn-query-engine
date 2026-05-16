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

    pub fn peek(&self) -> Option<TokenSpan> {
        self.tokens.get(self.position).cloned()
    }

    pub fn expect(&mut self, expect: Token) -> Result<()> {
        let span = self.next().with_context(|| "No remaining tokens")?;
        if span.token != expect {
            bail!("Unexpected token: expected {}, got {}", expect, span.token);
        }
        Ok(())
    }
}

impl Iterator for TokenStream {
    type Item = TokenSpan;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.tokens.get(self.position)?;
        self.position += 1;
        Some(token.clone())
    }
}
