use crate::sql::{
    expr::{SqlExpr, SqlIdentifier},
    token::{Literal, Symbol, Token},
    token_stream::TokenStream,
};

pub struct Parser {
    stream: TokenStream,
}

impl Parser {
    pub fn new(stream: TokenStream) -> Parser {
        Parser { stream }
    }

    fn prefix(&mut self) -> Option<SqlExpr> {
        let span = self.stream.next()?;
        let expr = match span.token {
            Token::Keyword(_keyword) => todo!(),
            Token::Literal(literal) => match literal {
                Literal::Long(v) => SqlExpr::Long(v),
                Literal::Double(v) => SqlExpr::Double(v),
                Literal::String(v) => SqlExpr::String(v),
                Literal::Indentifier(ident) => SqlExpr::Indentifier(SqlIdentifier(ident)),
            },
            Token::Symbol(_symbol) => todo!(),
        };
        Some(expr)
    }

    fn infix(&mut self, lhs: SqlExpr, bp: u8) -> Option<SqlExpr> {
        let op = self.stream.next()?.token;
        let rhs = self.parse_expr(bp)?;
        let expr = match op {
            Token::Keyword(_keyword) => todo!(),
            Token::Literal(_literal) => todo!(),
            Token::Symbol(symbol) => SqlExpr::BinaryExpr {
                lhs: Box::new(lhs),
                op: symbol.to_string(),
                rhs: Box::new(rhs),
            },
        };
        Some(expr)
    }

    fn parse_expr(&mut self, min_power: u8) -> Option<SqlExpr> {
        let mut lhs = self.prefix()?;
        // TODO: token can be other than symbol
        while let Some(Token::Symbol(op)) = self.stream.peek().map(|s| s.token) {
            let bp = infix_power(op);
            if bp <= min_power {
                break;
            }
            lhs = self.infix(lhs, bp)?;
        }
        Some(lhs)
    }

    fn parse(&mut self) -> Option<SqlExpr> {
        self.parse_expr(0)
    }
}

// TODO: power other than symbol
pub fn infix_power(symbol: Symbol) -> u8 {
    match symbol {
        Symbol::Plus => 10,
        Symbol::Minus => 10,
        Symbol::Multiply => 20,
    }
}

// TODO: power other than symbol
pub fn prefix_power(symbol: Symbol) -> u8 {
    match symbol {
        Symbol::Plus => 10,
        Symbol::Minus => 10,
        Symbol::Multiply => 20,
    }
}

#[cfg(test)]
mod test {
    use insta::assert_debug_snapshot;

    use crate::sql::{
        error::ParseError,
        expr::{SqlExpr, SqlIdentifier},
        parser::Parser,
        tokenizer::Tokenizer,
    };

    fn parse(sql: &str) -> Result<SqlExpr, ParseError> {
        let tokenizer = Tokenizer::new(sql);
        let stream = tokenizer.stream()?;
        let mut parser = Parser::new(stream);
        let expr = parser.parse().unwrap();
        Ok(expr)
    }

    #[test]
    pub fn identifier() -> Result<(), ParseError> {
        let data = parse("employee")?;
        assert_eq!(
            data,
            SqlExpr::Indentifier(SqlIdentifier("employee".to_string()))
        );
        Ok(())
    }

    #[test]
    pub fn expr() -> Result<(), ParseError> {
        let data = parse("1 + 2")?;
        assert_debug_snapshot!(
            data,
            @r#"
        BinaryExpr {
            lhs: Long(
                1,
            ),
            op: "+",
            rhs: Long(
                2,
            ),
        }
        "#
        );
        Ok(())
    }

    #[test]
    pub fn expr_associate() -> Result<(), ParseError> {
        assert_debug_snapshot!(
        parse("1 + 2 * 3")?,
                    @r#"
        BinaryExpr {
            lhs: Long(
                1,
            ),
            op: "+",
            rhs: BinaryExpr {
                lhs: Long(
                    2,
                ),
                op: "*",
                rhs: Long(
                    3,
                ),
            },
        }
        "#
                );

        assert_debug_snapshot!(
        parse("1 * 2 + 3")?,
                    @r#"
        BinaryExpr {
            lhs: BinaryExpr {
                lhs: Long(
                    1,
                ),
                op: "*",
                rhs: Long(
                    2,
                ),
            },
            op: "+",
            rhs: Long(
                3,
            ),
        }
        "#
                );

        Ok(())
    }
}
