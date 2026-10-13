//! Typed, explicit human edits. Separate from the read-only SQL policy.
use crate::{
    models::*,
    policy::quote_identifier,
    postgres::{Database, query_error},
};
use std::collections::BTreeSet;
use tokio_postgres::types::ToSql;

impl Database {
    pub async fn save(&self, schema: &str, request: &SaveRequest) -> AppResult<SaveResult> {
        if !self.manual_edits {
            return Err(AppError::policy("Manual editing is disabled."));
        }
        validate_size(request)?;
        self.preflight().await?;
        // Reads use their own read-only connections. Only this typed path starts
        // a write transaction; dropping this dedicated connection rolls it back.
        self.client
            .batch_execute("ROLLBACK; BEGIN READ WRITE")
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
        let tables = self.tables(schema).await?;
        let table = tables
            .iter()
            .find(|t| t.name == request.table)
            .ok_or_else(|| AppError::policy("This table is outside the supported scope."))?;
        let keys: Vec<_> = table.columns.iter().filter(|c| c.primary_key).collect();
        if keys.is_empty() {
            return Err(AppError::policy(
                "Editing requires a primary key to identify each row.",
            ));
        }
        let mut seen = BTreeSet::new();
        for row in &request.rows {
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
                    "{} = ${}::pg_catalog.text::pg_catalog.{}",
                    quote_identifier(name),
                    values.len(),
                    quote_identifier(&column.data_type)
                ));
            }
            let mut predicates = Vec::new();
            for column in &table.columns {
                values.push(row.original[&column.name].clone());
                predicates.push(format!(
                    "{} IS NOT DISTINCT FROM ${}::pg_catalog.text::pg_catalog.{}",
                    quote_identifier(&column.name),
                    values.len(),
                    quote_identifier(&column.data_type)
                ));
            }
            let sql = format!(
                "UPDATE ONLY {table_sql} SET {} WHERE {}",
                assignments.join(", "),
                predicates.join(" AND ")
            );
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
            updated_rows: request.rows.len(),
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
        || request.rows.iter().any(|r| r.changes.is_empty())
    {
        return Err(AppError::new(
            "EDIT_LIMIT",
            "Save 1–100 changed rows, at most 500 changed cells and 4 MiB at a time.",
        ));
    }
    Ok(())
}
