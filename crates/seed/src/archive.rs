//! Archive of the fetched data as JSON lines, so a fresh database can be filled
//! without fetching every provider, feed and repository again.
//!
//! Each archived table becomes `<table>.jsonl`: one row per line, ordered by its
//! natural key. Row ids are dropped and references to other rows are replaced by
//! the natural keys of those rows (organization slug, product slug, source slug,
//! news URL), because ids differ between databases. Postgres writes each line as
//! jsonb text, so numbers keep their exact precision.
//!
//! The import upserts on the natural key, so it can run any number of times. A
//! row whose reference does not resolve, such as a news tag of an organization
//! that is not in the database, is skipped. Curated data (organizations, products,
//! offerings, feeds) comes from the YAML in `data/` and is not archived, and the
//! market summaries are rebuilt from the listings instead of being stored.

use std::path::Path;

use ih_db::{
    PgPool,
    sqlx::{self, AssertSqlSafe},
};

/// Rows that contain something shaped like an access token are left out of the
/// archive. Providers list user-named models, and a name can be a pasted token
/// (a Hugging Face repository called `hf_...` was one), which must not be
/// republished in a repository. Postgres regular expression.
const TOKEN_SHAPED: &str = r"hf_[A-Za-z0-9]{30,}|gh[pousr]_[A-Za-z0-9]{30,}|sk-[A-Za-z0-9_-]{32,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,}";

/// A table of fetched data and how its references map to natural keys.
struct Table {
    name: &'static str,
    /// The unique key the import upserts on, as table columns.
    key: &'static [&'static str],
    /// Order of the exported lines.
    order: &'static str,
    /// Columns not archived, besides the referencing ones in `refs`.
    drop: &'static [&'static str],
    refs: &'static [Ref],
}

/// A reference column archived as the natural key of the row it points to.
struct Ref {
    /// The column, such as `organization_id`.
    column: &'static str,
    /// The field in the archive, such as `organization`.
    field: &'static str,
    /// The referenced table and its natural key column.
    table: &'static str,
    natural_key: &'static str,
    /// Whether a row needs the reference to be imported.
    required: bool,
}

const fn r(
    column: &'static str,
    field: &'static str,
    table: &'static str,
    natural_key: &'static str,
    required: bool,
) -> Ref {
    Ref {
        column,
        field,
        table,
        natural_key,
        required,
    }
}

/// In import order: a table comes after the tables its references point to.
const TABLES: &[Table] = &[
    Table {
        name: "market_listings",
        key: &["adapter", "listing_key"],
        order: "adapter, listing_key",
        drop: &["id"],
        refs: &[],
    },
    Table {
        name: "price_holds",
        key: &["adapter", "listing_key"],
        order: "adapter, listing_key",
        drop: &["id"],
        refs: &[],
    },
    Table {
        name: "adapter_state",
        key: &["adapter"],
        order: "adapter",
        drop: &[],
        refs: &[r(
            "organization_id",
            "organization",
            "organizations",
            "slug",
            false,
        )],
    },
    Table {
        name: "news_items",
        key: &["url"],
        order: "url",
        drop: &["id", "embedding"],
        refs: &[r("source_id", "source", "sources", "slug", false)],
    },
    Table {
        name: "news_item_organizations",
        key: &["news_item_id", "organization_id"],
        order: "news_url, organization",
        drop: &[],
        refs: &[
            r("news_item_id", "news_url", "news_items", "url", true),
            r(
                "organization_id",
                "organization",
                "organizations",
                "slug",
                true,
            ),
        ],
    },
    Table {
        name: "news_enrichment",
        key: &["news_item_id"],
        order: "news_url",
        drop: &[],
        refs: &[r("news_item_id", "news_url", "news_items", "url", true)],
    },
    Table {
        name: "repo_stats",
        key: &["product_id"],
        order: "product",
        drop: &[],
        refs: &[r("product_id", "product", "products", "slug", true)],
    },
    Table {
        name: "repo_star_history",
        key: &["product_id", "day"],
        order: "product, day",
        drop: &[],
        refs: &[r("product_id", "product", "products", "slug", true)],
    },
];

/// Rows written or read for one table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCount {
    pub table: &'static str,
    pub rows: u64,
}

impl Table {
    fn file(&self, dir: &Path) -> std::path::PathBuf {
        dir.join(format!("{}.jsonl", self.name))
    }

    /// Each row as jsonb text, references replaced by natural keys.
    fn export_sql(&self) -> String {
        let mut removed: Vec<&str> = self.drop.to_vec();
        removed.extend(self.refs.iter().map(|r| r.column));
        let minus: String = removed.iter().map(|c| format!(" - '{c}'")).collect();
        let joins: String = self
            .refs
            .iter()
            .enumerate()
            .map(|(i, r)| format!(" LEFT JOIN {} j{i} ON j{i}.id = t.{}", r.table, r.column))
            .collect();
        let fields: Vec<String> = self
            .refs
            .iter()
            .enumerate()
            .map(|(i, r)| format!("'{}', j{i}.{}", r.field, r.natural_key))
            .collect();
        let extra = if fields.is_empty() {
            String::new()
        } else {
            format!(" || jsonb_build_object({})", fields.join(", "))
        };
        format!(
            "SELECT line::text FROM (
                 SELECT (to_jsonb(t){minus}){extra} AS line FROM {} t{joins}
             ) x WHERE line::text !~ '{TOKEN_SHAPED}' ORDER BY {}",
            self.name,
            self.order
                .split(", ")
                .map(|f| format!("line->>'{f}'"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    /// Upserts the rows of `$1`, a jsonb array of archived lines.
    fn import_sql(&self, columns: &[(String, String)]) -> String {
        let mut select = Vec::new();
        let mut insert = Vec::new();
        for (column, data_type) in columns {
            if self.drop.contains(&column.as_str()) {
                continue;
            }
            insert.push(column.clone());
            match self.refs.iter().find(|r| r.column == column) {
                Some(r) => select.push(format!(
                    "(SELECT id FROM {} WHERE {} = e->>'{}')",
                    r.table, r.natural_key, r.field
                )),
                // JSON null in a jsonb column stays JSON null, as it was stored.
                None if data_type == "jsonb" => select.push(format!("e->'{column}'")),
                None => select.push(format!("(rec).{column}")),
            }
        }
        let updates: Vec<String> = insert
            .iter()
            .filter(|c| !self.key.contains(&c.as_str()))
            .map(|c| format!("{c} = EXCLUDED.{c}"))
            .collect();
        let on_conflict = if updates.is_empty() {
            "DO NOTHING".to_owned()
        } else {
            format!("DO UPDATE SET {}", updates.join(", "))
        };
        let required: String = self
            .refs
            .iter()
            .filter(|r| r.required)
            .map(|r| {
                format!(
                    " AND EXISTS (SELECT 1 FROM {} WHERE {} = e->>'{}')",
                    r.table, r.natural_key, r.field
                )
            })
            .collect();
        format!(
            "INSERT INTO {name} ({insert})
             SELECT {select} FROM (
                 SELECT e, jsonb_populate_record(NULL::{name}, e) AS rec
                 FROM jsonb_array_elements($1::jsonb) e
             ) x WHERE true{required}
             ON CONFLICT ({key}) {on_conflict}",
            name = self.name,
            insert = insert.join(", "),
            select = select.join(", "),
            key = self.key.join(", "),
        )
    }
}

/// Writes every archived table to `dir/<table>.jsonl`.
pub async fn export(pool: &PgPool, dir: &Path) -> anyhow::Result<Vec<TableCount>> {
    std::fs::create_dir_all(dir)?;
    let mut counts = Vec::new();
    for table in TABLES {
        let lines: Vec<String> = sqlx::query_scalar(AssertSqlSafe(table.export_sql()))
            .fetch_all(pool)
            .await?;
        let mut text = lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        let path = table.file(dir);
        let tmp = path.with_extension("jsonl.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &path)?;
        counts.push(TableCount {
            table: table.name,
            rows: lines.len() as u64,
        });
    }
    Ok(counts)
}

/// Rows sent to Postgres in one statement.
const BATCH: usize = 2_000;

/// Upserts every `dir/<table>.jsonl` that exists, in one transaction. Returns
/// the lines read per table; rows with an unresolved required reference are skipped.
pub async fn import(pool: &PgPool, dir: &Path) -> anyhow::Result<Vec<TableCount>> {
    let mut tx = pool.begin().await?;
    let mut counts = Vec::new();
    for table in TABLES {
        let path = table.file(dir);
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        for (n, line) in lines.iter().enumerate() {
            serde_json::from_str::<serde::de::IgnoredAny>(line)
                .map_err(|e| anyhow::anyhow!("{}:{}: not JSON: {e}", path.display(), n + 1))?;
        }
        let columns: Vec<(String, String)> = sqlx::query_as(
            "SELECT column_name::text, data_type::text FROM information_schema.columns
              WHERE table_schema = current_schema() AND table_name = $1
                AND is_generated = 'NEVER'
              ORDER BY ordinal_position",
        )
        .bind(table.name)
        .fetch_all(&mut *tx)
        .await?;
        let sql = table.import_sql(&columns);
        for chunk in lines.chunks(BATCH) {
            let array = format!("[{}]", chunk.join(","));
            sqlx::query(AssertSqlSafe(sql.clone()))
                .bind(array)
                .execute(&mut *tx)
                .await?;
        }
        counts.push(TableCount {
            table: table.name,
            rows: lines.len() as u64,
        });
    }
    tx.commit().await?;
    Ok(counts)
}
