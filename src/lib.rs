#![allow(clippy::uninlined_format_args)]

use std::collections::HashMap;
use std::future::Future;
use turso::{Column, Connection, IntoParams};
pub use turso_mappers_derive::{TryFromRowByIndex, TryFromRowByName};

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

pub trait TryFromRow: Send {
    fn try_from_row(row: turso::Row) -> TursoMapperResult<Self>
    where
        Self: Sized;
}

pub trait QueryAs {
    fn query_as<T>(&self, sql: &str, params: impl IntoParams) -> impl Future<Output = TursoMapperResult<Vec<T>>>
    where
        T: TryFromRow + Send;
}

impl QueryAs for Connection {
    async fn query_as<T>(&self, sql: &str, params: impl IntoParams) -> TursoMapperResult<Vec<T>>
    where
        T: TryFromRow + Send,
    {
        let rows = self.query(sql, params).await?;
        rows.map_rows(T::try_from_row).await
    }
}

pub trait TryFromRowByName: Send {
    type Indices;
    fn resolve_indices(column_indices: &ColumnIndices) -> TursoMapperResult<Self::Indices>;
    fn try_from_row_by_name(row: turso::Row, indices: &Self::Indices) -> TursoMapperResult<Self>
    where
        Self: Sized;
}

pub trait QueryAsByName {
    fn query_as_by_name<T>(&self, sql: &str, params: impl IntoParams) -> impl Future<Output = TursoMapperResult<Vec<T>>>
    where
        T: TryFromRowByName + Send;
}

impl QueryAsByName for Connection {
    async fn query_as_by_name<T>(&self, sql: &str, params: impl IntoParams) -> TursoMapperResult<Vec<T>>
    where
        T: TryFromRowByName + Send,
    {
        let mut rows = self.query(sql, params).await?;
        let column_indices = ColumnIndices::new(rows.columns());
        let indices = T::resolve_indices(&column_indices)?;
        let mut results = vec![];
        while let Some(row) = rows.next().await? {
            results.push(T::try_from_row_by_name(row, &indices)?);
        }
        Ok(results)
    }
}

pub trait TryFromRowByIndex: Send {
    fn try_from_row_by_index(row: turso::Row) -> TursoMapperResult<Self>
    where
        Self: Sized;
}

pub trait QueryAsByIndex {
    fn query_as_by_index<T>(&self, sql: &str, params: impl IntoParams) -> impl Future<Output = TursoMapperResult<Vec<T>>>
    where
        T: TryFromRowByIndex + Send;
}

impl QueryAsByIndex for Connection {
    async fn query_as_by_index<T>(&self, sql: &str, params: impl IntoParams) -> TursoMapperResult<Vec<T>>
    where
        T: TryFromRowByIndex + Send,
    {
        let rows = self.query(sql, params).await?;
        rows.map_rows(T::try_from_row_by_index).await
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

#[cfg(test)]
mod tests {
    use super::{ColumnIndices, QueryAs, QueryAsByIndex, QueryAsByName, TryFromRow, TryFromRowByIndex, TryFromRowByName, TursoMapperResult};
    use crate::{MapRows, TursoMapperError};
    use turso::{Builder, Row};

    struct CustomerWithManualTryFromRow {
        id: i64,
        name: String,
        value: f64,
        image: Vec<u8>,
    }

    impl TryFromRowByIndex for CustomerWithManualTryFromRow {
        fn try_from_row_by_index(row: Row) -> TursoMapperResult<Self> {
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

    // Manual TryFromRow impl — positional access matching SELECT id, name, value, image
    struct CustomerManual {
        id: i64,
        name: String,
        value: f64,
        image: Vec<u8>,
    }

    impl TryFromRow for CustomerManual {
        fn try_from_row(row: Row) -> TursoMapperResult<Self> {
            Ok(CustomerManual {
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

    // Manual TryFromRow impl — positional access matching SELECT image, value, name, id
    struct CustomerManualReordered {
        id: i64,
        name: String,
        value: f64,
        image: Vec<u8>,
    }

    impl TryFromRow for CustomerManualReordered {
        fn try_from_row(row: Row) -> TursoMapperResult<Self> {
            Ok(CustomerManualReordered {
                image: row
                    .get_value(0)?
                    .as_blob()
                    .ok_or_else(|| TursoMapperError::ConversionError("image is not a blob".to_string()))?
                    .clone(),
                value: *row
                    .get_value(1)?
                    .as_real()
                    .ok_or_else(|| TursoMapperError::ConversionError("value is not a real".to_string()))?,
                name: row
                    .get_value(2)?
                    .as_text()
                    .ok_or_else(|| TursoMapperError::ConversionError("name is not a string".to_string()))?
                    .clone(),
                id: *row
                    .get_value(3)?
                    .as_integer()
                    .ok_or_else(|| TursoMapperError::ConversionError("id is not an integer".to_string()))?,
            })
        }
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
    async fn manual_try_from_row_by_index_impl() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);", ()).await?;
        conn.execute("INSERT INTO t (name, value, image) VALUES ('Charlie', 3.12, x'01020300');", ()).await?;

        let mut rows = conn.query("SELECT id, name, value, image FROM t;", ()).await?;
        let row = rows.next().await?.unwrap();
        let customer = CustomerWithManualTryFromRow::try_from_row_by_index(row)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");
        assert_eq!(customer.value, 3.12);
        assert_eq!(customer.image, vec![1, 2, 3, 0]);

        Ok(())
    }

    #[tokio::test]
    async fn derived_try_from_row_by_index_impl() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);", ()).await?;
        conn.execute("INSERT INTO t (name, value, image) VALUES ('Charlie', 3.12, x'01020300');", ()).await?;

        let mut rows = conn.query("SELECT id, name, value, image FROM t;", ()).await?;
        let row = rows.next().await?.unwrap();
        let customer = Customer::try_from_row_by_index(row)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");
        assert_eq!(customer.value, 3.12);
        assert_eq!(customer.image, vec![1, 2, 3, 0]);

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
            .map_rows(Customer::try_from_row_by_index)
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
    async fn end_to_end_test_with_query_as_by_index() -> TursoMapperResult<()> {
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

        let customers = conn.query_as_by_index::<Customer>("SELECT id, name, value, image FROM customer;", ()).await?;

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
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, optional_value REAL, optional_note TEXT, optional_data BLOB, optional_count INTEGER);",
            (),
        ).await?;

        // Row with some NULLs
        conn.execute("INSERT INTO t (name, optional_value, optional_data) VALUES ('Charlie', 3.12, x'010203');", ()).await?;
        // Row with all non-NULL
        conn.execute("INSERT INTO t (name, optional_value, optional_note, optional_data, optional_count) VALUES ('Sarah', 0.99, 'Some note', x'09080706', 42);", ()).await?;

        let customers = conn.query_as_by_index::<CustomerWithOptions>("SELECT id, name, optional_value, optional_note, optional_data, optional_count FROM t;", ()).await?;

        assert_eq!(customers[0].id, 1);
        assert_eq!(customers[0].name, "Charlie");
        assert_eq!(customers[0].optional_value, Some(3.12));
        assert_eq!(customers[0].optional_note, None);
        assert_eq!(customers[0].optional_data, Some(vec![1, 2, 3]));
        assert_eq!(customers[0].optional_count, None);

        assert_eq!(customers[1].id, 2);
        assert_eq!(customers[1].name, "Sarah");
        assert_eq!(customers[1].optional_value, Some(0.99));
        assert_eq!(customers[1].optional_note, Some("Some note".to_string()));
        assert_eq!(customers[1].optional_data, Some(vec![9, 8, 7, 6]));
        assert_eq!(customers[1].optional_count, Some(42));

        Ok(())
    }

    // --- TryFromRow (manual, simple) tests ---

    #[tokio::test]
    async fn try_from_row_manual_single_row() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);", ()).await?;
        conn.execute("INSERT INTO t (name, value, image) VALUES ('Charlie', 3.12, x'01020300');", ()).await?;

        let mut rows = conn.query("SELECT id, name, value, image FROM t;", ()).await?;
        let row = rows.next().await?.unwrap();
        let customer = CustomerManual::try_from_row(row)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");
        assert_eq!(customer.value, 3.12);
        assert_eq!(customer.image, vec![1, 2, 3, 0]);

        Ok(())
    }

    #[tokio::test]
    async fn query_as_end_to_end() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);", ()).await?;
        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Charlie', 3.12, x'00010203');", ()).await?;
        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Sarah', 0.99, x'09080706');", ()).await?;

        let customers = conn.query_as::<CustomerManual>("SELECT id, name, value, image FROM customer;", ()).await?;

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
    async fn query_as_column_reorder() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);", ()).await?;
        conn.execute("INSERT INTO t (name, value, image) VALUES ('Charlie', 3.12, x'01020300');", ()).await?;

        // Manual impl maps indices to match reordered SELECT
        let customers = conn.query_as::<CustomerManualReordered>("SELECT image, value, name, id FROM t;", ()).await?;

        assert_eq!(customers[0].id, 1);
        assert_eq!(customers[0].name, "Charlie");
        assert_eq!(customers[0].value, 3.12);
        assert_eq!(customers[0].image, vec![1, 2, 3, 0]);

        Ok(())
    }

    // --- By-name mapping tests ---

    #[derive(TryFromRowByName)]
    struct CustomerByName {
        id: i64,
        name: String,
        value: f64,
        image: Vec<u8>,
    }

    #[derive(TryFromRowByName)]
    struct CustomerByNameWithOptions {
        id: i64,
        name: String,
        optional_value: Option<f64>,
        optional_note: Option<String>,
        optional_data: Option<Vec<u8>>,
        optional_count: Option<i64>,
    }

    #[tokio::test]
    async fn derived_try_from_row_by_name_impl() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);", ()).await?;
        conn.execute("INSERT INTO t (name, value, image) VALUES ('Charlie', 3.12, x'01020300');", ()).await?;

        let mut rows = conn.query("SELECT id, name, value, image FROM t;", ()).await?;
        let column_indices = ColumnIndices::new(rows.columns());
        let indices = CustomerByName::resolve_indices(&column_indices)?;
        let row = rows.next().await?.unwrap();
        let customer = CustomerByName::try_from_row_by_name(row, &indices)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");
        assert_eq!(customer.value, 3.12);
        assert_eq!(customer.image, vec![1, 2, 3, 0]);

        Ok(())
    }

    #[tokio::test]
    async fn end_to_end_test_with_query_as_by_name() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);", ()).await?;
        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Charlie', 3.12, x'00010203');", ()).await?;
        conn.execute("INSERT INTO customer (name, value, image) VALUES ('Sarah', 0.99, x'09080706');", ()).await?;

        let customers = conn.query_as_by_name::<CustomerByName>("SELECT id, name, value, image FROM customer;", ()).await?;

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
    async fn query_as_with_column_reorder() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);", ()).await?;
        conn.execute("INSERT INTO t (name, value, image) VALUES ('Charlie', 3.12, x'01020300');", ()).await?;

        // SELECT columns in different order than struct fields
        let customers = conn.query_as_by_name::<CustomerByName>("SELECT image, value, name, id FROM t;", ()).await?;

        assert_eq!(customers[0].id, 1);
        assert_eq!(customers[0].name, "Charlie");
        assert_eq!(customers[0].value, 3.12);
        assert_eq!(customers[0].image, vec![1, 2, 3, 0]);

        Ok(())
    }

    #[tokio::test]
    async fn try_from_row_by_name_column_reorder() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);", ()).await?;
        conn.execute("INSERT INTO t (name, value, image) VALUES ('Charlie', 3.12, x'01020300');", ()).await?;

        // SELECT columns in different order — resolved automatically by name
        let mut rows = conn.query("SELECT image, value, name, id FROM t;", ()).await?;
        let column_indices = ColumnIndices::new(rows.columns());
        let indices = CustomerByName::resolve_indices(&column_indices)?;
        let row = rows.next().await?.unwrap();
        let customer = CustomerByName::try_from_row_by_name(row, &indices)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");
        assert_eq!(customer.value, 3.12);
        assert_eq!(customer.image, vec![1, 2, 3, 0]);

        Ok(())
    }

    #[tokio::test]
    async fn query_as_with_options() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, optional_value REAL, optional_note TEXT, optional_data BLOB, optional_count INTEGER);",
            (),
        ).await?;
        conn.execute("INSERT INTO t (name, optional_value, optional_data) VALUES ('Charlie', 3.12, x'010203');", ()).await?;
        conn.execute("INSERT INTO t (name, optional_value, optional_note, optional_data, optional_count) VALUES ('Sarah', 0.99, 'Some note', x'09080706', 42);", ()).await?;

        let customers = conn.query_as_by_name::<CustomerByNameWithOptions>("SELECT id, name, optional_value, optional_note, optional_data, optional_count FROM t;", ()).await?;

        assert_eq!(customers[0].id, 1);
        assert_eq!(customers[0].name, "Charlie");
        assert_eq!(customers[0].optional_value, Some(3.12));
        assert_eq!(customers[0].optional_note, None);
        assert_eq!(customers[0].optional_data, Some(vec![1, 2, 3]));
        assert_eq!(customers[0].optional_count, None);

        assert_eq!(customers[1].id, 2);
        assert_eq!(customers[1].name, "Sarah");
        assert_eq!(customers[1].optional_value, Some(0.99));
        assert_eq!(customers[1].optional_note, Some("Some note".to_string()));
        assert_eq!(customers[1].optional_data, Some(vec![9, 8, 7, 6]));
        assert_eq!(customers[1].optional_count, Some(42));

        Ok(())
    }

    #[tokio::test]
    async fn query_as_missing_column_error() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);", ()).await?;
        conn.execute("INSERT INTO t (name, value, image) VALUES ('Charlie', 3.12, x'01020300');", ()).await?;

        // Omit 'image' column -- should fail with ColumnNotFound
        let result = conn.query_as_by_name::<CustomerByName>("SELECT id, name, value FROM t;", ()).await;
        assert!(result.is_err());

        Ok(())
    }
}
