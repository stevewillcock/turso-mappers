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

/// A Result type alias for TursoMapperError
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

/// Defines a conversion from a turso::Row to a struct.
pub trait TryFromRow {
    /// Try to convert from a turso::Row to a struct. Returns a Result using the turso::error::Error type.
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

    struct CustomerWithManualMapping {
        id: i64,
        name: String,
    }

    #[derive(TryFromRow)]
    struct CustomerWithDeriveMacro {
        id: i64,
        name: String,
    }

    // Manual TryFromRow implementation for Customer
    impl TryFromRow for CustomerWithManualMapping {
        fn try_from_row(row: turso::Row) -> TursoMapperResult<Self> {
            Ok(CustomerWithManualMapping {
                id: *row
                    .get_value(0)?
                    .as_integer()
                    .ok_or_else(|| TursoMapperError::ConversionError("id is not an integer".to_string()))?,
                name: row
                    .get_value(1)?
                    .as_text()
                    .ok_or_else(|| TursoMapperError::ConversionError("name is not a string".to_string()))?
                    .clone(),
            })
        }
    }

    #[tokio::test]
    async fn can_get_structs_manually() -> Result<(), TursoMapperError> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;

        conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL);", ()).await?;
        conn.execute("INSERT INTO customer (name) VALUES ('Charlie');", ()).await?;
        conn.execute("INSERT INTO customer (name) VALUES ('Sarah');", ()).await?;

        let mut rows = conn.query("SELECT * FROM customer;", ()).await?;

        let mut customers = vec![];

        while let Some(row) = rows.next().await? {
            customers.push(CustomerWithManualMapping {
                id: *row
                    .get_value(0)?
                    .as_integer()
                    .ok_or_else(|| TursoMapperError::ConversionError("id is not an integer".to_string()))?,
                name: row
                    .get_value(1)?
                    .as_text()
                    .ok_or_else(|| TursoMapperError::ConversionError("name is not a string".to_string()))?
                    .clone(),
            });
        }

        assert!(rows.next().await?.is_none());

        assert_eq!(customers.len(), 2);
        assert_eq!(customers[0].id, 1);
        assert_eq!(customers[1].id, 2);
        assert_eq!(customers[0].name, "Charlie");
        assert_eq!(customers[1].name, "Sarah");

        Ok(())
    }

    #[tokio::test]
    async fn can_get_structs_using_map() -> Result<(), TursoMapperError> {
        let db = Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;

        conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL);", ()).await?;
        conn.execute("INSERT INTO customer (name) VALUES ('Charlie');", ()).await?;
        conn.execute("INSERT INTO customer (name) VALUES ('Sarah');", ()).await?;

        let rows = conn.query("SELECT * FROM customer;", ()).await?;

        let customers = rows
            .map_rows(|row| {
                Ok(CustomerWithManualMapping {
                    id: *row
                        .get_value(0)?
                        .as_integer()
                        .ok_or_else(|| TursoMapperError::ConversionError("id is not an integer".to_string()))?,
                    name: row
                        .get_value(1)?
                        .as_text()
                        .ok_or_else(|| TursoMapperError::ConversionError("name is not a string".to_string()))?
                        .clone(),
                })
            })
            .await?;

        assert_eq!(customers.len(), 2);
        assert_eq!(customers[0].id, 1);
        assert_eq!(customers[1].id, 2);
        assert_eq!(customers[0].name, "Charlie");
        assert_eq!(customers[1].name, "Sarah");

        Ok(())
    }

    #[tokio::test]
    async fn can_map_row_manually() -> Result<(), TursoMapperError> {
        let row: Row = Row::from_iter([turso_core::Value::Integer(1), turso_core::Value::Text(Text::new("Charlie"))].iter());

        let customer = CustomerWithManualMapping {
            id: *row
                .get_value(0)?
                .as_integer()
                .ok_or_else(|| TursoMapperError::ConversionError("id is not an integer".to_string()))?,
            name: row
                .get_value(1)?
                .as_text()
                .ok_or_else(|| TursoMapperError::ConversionError("name is not a string".to_string()))?
                .clone(),
        };

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");

        Ok(())
    }

    #[tokio::test]
    async fn can_map_row_with_manual_try_from_row_impl() -> Result<(), TursoMapperError> {
        let row: Row = Row::from_iter([turso_core::Value::Integer(1), turso_core::Value::Text(Text::new("Charlie"))].iter());

        let customer = CustomerWithManualMapping::try_from_row(row)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");

        Ok(())
    }

    #[tokio::test]
    async fn can_map_row_with_derive_macro() -> Result<(), TursoMapperError> {
        let row: Row = Row::from_iter([turso_core::Value::Integer(1), turso_core::Value::Text(Text::new("Charlie"))].iter());

        let customer = CustomerWithDeriveMacro::try_from_row(row)?;

        assert_eq!(customer.id, 1);
        assert_eq!(customer.name, "Charlie");

        Ok(())
    }
}
