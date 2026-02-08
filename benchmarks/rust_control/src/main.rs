use std::time::Instant;

use async_graphql::dynamic::{Field, FieldFuture, FieldValue, Object, Schema, TypeRef};

struct RowData {
    a: i64,
    cells: Vec<i64>,
}

#[tokio::main]
async fn main() {
    let cell = Object::new("Cell").field(Field::new(
        "value",
        TypeRef::named_nn(TypeRef::STRING),
        |ctx| {
            FieldFuture::new(async move {
                let j = ctx.parent_value.try_downcast_ref::<i64>()?;
                tokio::task::yield_now().await;
                Ok(Some(FieldValue::value(format!("Cell {j}"))))
            })
        },
    ));

    let row = Object::new("Row")
        .field(Field::new("a", TypeRef::named_nn(TypeRef::INT), |ctx| {
            FieldFuture::new(async move {
                let data = ctx.parent_value.try_downcast_ref::<RowData>()?;
                Ok(Some(FieldValue::value(data.a)))
            })
        }))
        .field(Field::new(
            "cells",
            TypeRef::named_nn_list_nn("Cell"),
            |ctx| {
                FieldFuture::new(async move {
                    let data = ctx.parent_value.try_downcast_ref::<RowData>()?;
                    let cells: Vec<FieldValue> = data
                        .cells
                        .iter()
                        .map(|&j| FieldValue::owned_any(j))
                        .collect();
                    Ok(Some(FieldValue::list(cells)))
                })
            },
        ));

    let query = Object::new("Query").field(Field::new(
        "rows",
        TypeRef::named_nn_list_nn("Row"),
        |_ctx| {
            FieldFuture::new(async move {
                let rows: Vec<FieldValue> = (0..100_000i64)
                    .map(|i| {
                        FieldValue::owned_any(RowData {
                            a: i,
                            cells: (0..5).collect(),
                        })
                    })
                    .collect();
                Ok(Some(FieldValue::list(rows)))
            })
        },
    ));

    let schema = Schema::build("Query", None, None)
        .register(cell)
        .register(row)
        .register(query)
        .finish()
        .unwrap();

    let start = Instant::now();
    let result = schema.execute("{ rows { a cells { value } } }").await;
    let elapsed = start.elapsed();

    assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);

    let data = result.data.into_json().unwrap();
    let rows = data["rows"].as_array().unwrap();
    let size = rows.len();
    println!(
        "async-graphql: Fetched {} cells ({}x5) in {:.4}s",
        size * 5,
        size,
        elapsed.as_secs_f64()
    );
}
