#[derive(Debug, PartialEq, Clone)]
pub enum SqlExpr {
    // TODO: add correct class
    Indentifier(SqlIdentifier),
    String(String),
    Long(i64),
    Double(f64),
    BinaryExpr {
        lhs: Box<SqlExpr>,
        // TODO: op enum
        op: String,
        rhs: Box<SqlExpr>,
    },
    Alias {
        expr: Box<SqlExpr>,
        alias: SqlIdentifier,
    },
    Function {
        id: String,
        args: Vec<SqlExpr>,
    },
    Cast {
        expr: Box<SqlExpr>,
        data_type: SqlIdentifier,
    },
    Sort {
        expr: Box<SqlExpr>,
        // TODO: enum
        asc: bool,
    },
    Select(Select),
}

impl Eq for SqlExpr {}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SqlIdentifier(pub String);

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Select {
    pub projection: Vec<SqlExpr>,
    pub filter: Option<Box<SqlExpr>>,
    pub group_by: Vec<SqlExpr>,
    pub having: Option<Box<SqlExpr>>,
    pub table_name: SqlIdentifier,
    // TODO: add more fields
}
