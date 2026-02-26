# turso-mappers

Map [turso](https://github.com/tursodatabase/turso) database rows to Rust structs. Provides traits and derive macros that eliminate boilerplate when converting query results into typed data structures.

Three approaches to row mapping:
- **Manual** (`TryFromRow`) — you control exactly how each column maps to each field
- **Positional** (`TryFromRowByIndex`) — derive macro, columns matched by position in the SELECT
- **By name** (`TryFromRowByName`) — derive macro, columns matched by name regardless of SELECT order

Each has a corresponding query extension trait (`QueryAs`, `QueryAsByIndex`, `QueryAsByName`) on `turso::Connection` for one-line query-and-map.

See the [published crate](https://crates.io/crates/turso-mappers) and
the [documentation](https://docs.rs/crate/turso-mappers/latest) for more information.

## Features

- `MapRows` trait for iterating over `turso::Rows` with a mapping closure
- Three mapping traits with derive macros and query helpers:
  - **`TryFromRow`** — manual impl, full control over row-to-struct mapping
  - **`TryFromRowByIndex`** — derive macro, maps columns by position
  - **`TryFromRowByName`** — derive macro, maps columns by name (order-independent)
- Supported column types: `i64`, `f64`, `String`, `Vec<u8>`, and `Option<T>` wrappers
- Query extension traits on `turso::Connection`: `QueryAs`, `QueryAsByIndex`, `QueryAsByName`

## MapRows — low-level row mapping

`MapRows::map_rows` applies a closure to each row, useful for ad-hoc queries or when you don't need a full struct mapping.

```rust
use turso_mappers::{MapRows, TursoMapperResult, TursoMapperError};
use turso::Builder;

#[tokio::main]
async fn main() -> TursoMapperResult<()> {
    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL);", ()).await?;
    conn.execute("INSERT INTO customer (name) VALUES ('Charlie');", ()).await?;
    conn.execute("INSERT INTO customer (name) VALUES ('Sarah');", ()).await?;

    let names = conn
        .query("SELECT id, name FROM customer;", ())
        .await?
        .map_rows(|row| {
            Ok(row.get_value(1)?.as_text()
                .ok_or_else(|| TursoMapperError::ConversionError("name is not text".to_string()))?
                .clone())
        })
        .await?;

    assert_eq!(names, vec!["Charlie", "Sarah"]);
    Ok(())
}
```

## TryFromRow — manual mapping

Implement `TryFromRow` for full control. Use `QueryAs::query_as` or `MapRows::map_rows` to execute.

```rust
use turso_mappers::{TryFromRow, TursoMapperResult, TursoMapperError, QueryAs};
use turso::{Builder, Row};

struct Customer {
    id: i64,
    name: String,
}

impl TryFromRow for Customer {
    fn try_from_row(row: Row) -> TursoMapperResult<Self> {
        Ok(Customer {
            id: *row.get_value(0)?.as_integer()
                .ok_or_else(|| TursoMapperError::ConversionError("id is not an integer".to_string()))?,
            name: row.get_value(1)?.as_text()
                .ok_or_else(|| TursoMapperError::ConversionError("name is not a string".to_string()))?.clone(),
        })
    }
}

#[tokio::main]
async fn main() -> TursoMapperResult<()> {
    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL);", ()).await?;
    conn.execute("INSERT INTO customer (name) VALUES ('Charlie');", ()).await?;

    let customers = conn.query_as::<Customer>("SELECT id, name FROM customer;", ()).await?;
    assert_eq!(customers[0].id, 1);
    assert_eq!(customers[0].name, "Charlie");
    Ok(())
}
```

## TryFromRowByIndex — derive, positional

Derive `TryFromRowByIndex` to map columns by position. Struct field order must match the SELECT column order. Use `QueryAsByIndex::query_as_by_index` to execute.

```rust
use turso_mappers::{TryFromRowByIndex, QueryAsByIndex, TursoMapperResult, TursoMapperError};
use turso::Builder;

#[derive(TryFromRowByIndex)]
struct Customer {
    id: i64,
    name: String,
    value: f64,
    description: Option<String>,
}

#[tokio::main]
async fn main() -> TursoMapperResult<()> {
    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, description TEXT);", ()).await?;
    conn.execute("INSERT INTO customer (name, value) VALUES ('Charlie', 3.12);", ()).await?;

    let customers = conn.query_as_by_index::<Customer>("SELECT id, name, value, description FROM customer;", ()).await?;
    assert_eq!(customers[0].id, 1);
    assert_eq!(customers[0].name, "Charlie");
    assert_eq!(customers[0].value, 3.12);
    assert_eq!(customers[0].description, None);
    Ok(())
}
```

## TryFromRowByName — derive, by name

Derive `TryFromRowByName` to map columns by name. Column order in the SELECT doesn't matter — fields are resolved by matching struct field names to column names. Use `QueryAsByName::query_as_by_name` to execute.

```rust
use turso_mappers::{TryFromRowByName, QueryAsByName, TursoMapperResult, TursoMapperError, ColumnIndices};
use turso::Builder;

#[derive(TryFromRowByName)]
struct Customer {
    id: i64,
    name: String,
    value: f64,
    description: Option<String>,
}

#[tokio::main]
async fn main() -> TursoMapperResult<()> {
    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, description TEXT);", ()).await?;
    conn.execute("INSERT INTO customer (name, value) VALUES ('Charlie', 3.12);", ()).await?;

    // Column order doesn't need to match struct field order
    let customers = conn.query_as_by_name::<Customer>("SELECT description, value, name, id FROM customer;", ()).await?;
    assert_eq!(customers[0].id, 1);
    assert_eq!(customers[0].name, "Charlie");
    assert_eq!(customers[0].value, 3.12);
    assert_eq!(customers[0].description, None);
    Ok(())
}
```

## TODO
- Improve error messages and error handling
- Reduce duplicated code in derive macro implementation
