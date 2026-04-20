#![allow(unused_crate_dependencies)]

use pgdr::commands::connections;
use serde_json::Value;
use tests::VERSIONS;
use tests::try_connect;

#[tokio::test]
async fn lists_connections_on_every_version() {
    for (version, port) in VERSIONS {
        let Some(client) = try_connect(*port).await else {
            eprintln!("skipping pg{version}: not reachable on port {port}");
            continue;
        };

        let value = connections::run(&client, None, None, false)
            .await
            .unwrap_or_else(|e| panic!("pg{version}: {e}"));

        let Value::Array(rows) = value else {
            panic!("pg{version}: expected array, got {value}");
        };
        assert!(!rows.is_empty(), "pg{version}: expected at least one row");

        let row = rows[0].as_object().expect("row is object");
        for field in [
            "pid",
            "backend_type",
            "database",
            "user",
            "application_name",
            "state",
            "query",
        ] {
            assert!(
                row.contains_key(field),
                "pg{version}: missing field {field}"
            );
        }
    }
}

#[tokio::test]
async fn filters_by_state() {
    for (version, port) in VERSIONS {
        let Some(client) = try_connect(*port).await else {
            continue;
        };

        let value = connections::run(&client, Some("active"), None, false)
            .await
            .unwrap_or_else(|e| panic!("pg{version}: {e}"));

        let rows = value.as_array().expect("array");
        for row in rows {
            let state = row.get("state").and_then(Value::as_str);
            assert_eq!(
                state,
                Some("active"),
                "pg{version}: unexpected state {state:?}"
            );
        }
    }
}

#[tokio::test]
async fn exclude_internal_drops_non_client_backends() {
    for (version, port) in VERSIONS {
        let Some(client) = try_connect(*port).await else {
            continue;
        };

        let value = connections::run(&client, None, None, true)
            .await
            .unwrap_or_else(|e| panic!("pg{version}: {e}"));

        let rows = value.as_array().expect("array");
        for row in rows {
            let backend = row
                .get("backend_type")
                .and_then(Value::as_str)
                .unwrap_or("");
            assert!(
                backend == "client backend" || backend == "parallel worker",
                "pg{version}: unexpected backend_type {backend:?}"
            );
        }
    }
}
