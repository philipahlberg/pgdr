use crate::error::Result;
use crate::output;
use crate::parse;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeSet;
use std::collections::HashMap;
use tokio_postgres::Client;

const EXCLUDED: &str = "'pg_catalog', 'information_schema', 'pg_toast'";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Node {
    kind: String,
    schema: String,
    name: String,
    oid: i64,
}

impl Node {
    fn to_json(&self) -> Value {
        json!({
            "kind":   self.kind,
            "schema": self.schema,
            "name":   self.name,
            "oid":    self.oid,
        })
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Edge {
    dependent: Node,
    dependency: Node,
}

impl Edge {
    fn to_json(&self) -> Value {
        json!({
            "dependent":  self.dependent.to_json(),
            "dependency": self.dependency.to_json(),
        })
    }
}

struct Pattern {
    schema: Option<String>,
    name: String,
}

impl Pattern {
    fn parse(s: &str) -> Self {
        match s.split_once('.') {
            Some((schema, name)) => Self {
                schema: Some(schema.to_owned()),
                name: name.to_owned(),
            },
            None => Self {
                schema: None,
                name: s.to_owned(),
            },
        }
    }

    fn matches(&self, node: &Node) -> bool {
        if let Some(schema_glob) = &self.schema
            && !glob_match(schema_glob, &node.schema)
        {
            return false;
        }
        glob_match(&self.name, &node.name)
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut match_i) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            match_i = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            match_i += 1;
            ti = match_i;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

// Structural edges derived entirely from catalog metadata:
//   - views / materialized views  → their referenced tables, views, functions  (pg_depend)
//   - SQL-language functions       → their referenced tables, views, functions  (pg_depend)
//   - tables                       → tables they reference via foreign keys      (pg_constraint)
//   - triggers                     → the table they fire on + the function they call (pg_trigger)
const STRUCTURAL_QUERY: &str = r#"
SELECT DISTINCT
  CASE obj_cls.relkind WHEN 'v' THEN 'view' WHEN 'm' THEN 'materialized_view' END AS dependent_kind,
  obj_ns.nspname   AS dependent_schema,
  obj_cls.relname  AS dependent_name,
  obj_cls.oid::bigint AS dependent_oid,
  CASE dep.refclassid
    WHEN 'pg_class'::regclass THEN CASE ref_cls.relkind
      WHEN 'r' THEN 'table'  WHEN 'v' THEN 'view'  WHEN 'm' THEN 'materialized_view'
      WHEN 'S' THEN 'sequence'  WHEN 'f' THEN 'foreign_table'
      ELSE ref_cls.relkind::text END
    WHEN 'pg_proc'::regclass THEN 'function'
  END AS dependency_kind,
  ref_ns.nspname   AS dependency_schema,
  COALESCE(ref_cls.relname, ref_proc.proname) AS dependency_name,
  COALESCE(ref_cls.oid, ref_proc.oid)::bigint AS dependency_oid
FROM pg_depend dep
JOIN pg_class obj_cls ON obj_cls.oid = dep.objid
  AND dep.classid = 'pg_class'::regclass AND obj_cls.relkind IN ('v', 'm')
JOIN pg_namespace obj_ns ON obj_ns.oid = obj_cls.relnamespace
LEFT JOIN pg_class ref_cls ON ref_cls.oid = dep.refobjid
  AND dep.refclassid = 'pg_class'::regclass AND ref_cls.relkind IN ('r', 'v', 'm', 'S', 'f')
LEFT JOIN pg_proc ref_proc ON ref_proc.oid = dep.refobjid
  AND dep.refclassid = 'pg_proc'::regclass
JOIN pg_namespace ref_ns ON ref_ns.oid = COALESCE(ref_cls.relnamespace, ref_proc.pronamespace)
WHERE dep.deptype = 'n'
  AND COALESCE(ref_cls.relname, ref_proc.proname) IS NOT NULL
  AND obj_ns.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
  AND ref_ns.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')

UNION

SELECT DISTINCT
  'function'       AS dependent_kind,
  fn_ns.nspname    AS dependent_schema,
  fn.proname       AS dependent_name,
  fn.oid::bigint   AS dependent_oid,
  CASE dep.refclassid
    WHEN 'pg_class'::regclass THEN CASE ref_cls.relkind
      WHEN 'r' THEN 'table'  WHEN 'v' THEN 'view'  WHEN 'm' THEN 'materialized_view'
      WHEN 'S' THEN 'sequence'  WHEN 'f' THEN 'foreign_table'
      ELSE ref_cls.relkind::text END
    WHEN 'pg_proc'::regclass THEN 'function'
  END AS dependency_kind,
  ref_ns.nspname   AS dependency_schema,
  COALESCE(ref_cls.relname, ref_proc.proname) AS dependency_name,
  COALESCE(ref_cls.oid, ref_proc.oid)::bigint AS dependency_oid
FROM pg_depend dep
JOIN pg_proc fn ON fn.oid = dep.objid AND dep.classid = 'pg_proc'::regclass
JOIN pg_namespace fn_ns ON fn_ns.oid = fn.pronamespace
JOIN pg_language lang ON lang.oid = fn.prolang AND lang.lanname = 'sql'
LEFT JOIN pg_class ref_cls ON ref_cls.oid = dep.refobjid
  AND dep.refclassid = 'pg_class'::regclass AND ref_cls.relkind IN ('r', 'v', 'm', 'S', 'f')
LEFT JOIN pg_proc ref_proc ON ref_proc.oid = dep.refobjid
  AND dep.refclassid = 'pg_proc'::regclass AND ref_proc.oid <> fn.oid
JOIN pg_namespace ref_ns ON ref_ns.oid = COALESCE(ref_cls.relnamespace, ref_proc.pronamespace)
WHERE dep.deptype = 'n'
  AND COALESCE(ref_cls.relname, ref_proc.proname) IS NOT NULL
  AND fn_ns.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
  AND ref_ns.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')

UNION

SELECT DISTINCT
  'table'              AS dependent_kind,
  from_ns.nspname      AS dependent_schema,
  from_cls.relname     AS dependent_name,
  from_cls.oid::bigint AS dependent_oid,
  'table'              AS dependency_kind,
  to_ns.nspname        AS dependency_schema,
  to_cls.relname       AS dependency_name,
  to_cls.oid::bigint   AS dependency_oid
FROM pg_constraint c
JOIN pg_class from_cls ON from_cls.oid = c.conrelid
JOIN pg_namespace from_ns ON from_ns.oid = from_cls.relnamespace
JOIN pg_class to_cls ON to_cls.oid = c.confrelid
JOIN pg_namespace to_ns ON to_ns.oid = to_cls.relnamespace
WHERE c.contype = 'f'
  AND from_ns.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
  AND to_ns.nspname   NOT IN ('pg_catalog', 'information_schema', 'pg_toast')

UNION

SELECT DISTINCT
  'trigger'          AS dependent_kind,
  tbl_ns.nspname     AS dependent_schema,
  t.tgname           AS dependent_name,
  t.oid::bigint      AS dependent_oid,
  CASE tbl.relkind
    WHEN 'r' THEN 'table' WHEN 'v' THEN 'view' WHEN 'f' THEN 'foreign_table'
    ELSE tbl.relkind::text END AS dependency_kind,
  tbl_ns.nspname     AS dependency_schema,
  tbl.relname        AS dependency_name,
  tbl.oid::bigint    AS dependency_oid
FROM pg_trigger t
JOIN pg_class tbl ON tbl.oid = t.tgrelid
JOIN pg_namespace tbl_ns ON tbl_ns.oid = tbl.relnamespace
WHERE NOT t.tgisinternal
  AND tbl_ns.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')

UNION

SELECT DISTINCT
  'trigger'          AS dependent_kind,
  tbl_ns.nspname     AS dependent_schema,
  t.tgname           AS dependent_name,
  t.oid::bigint      AS dependent_oid,
  'function'         AS dependency_kind,
  fn_ns.nspname      AS dependency_schema,
  fn.proname         AS dependency_name,
  fn.oid::bigint     AS dependency_oid
FROM pg_trigger t
JOIN pg_class tbl ON tbl.oid = t.tgrelid
JOIN pg_namespace tbl_ns ON tbl_ns.oid = tbl.relnamespace
JOIN pg_proc fn ON fn.oid = t.tgfoid
JOIN pg_namespace fn_ns ON fn_ns.oid = fn.pronamespace
WHERE NOT t.tgisinternal
  AND tbl_ns.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')

ORDER BY dependent_kind, dependent_schema, dependent_name, dependency_kind, dependency_schema, dependency_name
"#;

pub async fn run(client: &Client, patterns: &[String]) -> Result<()> {
    let patterns: Vec<Pattern> = patterns.iter().map(|s| Pattern::parse(s)).collect();
    let mut edges: BTreeSet<Edge> = BTreeSet::new();

    for row in client.query(STRUCTURAL_QUERY, &[]).await? {
        edges.insert(Edge {
            dependent: Node {
                kind: row.get("dependent_kind"),
                schema: row.get("dependent_schema"),
                name: row.get("dependent_name"),
                oid: row.get("dependent_oid"),
            },
            dependency: Node {
                kind: row.get("dependency_kind"),
                schema: row.get("dependency_schema"),
                name: row.get("dependency_name"),
                oid: row.get("dependency_oid"),
            },
        });
    }

    for edge in plpgsql_edges(client).await? {
        edges.insert(edge);
    }

    let values: Vec<Value> = edges
        .iter()
        .filter(|e| {
            patterns.is_empty()
                || patterns
                    .iter()
                    .any(|p| p.matches(&e.dependent) || p.matches(&e.dependency))
        })
        .map(Edge::to_json)
        .collect();
    output::print_json(&values);
    Ok(())
}

async fn plpgsql_edges(client: &Client) -> Result<Vec<Edge>> {
    // Fetch all non-SQL user-defined functions with their full definitions.
    // SQL functions are already covered by pg_depend in the structural query.
    let fn_rows = client
        .query(
            "SELECT p.proname AS name, n.nspname AS schema, p.oid::bigint AS oid, \
               pg_get_functiondef(p.oid) AS def \
             FROM pg_proc p \
             JOIN pg_namespace n ON n.oid = p.pronamespace \
             JOIN pg_language l ON l.oid = p.prolang \
             WHERE l.lanname NOT IN ('sql', 'c', 'internal') \
               AND n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')",
            &[],
        )
        .await?;

    if fn_rows.is_empty() {
        return Ok(vec![]);
    }

    // Parse every function body, accumulating referenced names per function
    // and a global set of all names we'll need to resolve via the DB.
    struct FnRefs {
        name: String,
        schema: String,
        oid: i64,
        tables: BTreeSet<String>,
        functions: BTreeSet<String>,
    }

    let mut all_table_refs: BTreeSet<String> = BTreeSet::new();
    let mut all_fn_refs: BTreeSet<String> = BTreeSet::new();
    let mut fn_refs: Vec<FnRefs> = Vec::new();

    for row in &fn_rows {
        let fname: String = row.get("name");
        let fschema: String = row.get("schema");
        let foid: i64 = row.get("oid");
        let def: &str = row.get("def");

        let mut tables = BTreeSet::new();
        let mut functions = BTreeSet::new();

        if let Ok(json) = pg_query::parse_plpgsql(def) {
            let mut queries = Vec::new();
            parse::collect_plpgsql_queries(&json, &mut queries);
            for q in &queries {
                if let Ok(result) = pg_query::parse(q) {
                    parse::collect_from_parse_result(&result, &mut tables, &mut functions);
                }
            }
        }

        all_table_refs.extend(tables.iter().cloned());
        all_fn_refs.extend(functions.iter().cloned());
        fn_refs.push(FnRefs {
            name: fname,
            schema: fschema,
            oid: foid,
            tables,
            functions,
        });
    }

    if all_table_refs.is_empty() && all_fn_refs.is_empty() {
        return Ok(vec![]);
    }

    // Resolve all referenced base names to concrete DB objects in one query.
    // A name may resolve to multiple objects in different schemas.
    let table_names: Vec<&str> = all_table_refs.iter().map(String::as_str).collect();
    let fn_names: Vec<&str> = all_fn_refs.iter().map(String::as_str).collect();

    let obj_rows = client
        .query(
            &format!(
                "SELECT kind, schema, name, oid FROM ( \
                   SELECT \
                     CASE c.relkind \
                       WHEN 'r' THEN 'table'  WHEN 'v' THEN 'view' \
                       WHEN 'm' THEN 'materialized_view'  WHEN 'S' THEN 'sequence' \
                       WHEN 'f' THEN 'foreign_table'  ELSE c.relkind::text END AS kind, \
                     n.nspname AS schema, c.relname AS name, c.oid::bigint AS oid \
                   FROM pg_class c \
                   JOIN pg_namespace n ON n.oid = c.relnamespace \
                   WHERE c.relname = ANY($1) AND c.relkind IN ('r', 'v', 'm', 'S', 'f') \
                     AND n.nspname NOT IN ({EXCLUDED}) \
                   UNION ALL \
                   SELECT 'function' AS kind, n.nspname AS schema, \
                     p.proname AS name, p.oid::bigint AS oid \
                   FROM pg_proc p \
                   JOIN pg_namespace n ON n.oid = p.pronamespace \
                   WHERE p.proname = ANY($2) \
                     AND n.nspname NOT IN ({EXCLUDED}) \
                 ) obj"
            ),
            &[&table_names, &fn_names],
        )
        .await?;

    // Build a name → [(kind, schema, oid)] lookup map.
    let mut lookup: HashMap<String, Vec<(String, String, i64)>> = HashMap::new();
    for row in &obj_rows {
        let kind: String = row.get("kind");
        let schema: String = row.get("schema");
        let name: String = row.get("name");
        let oid: i64 = row.get("oid");
        lookup.entry(name).or_default().push((kind, schema, oid));
    }

    // Emit one edge per (function → resolved object) pair.
    let mut edges: Vec<Edge> = Vec::new();
    for fr in &fn_refs {
        let dependent = Node {
            kind: "function".to_owned(),
            schema: fr.schema.clone(),
            name: fr.name.clone(),
            oid: fr.oid,
        };
        for tref in &fr.tables {
            if let Some(objects) = lookup.get(tref) {
                for (kind, schema, oid) in objects {
                    edges.push(Edge {
                        dependent: Node {
                            ..dependent.clone()
                        },
                        dependency: Node {
                            kind: kind.clone(),
                            schema: schema.clone(),
                            name: tref.clone(),
                            oid: *oid,
                        },
                    });
                }
            }
        }
        for fref in &fr.functions {
            if let Some(objects) = lookup.get(fref) {
                for (kind, ref_schema, oid) in objects {
                    if fref == &fr.name && ref_schema == &fr.schema {
                        continue; // skip self-reference
                    }
                    edges.push(Edge {
                        dependent: Node {
                            ..dependent.clone()
                        },
                        dependency: Node {
                            kind: kind.clone(),
                            schema: ref_schema.clone(),
                            name: fref.clone(),
                            oid: *oid,
                        },
                    });
                }
            }
        }
    }

    Ok(edges)
}
