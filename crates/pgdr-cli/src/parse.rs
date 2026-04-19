use std::collections::BTreeSet;

use serde_json::Value;

pub(crate) fn collect_from_parse_result(
    result: &pg_query::ParseResult,
    tables: &mut BTreeSet<String>,
    functions: &mut BTreeSet<String>,
) {
    for (name, _ctx) in &result.tables {
        let base = name.rsplit_once('.').map_or(name.as_str(), |(_, n)| n);
        tables.insert(base.to_owned());
    }
    for (name, _ctx) in &result.functions {
        let base = name.rsplit_once('.').map_or(name.as_str(), |(_, n)| n);
        functions.insert(base.to_owned());
    }
}

/// Recursively collects all `query` strings from PL/pgSQL expression nodes.
pub(crate) fn collect_plpgsql_queries(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(q)) = map.get("query") {
                out.push(q.clone());
            }
            for v in map.values() {
                collect_plpgsql_queries(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_plpgsql_queries(v, out);
            }
        }
        _ => {}
    }
}
