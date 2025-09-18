#![allow(clippy::uninlined_format_args)]

use std::collections::HashMap;
use std::future::Future;
use turso::{Column, Connection, IntoParams};
pub use turso_mappers_derive::TryFromRowByIndex;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDocTests;

#[derive(Debug)]
pub enum TursoMapperError {
    ColumnNotFound(String),
    InvalidType(String),
    NullValue(String),
    ConversionError(String),
    IoError(std::io::Error),
    TursoError(turso::Error),
}

impl From<turso::Error> for TursoMapperError {
    fn from(err: turso::Error) -> Self {
        TursoMapperError::TursoError(err)
    }
}

impl From<std::io::Error> for TursoMapperError {
    fn from(err: std::io::Error) -> Self {
        TursoMapperError::IoError(err)
    }
}

impl std::fmt::Display for TursoMapperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TursoMapperError::ColumnNotFound(msg) => write!(f, "Column not found: {}", msg),
            TursoMapperError::InvalidType(msg) => write!(f, "Invalid type: {}", msg),
            TursoMapperError::NullValue(msg) => write!(f, "Null value: {}", msg),
            TursoMapperError::ConversionError(msg) => write!(f, "Conversion error: {}", msg),
            TursoMapperError::IoError(err) => write!(f, "IO error: {}", err),
            TursoMapperError::TursoError(err) => write!(f, "Turso error: {}", err),
        }
    }
}

impl std::error::Error for TursoMapperError {}

pub type TursoMapperResult<T> = Result<T, TursoMapperError>;

pub trait MapRows {
    fn map_rows<F, T>(self, f: F) -> impl Future<Output = TursoMapperResult<Vec<T>>>
    where
        F: Fn(turso::Row) -> TursoMapperResult<T>,
        T: Send;
}

impl MapRows for turso::Rows {
    async fn map_rows<F, T>(mut self, f: F) -> TursoMapperResult<Vec<T>>
    where
        F: Fn(turso::Row) -> TursoMapperResult<T>,
        T: Send,
    {
        let mut rows = vec![];

        while let Some(row) = self.next().await? {
            let t: T = f(row)?;
            rows.push(t);
        }

        Ok(rows)
    }
}

pub trait TryFromRowByIndex: Send {
    fn try_from_row(row: turso::Row) -> TursoMapperResult<Self>
    where
        Self: Sized;
}

pub trait QueryAs {
    fn query_as<T>(&self, sql: &str, params: impl IntoParams) -> impl Future<Output = TursoMapperResult<Vec<T>>>
    where
        T: TryFromRowByIndex + Send;
}

impl QueryAs for Connection {
    async fn query_as<T>(&self, sql: &str, params: impl IntoParams) -> TursoMapperResult<Vec<T>>
    where
        T: TryFromRowByIndex + Send,
    {
        let rows = self.query(sql, params).await?;
        rows.map_rows(T::try_from_row).await
    }
}

pub struct ColumnIndices {
    column_names: HashMap<String, usize>,
}

impl ColumnIndices {
    pub fn new(columns: Vec<Column>) -> Self {
        let column_names = columns
            .iter()
            .enumerate()
            .map(|(i, column)| (column.name().to_string(), i))
            .collect::<HashMap<String, usize>>();

        ColumnIndices { column_names }
    }

    pub fn get_index(&self, column_name: &str) -> Result<usize, TursoMapperError> {
        self.column_names
            .get(column_name)
            .cloned()
            .ok_or_else(|| TursoMapperError::ColumnNotFound(column_name.to_string()))
    }
}

pub trait TryFromRowByName {
    fn try_from_row(row: turso::Row, column_indices: ColumnIndices) -> TursoMapperResult<Self>
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {
    use super::{ColumnIndices, QueryAs, TryFromRowByIndex, TursoMapperResult};
    use crate::{MapRows, TursoMapperError};
    use turso::{Builder, Row};
    use turso_core::Value;
    use turso_core::types::Text;

    struct CustomerWithManualTryFromRow {
        id: i64,
        name: String,
        value: f64,
        image: Vec<u8>,
    }

    impl TryFromRowByIndex for CustomerWithManualTryFromRow {
        fn try_from_row(row: Row) -> TursoMapperResult<Self> {
            Ok(CustomerWithManualTryFromRow {
                id: *row
                    .get_value(0)?
                    .as_integer()
                    .ok_or_else(|| TursoMapperError::ConversionError("id is not an integer".to_string()))?,
                name: row
                    .get_value(1)?
                    .as_text()
                    .ok_or_else(|| TursoMapperError::ConversionError("name is not a string".to_string()))?
                    .clone(),
                value: *row
                    .get_value(2)?
                    .as_real()
                    .ok_or_else(|| TursoMapperError::ConversionError("value is not a real".to_string()))?,
                image: row
                    .get_value(3)?
                    .as_blob()
                    .ok_or_else(|| TursoMapperError::ConversionError("image is not a blob".to_string()))?
                    .clone(),
            })
        }
    }

    #[derive(TryFromRowByIndex)]
    struct Customer {
        id: i64,
        name: String,
        value: f64,
        image: Vec<u8>,
    }

    #[derive(TryFromRowByIndex)]
    struct CustomerWithOptions {
        id: i64,
        name: String,
        optional_value: Option<f64>,
        optional_note: Option<String>,
        optional_data: Option<Vec<u8>>,
        optional_count: Option<i64>,
    }

    #[tokio::test]
    async fn can_get_values_using_map() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;

        conn.execute(
            "CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);",
            (),
        )
        .await?;
        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Charlie', 3.12, x'00010203');", ())
            .await?;
        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Sarah', 0.99, x'09080706');", ())
            .await?;

        let rows = conn.query("SELECT id, name, value, image FROM customer;", ()).await?;

        let customer_names = rows
            .map_rows(|row| {



                Ok(row
                    .get_value(1)?
                    .as_text()
                    .ok_or_else(|| TursoMapperError::ConversionError("name is not a string".to_string()))?
                    .clone())
            })
            .await?;

        assert_eq!(customer_names.len(), 2);

        assert_eq!(customer_names[0], "Charlie");
        assert_eq!(customer_names[1], "Sarah");

        Ok(())
    }

    #[tokio::test]
    async fn can_get_values_using_map_with_names() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;

        conn.execute(
            "CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);",
            (),
        )
        .await?;

        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Charlie', 3.12, x'00010203');", ())
            .await?;

        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Sarah', 0.99, x'09080706');", ())
            .await?;

        let mut statement = conn.prepare("SELECT id, name, value, image FROM customer;").await?;
        let rows = statement.query(()).await?;

        let column_indices = ColumnIndices::new(statement.columns());
        let name_column_index = column_indices.get_index("name")?;

        let customer_names = rows
            .map_rows(|row| {
                Ok(row
                    .get_value(name_column_index)?
                    .as_text()
                    .ok_or_else(|| TursoMapperError::ConversionError("name is not a string".to_string()))?
                    .clone())
            })
            .await?;

        assert_eq!(customer_names.len(), 2);

        assert_eq!(customer_names[0], "Charlie");
        assert_eq!(customer_names[1], "Sarah");

        Ok(())
    }

    #[tokio::test]
    async fn manual_try_from_row_impl_works() -> TursoMapperResult<()> {
        let row: Row = Row::from_iter(
            [
                Value::Integer(1),
                Value::Text(Text::new("Charlie")),
                Value::Float(3.12),
                Value::Blob(vec![1, 2, 3]),
            ]
            .iter(),
        );

        let customer = CustomerWithManualTryFromRow::try_from_row(row)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");
        assert_eq!(customer.value, 3.12);
        assert_eq!(customer.image, vec![1, 2, 3]);

        Ok(())
    }

    #[tokio::test]
    async fn derive_macro_try_from_row_impl_works() -> TursoMapperResult<()> {
        let row: Row = Row::from_iter(
            [
                Value::Integer(1),
                Value::Text(Text::new("Charlie")),
                Value::Float(3.12),
                Value::Blob(vec![1, 2, 3]),
            ]
            .iter(),
        );

        let customer = Customer::try_from_row(row)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");
        assert_eq!(customer.value, 3.12);
        assert_eq!(customer.image, vec![1, 2, 3]);

        Ok(())
    }

    #[tokio::test]
    async fn end_to_end_test_with_map_rows_and_try_from_row() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;

        conn.execute(
            "CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);",
            (),
        )
        .await?;

        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Charlie', 3.12, x'00010203');", ())
            .await?;

        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Sarah', 0.99, x'09080706');", ())
            .await?;

        let customers = conn
            .query("SELECT id, name, value, image FROM customer;", ())
            .await?
            .map_rows(Customer::try_from_row)
            .await?;

        assert_eq!(customers.len(), 2);

        assert_eq!(customers[0].id, 1);
        assert_eq!(customers[0].name, "Charlie");
        assert_eq!(customers[0].value, 3.12);
        assert_eq!(customers[0].image, vec![0, 1, 2, 3]);

        assert_eq!(customers[1].id, 2);
        assert_eq!(customers[1].name, "Sarah");
        assert_eq!(customers[1].value, 0.99);
        assert_eq!(customers[1].image, vec![9, 8, 7, 6]);

        Ok(())
    }

    #[tokio::test]
    async fn end_to_end_test_with_query_as() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;

        conn.execute(
            "CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);",
            (),
        )
        .await?;

        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Charlie', 3.12, x'00010203');", ())
            .await?;

        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Sarah', 0.99, x'09080706');", ())
            .await?;

        let customers = conn.query_as::<Customer>("SELECT id, name, value, image FROM customer;", ()).await?;

        assert_eq!(customers.len(), 2);

        assert_eq!(customers[0].id, 1);
        assert_eq!(customers[0].name, "Charlie");
        assert_eq!(customers[0].value, 3.12);
        assert_eq!(customers[0].image, vec![0, 1, 2, 3]);

        assert_eq!(customers[1].id, 2);
        assert_eq!(customers[1].name, "Sarah");
        assert_eq!(customers[1].value, 0.99);
        assert_eq!(customers[1].image, vec![9, 8, 7, 6]);

        Ok(())
    }

    #[tokio::test]
    async fn option_types_support_works() -> TursoMapperResult<()> {
        // Test with a manually created Row with some NULL values
        let row: Row = Row::from_iter(
            [
                Value::Integer(1),
                Value::Text(Text::new("Charlie")),
                Value::Float(3.12),
                Value::Null,
                Value::Blob(vec![1, 2, 3]),
                Value::Null,
            ]
            .iter(),
        );

        let customer = CustomerWithOptions::try_from_row(row)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");
        assert_eq!(customer.optional_value, Some(3.12));
        assert_eq!(customer.optional_note, None);
        assert_eq!(customer.optional_data, Some(vec![1, 2, 3]));
        assert_eq!(customer.optional_count, None);

        // Test with a Row with all non-NULL values
        let row: Row = Row::from_iter(
            [
                Value::Integer(2),
                Value::Text(Text::new("Sarah")),
                Value::Float(0.99),
                Value::Text(Text::new("Some note")),
                Value::Blob(vec![9, 8, 7, 6]),
                Value::Integer(42),
            ]
            .iter(),
        );

        let customer = CustomerWithOptions::try_from_row(row)?;

        assert_eq!(customer.id, 2);
        assert_eq!(customer.name, "Sarah");
        assert_eq!(customer.optional_value, Some(0.99));
        assert_eq!(customer.optional_note, Some("Some note".to_string()));
        assert_eq!(customer.optional_data, Some(vec![9, 8, 7, 6]));
        assert_eq!(customer.optional_count, Some(42));

        Ok(())
    }
}
