# turso-mappers

Row mappers for turso

See the [published crate](https://crates.io/crates/turso-mappers) and
the [documentation](https://docs.rs/crate/turso-mappers/latest) for more information.

- Allows you to map turso rows to structs more easily
- Provides a `MapRows` trait with a `map_rows` method for easily mapping over `turso::Rows`
- Defines a `TryFromRow` trait for `turso::Row`
- Supports deriving the `TryFromRow` traits for structs via the turso-mappers-derive crate
- Currently requires the columns in the SQL query to be in the same order as the struct fields
- Currently maps by index in the TryFromRow implementation
- Currently only supports i64 and String types in the derive macro

## Usage

This is a work in progress. Currently, the following functionality is implemented.

- `map_rows` from `MapRows` is implemented to allow mapping over rows
- The `TryFromRow` derive macro is implemented (as a proof of concept, for simple cases only)

```rust
use turso_mappers::MapRows;
use turso_mappers::TryFromRow;
use turso_mappers::TursoMapperResult;
use turso_mappers::TursoMapperError;
use turso_core::types::Text;
use turso::Row;
use turso::Builder;

#[derive(TryFromRow)]
pub struct Customer {
    pub id: i64,
    pub name: String,
    // Note: Option<> is not currently supported by the derive macro
    // pub description: Option<String>,
}

#[tokio::main]
async fn main() -> TursoMapperResult<()> {

    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;

    conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL);", ()).await?;
    conn.execute("INSERT INTO customer (name) VALUES ('Charlie');", ()).await?;
    conn.execute("INSERT INTO customer (name) VALUES ('Sarah');", ()).await?;

    let customers = conn
        .query("SELECT id, name FROM customer;", ()).await?
        .map_rows(Customer::try_from_row).await?;

    assert_eq!(customers.len(), 2);
    assert_eq!(customers[0].id, 1);
    assert_eq!(customers[0].name, "Charlie");
    assert_eq!(customers[1].id, 2);
    assert_eq!(customers[1].name, "Sarah");

    Ok(())

}


```

## TODO

- Add support for Option<T> types to handle null values
- Add support for more data types (currently only i64 and String are supported)
- Add an option to use named mapping (by column name) instead of index-based mapping
- Improve error messages and error handling
