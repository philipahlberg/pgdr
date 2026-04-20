#![allow(unused_crate_dependencies)]

use pgdr::commands::locks;
use serde_json::Value;
use tests::VERSIONS;
use tests::try_connect;

#[tokio::test]
async fn lists_locks_on_every_version() {
    for (version, port) in VERSIONS {
        let Some(client) = try_connect(*port).await else {
            eprintln!("skipping pg{version}: not reachable on port {port}");
            continue;
        };

        let value = locks::run(&client, false, false, None, None, false)
            .await
            .unwrap_or_else(|e| panic!("pg{version}: {e}"));

        let rows = value.as_array().expect("array");
        assert!(
            !rows.is_empty(),
            "pg{version}: expected at least one self-lock"
        );

        let row = rows[0].as_object().expect("row is object");
        for field in ["locktype", "mode", "granted", "pid", "blocked_by"] {
            assert!(
                row.contains_key(field),
                "pg{version}: missing field {field}"
            );
        }

        let blocked_by = &row["blocked_by"];
        assert!(
            blocked_by.is_array(),
            "pg{version}: blocked_by should be an array, got {blocked_by}"
        );
    }
}

#[tokio::test]
async fn granted_filter_returns_only_granted_locks() {
    for (version, port) in VERSIONS {
        let Some(client) = try_connect(*port).await else {
            continue;
        };

        let value = locks::run(&client, true, false, None, None, false)
            .await
            .unwrap_or_else(|e| panic!("pg{version}: {e}"));

        let rows = value.as_array().expect("array");
        for row in rows {
            let granted = row.get("granted").and_then(Value::as_bool);
            assert_eq!(granted, Some(true), "pg{version}: expected granted=true");
        }
    }
}

#[tokio::test]
async fn excluding_advisory_locks_drops_them() {
    for (version, port) in VERSIONS {
        let Some(client) = try_connect(*port).await else {
            continue;
        };

        client
            .execute("SELECT pg_advisory_lock(42)", &[])
            .await
            .unwrap();

        let with = locks::run(&client, false, false, None, None, false)
            .await
            .unwrap();
        let without = locks::run(&client, false, false, None, None, true)
            .await
            .unwrap();

        let count_advisory = |v: &Value| {
            v.as_array()
                .unwrap()
                .iter()
                .filter(|r| r.get("locktype").and_then(Value::as_str) == Some("advisory"))
                .count()
        };
        assert!(
            count_advisory(&with) >= 1,
            "pg{version}: expected an advisory lock with the flag off"
        );
        assert_eq!(
            count_advisory(&without),
            0,
            "pg{version}: expected no advisory locks with the flag on"
        );

        client
            .execute("SELECT pg_advisory_unlock(42)", &[])
            .await
            .unwrap();
    }
}
