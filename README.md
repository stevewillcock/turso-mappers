# turso-mappers

Row mappers for turso

See the [published crate](https://crates.io/crates/turso-mappers) and
the [documentation](https://docs.rs/crate/turso-mappers/latest) for more information.

- Allows you to map turso rows to structs
- Defines a `TryFromRow` trait for `turso::Row`
- Supports deriving the `TryFromRow` traits for structs via the turso-mappers-derive crate
- Requires the columns in the SQL query to be in the same order as the struct fields
- Handles null values where these map to Option<T> fields in the struct
- Currently maps by name in FromRowBorrowed and by index in FromRowOwned

## Usage

This is a work in progress. Currently, the following functionality is implemented.

- The `TryFromRow` derive macro is implemented (for simple cases only)
- `map_rows` from `MapRows` is implemented to allow mapping over rows

```rust

use turso_mappers::TryFromRow;

#[derive(TryFromRow)] // Derive the FromRow trait on our struct
pub struct Customer {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
    pub description: Option<String>,
}

pub async fn print_customers(rows: Vec<turso::Row>) -> Result<(), Box<dyn std::error::Error>> {
    
    // Now we can call the try_from_row method on each row to get a Customer struct
    let customers: Vec<Customer> = rows
            .into_iter()
            .map(Customer::try_from_row).collect::<Result<Vec<Customer>, _>>()?;

    for customer in customers {
        println!("Customer: {} - {:?} - {:?}", customer.id, customer.first_name, customer.last_name);
    }

    Ok(())
}


```

## TODO

- Handle optional values
- Add an option to use named mapping to validate the row names in the returned query result set against the struct field for safety
- Improve error messages
