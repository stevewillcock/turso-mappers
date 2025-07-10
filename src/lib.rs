#![allow(clippy::uninlined_format_args)]

pub use turso_mappers_derive::TryFromRow;

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

pub trait TryFromRow {
    fn try_from_row(row: turso::Row) -> TursoMapperResult<Self>
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {
    use super::{TryFromRow, TursoMapperResult};
    use crate::{MapRows, TursoMapperError};
    use turso::{Builder, Row};
    use turso_core::types::Text;

    struct CustomerWithManualTryFromRow {
        id: i64,
        name: String,
        value: f64,
    }

    impl TryFromRow for CustomerWithManualTryFromRow {
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
            })
        }
    }

    #[derive(TryFromRow)]
    struct Customer {
        id: i64,
        name: String,
        value: f64,
    }

    #[tokio::test]
    async fn can_get_structs_using_map() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;

        conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL);", ()).await?;
        conn.execute("INSERT INTO customer (name, value) VALUES ('Charlie', 3.12);", ()).await?;
        conn.execute("INSERT INTO customer (name, value) VALUES ('Sarah', 0.99);", ()).await?;

        let rows = conn.query("SELECT id, name, value FROM customer;", ()).await?;

        let customers = rows
            .map_rows(|row| {
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
                })
            })
            .await?;

        assert_eq!(customers.len(), 2);

        assert_eq!(customers[0].id, 1);
        assert_eq!(customers[0].name, "Charlie");
        assert_eq!(customers[0].value, 3.12);

        assert_eq!(customers[1].id, 2);
        assert_eq!(customers[1].name, "Sarah");
        assert_eq!(customers[1].value, 0.99);

        Ok(())
    }

    #[tokio::test]
    async fn manual_try_from_row_impl_works() -> TursoMapperResult<()> {
        let row: Row = Row::from_iter([turso_core::Value::Integer(1), turso_core::Value::Text(Text::new("Charlie")), turso_core::Value::Float(3.12)].iter());

        let customer = CustomerWithManualTryFromRow::try_from_row(row)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");
        assert_eq!(customer.value, 3.12);

        Ok(())
    }

    #[tokio::test]
    async fn derive_macro_try_from_row_impl_works() -> TursoMapperResult<()> {
        let row: Row = Row::from_iter([turso_core::Value::Integer(1), turso_core::Value::Text(Text::new("Charlie")), turso_core::Value::Float(3.12)].iter());

        let customer = Customer::try_from_row(row)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");
        assert_eq!(customer.value, 3.12);

        Ok(())
    }

    #[tokio::test]
    async fn end_to_end_test_with_manual_mapping() -> TursoMapperResult<()> {
        // Note that this is not testing anything in this crate, it's just here as a functional baseline and to compare to
        // end_to_end_test_with_map_rows_and_try_from_row

        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;

        conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL);", ())
            .await?;
        conn.execute("INSERT INTO customer (name, value) VALUES ('Charlie', 3.12);", ()).await?;
        conn.execute("INSERT INTO customer (name, value) VALUES ('Sarah', 0.99);", ()).await?;

        let mut rows = conn.query("SELECT id, name, value FROM customer;", ()).await?;

        let mut customers = vec![];

        while let Some(row) = rows.next().await? {
            customers.push(Customer {
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
            });
        }

        assert_eq!(customers.len(), 2);

        assert_eq!(customers[0].id, 1);
        assert_eq!(customers[0].name, "Charlie");
        assert_eq!(customers[0].value, 3.12);

        assert_eq!(customers[1].id, 2);
        assert_eq!(customers[1].name, "Sarah");
        assert_eq!(customers[1].value, 0.99);

        Ok(())
    }

    #[tokio::test]
    async fn end_to_end_test_with_map_rows_and_try_from_row() -> TursoMapperResult<()> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;

        conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL);", ()).await?;
        conn.execute("INSERT INTO customer (name, value) VALUES ('Charlie', 3.12);", ()).await?;
        conn.execute("INSERT INTO customer (name, value) VALUES ('Sarah', 0.99);", ()).await?;

        let customers = conn.query("SELECT id, name, value FROM customer;", ()).await?.map_rows(Customer::try_from_row).await?;

        assert_eq!(customers.len(), 2);

        assert_eq!(customers[0].id, 1);
        assert_eq!(customers[0].name, "Charlie");
        assert_eq!(customers[0].value, 3.12);

        assert_eq!(customers[1].id, 2);
        assert_eq!(customers[1].name, "Sarah");

        Ok(())
    }
}
