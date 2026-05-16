use crate::sql::token::TokenSpan;

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
}

impl Iterator for TokenStream {
    type Item = TokenSpan;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.tokens.get(self.position)?;
        self.position += 1;
        Some(token.clone())
    }
}
