# turso-mappers

Row mappers for turso

See the [published crate](https://crates.io/crates/turso-mappers) and
the [documentation](https://docs.rs/crate/turso-mappers/latest) for more information.

- Allows you to map turso rows to structs more easily
- Provides a `MapRows` trait with a `map_rows` method for easily mapping over `turso::Rows`
- Defines a `TryFromRow` trait for `turso::Row`
- Supports deriving the `TryFromRow` traits for structs via the turso-mappers-derive crate
- The derive macro currently requires the columns in the SQL query to be in the same order as the struct fields
- The derive macro currently supports INTEGER (i64), TEXT (String), REAL (f64), and BLOB (Vec<u8>) types
- The derive macro does not currently support NULL (Option) types

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
    pub value: f64,
    pub image: Vec<u8>
    // Note: Option<> is not currently supported by the derive macro
    // pub description: Option<String>,
}

#[tokio::main]
async fn main() -> TursoMapperResult<()> {

    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;

    conn.execute("CREATE TABLE customer (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value REAL NOT NULL, image BLOB NOT NULL);", ()).await?;
    conn.execute("INSERT INTO customer (name, value, image) VALUES ('Charlie', 3.12, x'00010203');", ()).await?;
    conn.execute("INSERT INTO customer (name, value, image) VALUES ('Sarah', 0.99, x'09080706');", ()).await?;

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


```

## TODO

- Add support for Option<T> types to handle null values
- Add support for more data types (currently only i64 and String are supported)
- Add an option to use named mapping (by column name) instead of index-based mapping
- Improve error messages and error handling
