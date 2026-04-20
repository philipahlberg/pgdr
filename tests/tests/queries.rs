#![allow(unused_crate_dependencies)]

use pgdr::commands::queries;
use pgdr::commands::queries::OrderBy;
use tests::BARE_PORT;
use tests::VERSIONS;
use tests::try_connect;

#[tokio::test]
async fn lists_queries_on_every_version() {
    for (version, port) in VERSIONS {
        let Some(client) = try_connect(*port).await else {
            eprintln!("skipping pg{version}: not reachable on port {port}");
            continue;
        };

        client
            .execute("SELECT pg_stat_statements_reset()", &[])
            .await
            .unwrap();
        for _ in 0..3 {
            client.execute("SELECT 1", &[]).await.unwrap();
        }

        let value = queries::run(&client, OrderBy::TotalTime, 50, None, None)
            .await
            .unwrap_or_else(|e| panic!("pg{version}: {e}"));

        let rows = value.as_array().expect("array");
        assert!(!rows.is_empty(), "pg{version}: expected at least one row");

        let row = rows[0].as_object().expect("row is object");
        for field in [
            "queryid",
            "database",
            "role",
            "query",
            "calls",
            "rows",
            "plan",
            "exec",
            "blocks",
            "io_time_ms",
            "wal",
            "jit",
        ] {
            assert!(
                row.contains_key(field),
                "pg{version}: missing field {field}"
            );
        }

        let io = row["io_time_ms"].as_object().expect("io_time_ms is object");
        for field in [
            "blk_read",
            "blk_write",
            "shared_read",
            "shared_write",
            "local_read",
            "local_write",
            "temp_read",
            "temp_write",
        ] {
            assert!(
                io.contains_key(field),
                "pg{version}: io_time_ms.{field} missing"
            );
        }
    }
}

#[tokio::test]
async fn version_specific_io_fields_are_populated() {
    for (version, port) in VERSIONS {
        let Some(client) = try_connect(*port).await else {
            continue;
        };

        let version_num: i32 = client
            .query_one("SELECT current_setting('server_version_num')::int4", &[])
            .await
            .unwrap()
            .get(0);

        client
            .execute("SELECT pg_stat_statements_reset()", &[])
            .await
            .unwrap();
        client.execute("SELECT 1", &[]).await.unwrap();

        let value = queries::run(&client, OrderBy::TotalTime, 50, None, None)
            .await
            .unwrap_or_else(|e| panic!("pg{version}: {e}"));
        let row = &value.as_array().unwrap()[0];
        let io = row["io_time_ms"].as_object().unwrap();

        if version_num >= 170000 {
            assert!(
                io["blk_read"].is_null(),
                "pg{version}: blk_read should be null on >=17"
            );
            assert!(
                io["blk_write"].is_null(),
                "pg{version}: blk_write should be null on >=17"
            );
            assert!(
                !io["shared_read"].is_null(),
                "pg{version}: shared_read should be populated on >=17"
            );
        } else {
            assert!(
                io["shared_read"].is_null(),
                "pg{version}: shared_read should be null on <17"
            );
            assert!(
                io["local_read"].is_null(),
                "pg{version}: local_read should be null on <17"
            );
        }
    }
}

#[tokio::test]
async fn errors_when_extension_missing() {
    let Some(client) = try_connect(BARE_PORT).await else {
        eprintln!("skipping bare pg: not reachable on port {BARE_PORT}");
        return;
    };

    let err = queries::run(&client, OrderBy::TotalTime, 10, None, None)
        .await
        .expect_err("expected Error::Message");
    let msg = err.to_string();
    assert!(
        msg.contains("pg_stat_statements"),
        "unexpected error message: {msg}"
    );
}

#[tokio::test]
async fn limit_and_order_by_are_respected() {
    for (version, port) in VERSIONS {
        let Some(client) = try_connect(*port).await else {
            continue;
        };

        client
            .execute("SELECT pg_stat_statements_reset()", &[])
            .await
            .unwrap();
        for i in 0..5 {
            let sql = format!("SELECT {i}::int AS n");
            client.execute(&sql, &[]).await.unwrap();
        }

        let value = queries::run(&client, OrderBy::Calls, 2, None, None)
            .await
            .unwrap_or_else(|e| panic!("pg{version}: {e}"));
        let rows = value.as_array().unwrap();
        assert!(
            rows.len() <= 2,
            "pg{version}: expected at most 2 rows, got {}",
            rows.len()
        );

        let calls: Vec<i64> = rows
            .iter()
            .map(|r| r["calls"].as_i64().unwrap_or_default())
            .collect();
        let sorted = {
            let mut c = calls.clone();
            c.sort_by(|a, b| b.cmp(a));
            c
        };
        assert_eq!(calls, sorted, "pg{version}: rows not sorted by calls desc");
    }
}
