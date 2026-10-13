//! A deliberately small PostgreSQL SELECT dialect. Unknown syntax is denied.
//! This layer complements server privileges; it is not a SQL sandbox by itself.
use sqlparser::{
    ast::{
        BinaryOperator, Expr, Ident, LimitClause, OrderByKind, SelectItem, SetExpr, Statement,
        TableFactor, UnaryOperator, Value,
    },
    dialect::PostgreSqlDialect,
    parser::Parser,
};

use crate::models::{AppError, AppResult, TableInfo};

pub const MAX_SQL_BYTES: usize = 32_768;
pub const MAX_ROWS: usize = 1_000;
pub const MAX_RESULT_BYTES: usize = 4 * 1024 * 1024;
pub const PAGE_SIZE: usize = 200;

pub struct ApprovedQuery(String);

impl ApprovedQuery {
    pub(crate) fn sql(&self) -> &str {
        &self.0
    }
}

pub fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn identifier(ident: &Ident) -> String {
    if ident.quote_style.is_some() {
        ident.value.clone()
    } else {
        ident.value.to_lowercase()
    }
}

pub fn approve(sql: &str, tables: &[TableInfo]) -> AppResult<ApprovedQuery> {
    if sql.is_empty() || sql.len() > MAX_SQL_BYTES {
        return Err(AppError::policy("Use a query between 1 and 32,768 bytes."));
    }
    let statements = Parser::new(&PostgreSqlDialect {})
        .with_recursion_limit(32)
        .try_with_sql(sql)
        .and_then(|mut parser| parser.parse_statements())
        .map_err(|_| {
            AppError::policy("This SQL could not be parsed as a supported PostgreSQL SELECT.")
        })?;
    if statements.len() != 1 {
        return Err(AppError::policy(
            "Run exactly one SELECT statement at a time.",
        ));
    }
    let Statement::Query(query) = &statements[0] else {
        return Err(AppError::policy(
            "Only SELECT is allowed. Writing and administration are disabled.",
        ));
    };
    let SetExpr::Select(select) = query.body.as_ref() else {
        return Err(AppError::policy(
            "Use a single SELECT; unions and nested queries are not supported yet.",
        ));
    };

    let table = if select.from.is_empty() {
        None
    } else {
        if select.from.len() != 1 || !select.from[0].joins.is_empty() {
            return Err(AppError::policy(
                "This version reads one approved table per query.",
            ));
        }
        let TableFactor::Table { name, .. } = &select.from[0].relation else {
            return Err(AppError::policy("Only approved base tables are supported."));
        };
        // Require a schema-qualified name; no search_path-dependent resolution.
        let parts = name
            .0
            .iter()
            .map(|part| part.as_ident())
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| AppError::policy("Unsupported table reference."))?;
        if parts.len() != 2 {
            return Err(AppError::policy(
                "Use a schema-qualified table name, for example public.orders.",
            ));
        }
        Some(
            tables
                .iter()
                .find(|t| t.schema == identifier(parts[0]) && t.name == identifier(parts[1]))
                .ok_or_else(|| {
                    AppError::policy("This table is outside the approved base-table scope.")
                })?,
        )
    };

    // Reconstruct only supported clauses. Comparing the canonical form catches
    // extra SELECT options, aliases, locking, INTO, CTEs, etc. without silently
    // discarding them. Never send the unvalidated original SQL to PostgreSQL.
    let mut projection = Vec::new();
    for item in &select.projection {
        match item {
            SelectItem::UnnamedExpr(expr) => {
                check_expr(expr, table, 0)?;
                projection.push(expr.to_string());
            }
            SelectItem::ExprWithAlias { expr, alias } => {
                check_expr(expr, table, 0)?;
                projection.push(format!("{expr} AS {alias}"));
            }
            SelectItem::Wildcard(options) if table.is_some() && options.to_string().is_empty() => {
                projection.push("*".into())
            }
            _ => {
                return Err(AppError::policy(
                    "Unsupported projection. Use columns, literals or *.",
                ));
            }
        }
    }
    let mut normalized = format!("SELECT {}", projection.join(", "));
    if table.is_some() {
        let TableFactor::Table { name, .. } = &select.from[0].relation else {
            unreachable!()
        };
        normalized.push_str(&format!(" FROM {name}"));
    }
    if let Some(expr) = &select.selection {
        check_expr(expr, table, 0)?;
        normalized.push_str(&format!(" WHERE {expr}"));
    }
    if let Some(order) = &query.order_by {
        let OrderByKind::Expressions(expressions) = &order.kind else {
            return Err(AppError::policy("Order by explicit columns."));
        };
        let mut items = Vec::new();
        for item in expressions {
            check_expr(&item.expr, table, 0)?;
            items.push(format!("{}{}", item.expr, item.options));
        }
        normalized.push_str(&format!(" ORDER BY {}", items.join(", ")));
    }
    if let Some(limit) = &query.limit_clause {
        let LimitClause::LimitOffset {
            limit,
            offset,
            limit_by,
        } = limit
        else {
            return Err(AppError::policy("Unsupported LIMIT syntax."));
        };
        if !limit_by.is_empty() {
            return Err(AppError::policy("LIMIT BY is unsupported."));
        }
        if let Some(limit) = limit {
            check_integer(limit, MAX_ROWS as u64)?;
            normalized.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = offset {
            check_integer(&offset.value, 100_000)?;
            normalized.push_str(&format!(" OFFSET {}", offset.value));
        }
    }
    if normalized != query.to_string() {
        return Err(AppError::policy(
            "Unsupported SQL clause. Use SELECT, WHERE, ORDER BY, LIMIT and OFFSET only.",
        ));
    }
    Ok(ApprovedQuery(normalized))
}

fn check_integer(expr: &Expr, maximum: u64) -> AppResult<()> {
    if let Expr::Value(value) = expr
        && let Value::Number(number, false) = &value.value
        && number.parse::<u64>().is_ok_and(|n| n <= maximum)
    {
        return Ok(());
    }
    Err(AppError::policy(format!(
        "Use an integer between 0 and {maximum} for LIMIT/OFFSET."
    )))
}

fn check_expr(expr: &Expr, table: Option<&TableInfo>, depth: usize) -> AppResult<()> {
    if depth > 24 {
        return Err(AppError::policy("Expression nesting is too deep."));
    }
    let check = |e| check_expr(e, table, depth + 1);
    match expr {
        Expr::Identifier(name)
            if table.is_some_and(|t| t.columns.iter().any(|c| c.name == identifier(name))) =>
        {
            Ok(())
        }
        Expr::Value(v) => match &v.value {
            Value::Number(_, false)
            | Value::SingleQuotedString(_)
            | Value::Boolean(_)
            | Value::Null => Ok(()),
            _ => Err(AppError::policy(
                "Unsupported literal or parameter. Use the filter builder for bound values.",
            )),
        },
        Expr::Nested(e)
        | Expr::IsNull(e)
        | Expr::IsNotNull(e)
        | Expr::IsTrue(e)
        | Expr::IsFalse(e) => check(e),
        Expr::UnaryOp {
            op: UnaryOperator::Not | UnaryOperator::Plus | UnaryOperator::Minus,
            expr,
        } => check(expr),
        Expr::BinaryOp {
            left,
            op:
                BinaryOperator::Eq
                | BinaryOperator::NotEq
                | BinaryOperator::Gt
                | BinaryOperator::GtEq
                | BinaryOperator::Lt
                | BinaryOperator::LtEq
                | BinaryOperator::And
                | BinaryOperator::Or
                | BinaryOperator::Plus
                | BinaryOperator::Minus
                | BinaryOperator::Multiply
                | BinaryOperator::Divide,
            right,
        } => {
            check(left)?;
            check(right)
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            check(expr)?;
            check(low)?;
            check(high)
        }
        Expr::InList { expr, list, .. } if list.len() <= 100 => {
            check(expr)?;
            for value in list {
                check(value)?;
            }
            Ok(())
        }
        Expr::Like {
            expr,
            pattern,
            escape_char: None,
            ..
        }
        | Expr::ILike {
            expr,
            pattern,
            escape_char: None,
            ..
        } => {
            check(expr)?;
            check(pattern)
        }
        _ => Err(AppError::policy(
            "Unsupported expression. Functions, casts, subqueries and custom operators are disabled in this version.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ColumnInfo;

    fn tables() -> Vec<TableInfo> {
        vec![TableInfo {
            schema: "public".into(),
            name: "orders".into(),
            estimated_rows: None,
            columns: ["id", "name", "total"]
                .iter()
                .map(|name| ColumnInfo {
                    name: (*name).into(),
                    data_type: "text".into(),
                    nullable: true,
                    primary_key: false,
                    editable: false,
                })
                .collect(),
        }]
    }

    #[test]
    fn accepts_supported_reads() {
        for sql in [
            "SELECT 1",
            "SELECT 'DELETE; DROP TABLE x' AS note",
            "SELECT * FROM public.orders LIMIT 200",
            "SELECT id, name FROM public.orders WHERE total > 100 AND name IS NOT NULL ORDER BY id DESC LIMIT 10 OFFSET 20",
            "SELECT name FROM public.orders WHERE name IN ('a', 'b')",
            "SELECT * FROM public.orders WHERE name ILIKE '%a%'",
            "-- comment\nSELECT \"id\" FROM \"public\".\"orders\";",
            "SELECT * FROM public.orders LIMIT ALL",
        ] {
            assert!(approve(sql, &tables()).is_ok(), "{sql}");
        }
    }

    #[test]
    fn denies_mutations_and_indirect_execution() {
        for sql in [
            "",
            "DELETE FROM public.orders",
            "TRUNCATE public.orders",
            "DROP TABLE public.orders",
            "SELECT 1; SELECT 2",
            "SELECT pg_sleep(1)",
            "SELECT set_config('transaction_read_only','off',false)",
            "SELECT nextval('x')",
            "SELECT * INTO copy FROM public.orders",
            "SELECT * FROM public.orders FOR UPDATE",
            "WITH d AS (DELETE FROM public.orders RETURNING *) SELECT * FROM d",
            "SELECT * FROM pg_catalog.pg_authid",
            "SELECT * FROM public.orders UNION SELECT * FROM public.orders",
            "SELECT * FROM public.orders o",
            "SELECT * FROM public.orders TABLESAMPLE SYSTEM (1)",
            "SELECT id::custom_type FROM public.orders",
            "SELECT (SELECT 1)",
            "SELECT * FROM public.orders LIMIT 1001",
            "SELECT * FROM orders",
            "SELECT * FROM public.orders WHERE id OPERATOR(public.=) 1",
            "EXPLAIN ANALYZE DELETE FROM public.orders",
            "SELECT * FROM public.orders WHERE name SIMILAR TO '.*'",
        ] {
            assert!(approve(sql, &tables()).is_err(), "must deny: {sql}");
        }
    }

    #[test]
    fn quotes_identifiers_and_caps_input() {
        assert_eq!(
            quote_identifier("x\"; DROP TABLE t;--"),
            "\"x\"\"; DROP TABLE t;--\""
        );
        assert!(approve(&"a".repeat(MAX_SQL_BYTES + 1), &tables()).is_err());
    }
}
