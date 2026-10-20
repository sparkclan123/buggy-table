//! Typed, explicit human edits. Separate from the read-only SQL policy.
use crate::{
    models::*,
    policy::quote_identifier,
    postgres::{Database, column_type_sql, query_error, text_comparison},
};
use std::collections::BTreeSet;
use tokio_postgres::types::ToSql;

impl Database {
    pub async fn save(&self, schema: &str, request: &SaveRequest) -> AppResult<SaveResult> {
        self.prevent_reuse();
        if !self.manual_edits {
            return Err(AppError::policy("Manual editing is disabled."));
        }
        validate_size(request)?;
        self.preflight().await?;
        // Reads use read-only sessions. Only this dedicated typed path starts
        // a write transaction; dropping this dedicated connection rolls it back.
        self.client
            .batch_execute("BEGIN READ WRITE")
            .await
            .map_err(query_error)?;
        let table_sql = format!(
            "{}.{}",
            quote_identifier(schema),
            quote_identifier(&request.table)
        );
        // Hold the relation against concurrent DDL before inspecting its columns.
        self.client
            .batch_execute(&format!("LOCK TABLE {table_sql} IN ACCESS SHARE MODE"))
            .await
            .map_err(query_error)?;
        let table = self.approved_table(schema, &request.table).await?;
        let keys: Vec<_> = table.columns.iter().filter(|c| c.primary_key).collect();
        if keys.is_empty() {
            return Err(AppError::policy(
                "Editing requires a primary key to identify each row.",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut deleted_rows = 0;
        let mut inserted_rows = 0;
        for row in &request.rows {
            if row.insert {
                if !table.insertable {
                    return Err(AppError::policy(
                        "Inserting requires INSERT permission on a supported base table with a primary key.",
                    ));
                }
                let mut names = Vec::new();
                let mut expressions = Vec::new();
                let mut values = Vec::new();
                for (name, value) in &row.changes {
                    let column = table.columns.iter().find(|c| c.name == *name && c.insertable)
                        .ok_or_else(|| AppError::policy("A new-row column is generated or lacks INSERT permission. Reload the table."))?;
                    if value.is_none() && !column.nullable {
                        return Err(AppError::policy("A required column cannot be set to NULL."));
                    }
                    names.push(quote_identifier(name));
                    values.push(value.clone());
                    expressions.push(format!(
                        "${}::pg_catalog.text::{}",
                        values.len(),
                        column_type_sql(column)
                    ));
                }
                let sql = if names.is_empty() {
                    format!("INSERT INTO {table_sql} DEFAULT VALUES")
                } else {
                    format!(
                        "INSERT INTO {table_sql} ({}) VALUES ({})",
                        names.join(", "),
                        expressions.join(", ")
                    )
                };
                let params: Vec<&(dyn ToSql + Sync)> =
                    values.iter().map(|v| v as &(dyn ToSql + Sync)).collect();
                if self
                    .client
                    .execute(&sql, &params)
                    .await
                    .map_err(query_error)?
                    != 1
                {
                    return Err(AppError::new(
                        "EDIT_CONFLICT",
                        "An insert did not create exactly one row. No edits in this save were committed.",
                    ));
                }
                inserted_rows += 1;
                continue;
            }
            if row.delete && !table.deletable {
                return Err(AppError::policy(
                    "Deleting requires DELETE permission on a supported base table with a primary key.",
                ));
            }
            if row.original.len() != table.columns.len()
                || table
                    .columns
                    .iter()
                    .any(|c| !row.original.contains_key(&c.name))
            {
                return Err(AppError::policy(
                    "The original row must contain every current table column. Reload the table.",
                ));
            }
            let identity: Vec<_> = keys.iter().map(|c| row.original[&c.name].clone()).collect();
            if identity.iter().any(Option::is_none) || !seen.insert(identity) {
                return Err(AppError::policy(
                    "Each row must have a unique, non-null primary key.",
                ));
            }
            let mut values: Vec<Option<String>> = Vec::new();
            let mut assignments = Vec::new();
            for (name, value) in &row.changes {
                let column = table
                    .columns
                    .iter()
                    .find(|c| c.name == *name)
                    .filter(|c| c.editable && !c.primary_key)
                    .ok_or_else(|| {
                        AppError::policy("A changed column is not editable with this role.")
                    })?;
                if value.is_none() && !column.nullable {
                    return Err(AppError::policy("A required column cannot be set to NULL."));
                }
                values.push(value.clone());
                assignments.push(format!(
                    "{} = ${}::pg_catalog.text::{}",
                    quote_identifier(name),
                    values.len(),
                    column_type_sql(column)
                ));
            }
            let mut predicates = Vec::new();
            for column in &table.columns {
                values.push(row.original[&column.name].clone());
                if text_comparison(column) {
                    // json has no equality operator. Compare the exact displayed
                    // representation, preserving whitespace/duplicate-key changes.
                    predicates.push(format!(
                        "{}::pg_catalog.text IS NOT DISTINCT FROM ${}::pg_catalog.text",
                        quote_identifier(&column.name),
                        values.len()
                    ));
                    continue;
                }
                predicates.push(format!(
                    "{} IS NOT DISTINCT FROM ${}::pg_catalog.text::{}",
                    quote_identifier(&column.name),
                    values.len(),
                    column_type_sql(column)
                ));
            }
            let sql = if row.delete {
                deleted_rows += 1;
                format!(
                    "DELETE FROM ONLY {table_sql} WHERE {}",
                    predicates.join(" AND ")
                )
            } else {
                format!(
                    "UPDATE ONLY {table_sql} SET {} WHERE {}",
                    assignments.join(", "),
                    predicates.join(" AND ")
                )
            };
            let params: Vec<&(dyn ToSql + Sync)> =
                values.iter().map(|v| v as &(dyn ToSql + Sync)).collect();
            let affected = self
                .client
                .execute(&sql, &params)
                .await
                .map_err(query_error)?;
            if affected != 1 {
                return Err(AppError::new(
                    "EDIT_CONFLICT",
                    "A row changed or was removed since you loaded it. No edits in this save were committed. Discard and reload to compare.",
                ));
            }
        }
        self.client.batch_execute("COMMIT").await.map_err(|_| AppError::new(
            "SAVE_UNCERTAIN", "The commit response was lost or rejected. Reload the table to check the database before editing again."))?;
        Ok(SaveResult {
            updated_rows: request.rows.len() - deleted_rows - inserted_rows,
            deleted_rows,
            inserted_rows,
        })
    }
}

fn validate_size(request: &SaveRequest) -> AppResult<()> {
    let cells: usize = request.rows.iter().map(|r| r.changes.len()).sum();
    let bytes: usize = request
        .rows
        .iter()
        .flat_map(|r| r.original.iter().chain(r.changes.iter()))
        .map(|(k, v)| k.len() + v.as_ref().map_or(0, String::len))
        .sum();
    if request.table.is_empty()
        || request.table.len() > 253
        || request.rows.is_empty()
        || request.rows.len() > 100
        || cells > 500
        || bytes > 4 * 1024 * 1024
        || request.rows.iter().any(|r| {
            if r.insert {
                r.delete || !r.original.is_empty()
            } else {
                r.delete != r.changes.is_empty()
            }
        })
    {
        return Err(AppError::new(
            "EDIT_LIMIT",
            "Save 1–100 changed rows, at most 500 changed cells and 4 MiB at a time.",
        ));
    }
    Ok(())
}
