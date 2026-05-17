use anyhow::{Result, bail};

use crate::sql::{
    expr::{Select, SqlExpr, SqlIdentifier},
    token::{Keyword, Literal, Symbol, Token},
    token_stream::TokenStream,
};

pub struct Parser {
    stream: TokenStream,
}

impl Parser {
    pub fn new(stream: TokenStream) -> Parser {
        Parser { stream }
    }

    fn prefix(&mut self) -> Result<SqlExpr> {
        let token = self.stream.next_token()?;
        let expr = match token {
            Token::Keyword(keyword) => match keyword {
                Keyword::Select => self.select()?,
                Keyword::Cast => self.cast()?,
                Keyword::From => todo!(),
                Keyword::Where => todo!(),
                Keyword::As => todo!(),
                Keyword::And => todo!(),
                Keyword::Avg => SqlExpr::Indentifier(SqlIdentifier(Keyword::Avg.to_string())),
                Keyword::GroupBy => todo!(),
                Keyword::OrderBy => todo!(),
                Keyword::Having => todo!(),
            },
            Token::Literal(literal) => match literal {
                Literal::Long(v) => SqlExpr::Long(v),
                Literal::Double(v) => SqlExpr::Double(v),
                Literal::String(v) => SqlExpr::String(v),
                Literal::Identifier(ident) => SqlExpr::Indentifier(SqlIdentifier(ident)),
            },
            Token::Symbol(_symbol) => {
                todo!()
            }
        };
        Ok(expr)
    }

    fn infix(&mut self, lhs: SqlExpr, bp: u8) -> Result<SqlExpr> {
        let op = self.stream.next_token()?;
        let expr = match op {
            Token::Keyword(keyword) => match keyword {
                Keyword::Select => todo!(),
                Keyword::Cast => todo!(),
                Keyword::From => todo!(),
                Keyword::Where => todo!(),
                Keyword::As => match self.parse_expr(bp)? {
                    SqlExpr::Indentifier(alias) => SqlExpr::Alias {
                        expr: Box::new(lhs),
                        alias,
                    },
                    rhs => bail!("Expect Indentifier after `AS`, got {:?}", rhs),
                },
                Keyword::And => SqlExpr::BinaryExpr {
                    lhs: Box::new(lhs),
                    op: Keyword::And.to_string(),
                    rhs: Box::new(self.parse_expr(bp)?),
                },
                Keyword::Avg => todo!(),
                Keyword::GroupBy => todo!(),
                Keyword::OrderBy => todo!(),
                Keyword::Having => todo!(),
            },
            Token::Literal(_literal) => todo!(),
            Token::Symbol(symbol) => match symbol {
                Symbol::LParen => match lhs {
                    SqlExpr::Indentifier(SqlIdentifier(id)) => {
                        let exprs = self.expr_list()?;
                        self.stream.expect(Token::Symbol(Symbol::RParen))?;
                        SqlExpr::Function { id, args: exprs }
                    }
                    s => bail!("Expect a function, got `{:?}`", s),
                },
                symbol => SqlExpr::BinaryExpr {
                    lhs: Box::new(lhs),
                    op: symbol.to_string(),
                    rhs: Box::new(self.parse_expr(bp)?),
                },
            },
        };
        Ok(expr)
    }

    fn parse_expr(&mut self, min_power: u8) -> Result<SqlExpr> {
        let mut lhs = self.prefix()?;
        while let Ok(op) = self.stream.peek_token() {
            let Some(bp) = infix_power(&op) else {
                break;
            };
            if bp <= min_power {
                break;
            }
            lhs = self.infix(lhs, bp)?;
        }
        Ok(lhs)
    }

    pub fn parse(&mut self) -> Result<SqlExpr> {
        self.parse_expr(0)
    }

    fn select(&mut self) -> Result<SqlExpr> {
        let exprs = self.expr_list()?;
        self.stream.expect(Token::Keyword(Keyword::From))?;
        let Ok(Token::Literal(Literal::Identifier(table))) = self.stream.next_token() else {
            bail!(
                "Expect `FROM` table in select statement\n{}",
                self.stream.context()
            )
        };

        // WHERE
        let filter = match self.stream.consume(&[Token::Keyword(Keyword::Where)]) {
            true => {
                let expr = self.parse()?;
                Some(Box::new(expr))
            }
            false => None,
        };

        // GROUP BY
        let group_by = match self.stream.consume(&[Token::Keyword(Keyword::GroupBy)]) {
            true => self.expr_list()?,
            false => vec![],
        };

        // HAVING
        let having = match self.stream.consume(&[Token::Keyword(Keyword::Having)]) {
            true => {
                let expr = self.parse()?;
                Some(Box::new(expr))
            }
            false => None,
        };

        let select = SqlExpr::Select(Select {
            projection: exprs,
            filter,
            group_by,
            having,
            table_name: SqlIdentifier(table),
        });
        Ok(select)
    }

    fn cast(&mut self) -> Result<SqlExpr> {
        self.stream.expect(Token::Symbol(Symbol::LParen))?;
        let expr = self.parse()?;
        self.stream.expect(Token::Symbol(Symbol::RParen))?;

        let SqlExpr::Alias { expr, alias } = expr else {
            bail!("CAST should be AS Type")
        };

        let cast = SqlExpr::Cast {
            expr,
            data_type: alias,
        };
        Ok(cast)
    }

    fn expr_list(&mut self) -> Result<Vec<SqlExpr>> {
        let mut exprs = vec![];
        while let Ok(expr) = self.parse() {
            exprs.push(expr);
            match self.stream.peek_token() {
                Ok(Token::Symbol(Symbol::Comma)) => {
                    self.stream.next_token()?;
                    continue;
                }
                _ => break,
            }
        }
        Ok(exprs)
    }
}

// TODO: power other than symbol
pub fn infix_power(token: &Token) -> Option<u8> {
    match token {
        Token::Keyword(keyword) => match keyword {
            Keyword::Select => None,
            Keyword::From => None,
            Keyword::Where => None,
            Keyword::As => Some(10),
            Keyword::And => Some(10),
            Keyword::Cast => None,
            Keyword::Avg => todo!(),
            Keyword::GroupBy => None,
            Keyword::OrderBy => todo!(),
            Keyword::Having => None,
        },
        Token::Literal(_literal) => todo!(),
        Token::Symbol(symbol) => match symbol {
            Symbol::Plus => Some(50),
            Symbol::Minus => Some(50),
            Symbol::Multiply => Some(60),
            Symbol::Divide => Some(60),
            Symbol::Eq => Some(30),
            Symbol::Le => Some(40),
            Symbol::Ge => Some(40),
            Symbol::Comma => Some(0),
            Symbol::LParen => Some(70),
            Symbol::RParen => None,
        },
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use insta::assert_debug_snapshot;

    use crate::sql::{
        expr::{SqlExpr, SqlIdentifier},
        test::parse,
    };

    #[test]
    pub fn identifier() -> Result<()> {
        let data = parse("employee")?;
        assert_eq!(
            data,
            SqlExpr::Indentifier(SqlIdentifier("employee".to_string()))
        );
        Ok(())
    }

    #[test]
    fn expr_associate() -> Result<()> {
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

    #[test]
    fn select_1() -> Result<()> {
        assert_debug_snapshot!(parse(r#"
SELECT id, first_name, salary * 1.1 AS new_salary
FROM employee
WHERE state = 'CO'
"#)?, @r#"
        Select(
            Select {
                projection: [
                    Indentifier(
                        SqlIdentifier(
                            "id",
                        ),
                    ),
                    Indentifier(
                        SqlIdentifier(
                            "first_name",
                        ),
                    ),
                    Alias {
                        expr: BinaryExpr {
                            lhs: Indentifier(
                                SqlIdentifier(
                                    "salary",
                                ),
                            ),
                            op: "*",
                            rhs: Double(
                                1.1,
                            ),
                        },
                        alias: SqlIdentifier(
                            "new_salary",
                        ),
                    },
                ],
                filter: Some(
                    BinaryExpr {
                        lhs: Indentifier(
                            SqlIdentifier(
                                "state",
                            ),
                        ),
                        op: "=",
                        rhs: String(
                            "CO",
                        ),
                    },
                ),
                group_by: [],
                having: None,
                table_name: SqlIdentifier(
                    "employee",
                ),
            },
        )
        "#);
        Ok(())
    }

    #[test]
    fn select_2() -> Result<()> {
        assert_debug_snapshot!(parse(r#"
SELECT id, first_name, salary/12 AS monthly_salary
FROM employee
WHERE state = 'CO' AND monthly_salary > 1000
        "#)?, @r#"
        Select(
            Select {
                projection: [
                    Indentifier(
                        SqlIdentifier(
                            "id",
                        ),
                    ),
                    Indentifier(
                        SqlIdentifier(
                            "first_name",
                        ),
                    ),
                    Alias {
                        expr: BinaryExpr {
                            lhs: Indentifier(
                                SqlIdentifier(
                                    "salary",
                                ),
                            ),
                            op: "/",
                            rhs: Long(
                                12,
                            ),
                        },
                        alias: SqlIdentifier(
                            "monthly_salary",
                        ),
                    },
                ],
                filter: Some(
                    BinaryExpr {
                        lhs: BinaryExpr {
                            lhs: Indentifier(
                                SqlIdentifier(
                                    "state",
                                ),
                            ),
                            op: "=",
                            rhs: String(
                                "CO",
                            ),
                        },
                        op: "AND",
                        rhs: BinaryExpr {
                            lhs: Indentifier(
                                SqlIdentifier(
                                    "monthly_salary",
                                ),
                            ),
                            op: ">",
                            rhs: Long(
                                1000,
                            ),
                        },
                    },
                ),
                group_by: [],
                having: None,
                table_name: SqlIdentifier(
                    "employee",
                ),
            },
        )
        "#);

        Ok(())
    }

    #[test]
    fn select_3() -> Result<()> {
        assert_debug_snapshot!(parse(r#"
SELECT department, AVG(salary) AS avg_salary
FROM employee
WHERE state = 'CO'
GROUP BY department
HAVING avg_salary > 50000
        "#)?, @r#"
        Select(
            Select {
                projection: [
                    Indentifier(
                        SqlIdentifier(
                            "department",
                        ),
                    ),
                    Alias {
                        expr: Function {
                            id: "AVG",
                            args: [
                                Indentifier(
                                    SqlIdentifier(
                                        "salary",
                                    ),
                                ),
                            ],
                        },
                        alias: SqlIdentifier(
                            "avg_salary",
                        ),
                    },
                ],
                filter: Some(
                    BinaryExpr {
                        lhs: Indentifier(
                            SqlIdentifier(
                                "state",
                            ),
                        ),
                        op: "=",
                        rhs: String(
                            "CO",
                        ),
                    },
                ),
                group_by: [
                    Indentifier(
                        SqlIdentifier(
                            "department",
                        ),
                    ),
                ],
                having: Some(
                    BinaryExpr {
                        lhs: Indentifier(
                            SqlIdentifier(
                                "avg_salary",
                            ),
                        ),
                        op: ">",
                        rhs: Long(
                            50000,
                        ),
                    },
                ),
                table_name: SqlIdentifier(
                    "employee",
                ),
            },
        )
        "#);

        Ok(())
    }
}
