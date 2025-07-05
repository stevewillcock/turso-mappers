# turso-mappers

Row mappers for turso

See the [published crate](https://crates.io/crates/turso-mappers) and
the [documentation](https://docs.rs/crate/turso-mappers/latest) for more information.

- Allows you to map turso rows to structs
- Provides a `MapRows` trait with a `map_rows` method for easily mapping over `turso::Rows`
- Defines a `TryFromRow` trait for `turso::Row`
- Supports deriving the `TryFromRow` traits for structs via the turso-mappers-derive crate
- Requires the columns in the SQL query to be in the same order as the struct fields
- Currently maps by index in the TryFromRow implementation
- Currently only supports i64 and String types in the derive macro

## Usage

This is a work in progress. Currently, the following functionality is implemented.

- The `TryFromRow` derive macro is implemented (for simple cases only)
- `map_rows` from `MapRows` is implemented to allow mapping over rows

```rust

use turso_mappers::{TryFromRow, MapRows};

#[derive(TryFromRow)] // Derive the TryFromRow trait on our struct
pub struct Customer {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    // Note: Option<String> is not currently supported by the derive macro
    // pub description: Option<String>,
}

pub async fn print_customers(rows: turso::Rows) -> Result<(), Box<dyn std::error::Error>> {

    // Use the map_rows method to map each row to a Customer struct
    let customers: Vec<Customer> = rows
            .map_rows(Customer::try_from_row)
            .await?;

    for customer in customers {
        println!("Customer: {} - {:?} - {:?}", customer.id, customer.first_name, customer.last_name);
    }

    Ok(())
}


```

## TODO

- Add support for Option<T> types to handle null values
- Add support for more data types (currently only i64 and String are supported)
- Add an option to use named mapping (by column name) instead of index-based mapping
- Improve error messages and error handling
