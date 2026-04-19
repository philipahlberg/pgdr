use postgres_types::Type;
use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use tokio_postgres::Row;

pub fn rows_to_json(rows: &[Row]) -> Vec<Value> {
    rows.iter().map(row_to_value).collect()
}

fn row_to_value(row: &Row) -> Value {
    let mut map = Map::new();
    for (i, col) in row.columns().iter().enumerate() {
        let value = col_to_value(row, i, col.type_());
        map.insert(col.name().to_owned(), value);
    }
    Value::Object(map)
}

fn col_to_value(row: &Row, i: usize, ty: &Type) -> Value {
    match ty {
        &Type::BOOL => get_val::<bool>(row, i).map(Value::Bool),
        &Type::INT2 => get_val::<i16>(row, i).map(Value::from),
        &Type::INT4 => get_val::<i32>(row, i).map(Value::from),
        &Type::INT8 => get_val::<i64>(row, i).map(Value::from),
        &Type::FLOAT4 => get_val::<f32>(row, i).map(|v| {
            serde_json::Number::from_f64(f64::from(v)).map_or(Value::Null, Value::Number)
        }),
        &Type::FLOAT8 => get_val::<f64>(row, i).map(|v| {
            serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
        }),
        &Type::TEXT | &Type::VARCHAR | &Type::BPCHAR | &Type::NAME => {
            get_val::<String>(row, i).map(Value::String)
        }
        &Type::UUID => get_val::<uuid::Uuid>(row, i).map(|v| Value::String(v.to_string())),
        &Type::JSON | &Type::JSONB => get_val::<Value>(row, i),
        &Type::TIMESTAMPTZ => get_val::<time::OffsetDateTime>(row, i).map(|v| {
            v.format(&Rfc3339)
                .map(Value::String)
                .unwrap_or(Value::Null)
        }),
        &Type::TIMESTAMP => get_val::<time::PrimitiveDateTime>(row, i).map(|v| {
            Value::String(v.to_string())
        }),
        &Type::DATE => get_val::<time::Date>(row, i).map(|v| Value::String(v.to_string())),
        &Type::TIME => get_val::<time::Time>(row, i).map(|v| Value::String(v.to_string())),
        _ => get_val::<String>(row, i).map(Value::String),
    }
    .unwrap_or(Value::Null)
}

fn get_val<'a, T>(row: &'a Row, i: usize) -> Option<T>
where
    T: tokio_postgres::types::FromSql<'a>,
{
    row.try_get::<_, Option<T>>(i).ok().flatten()
}

pub fn print_json(values: &[Value]) {
    println!("{}", serde_json::to_string_pretty(values).expect("serialization is infallible"));
}
