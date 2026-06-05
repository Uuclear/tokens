use tokens::adapters::Adapter;

#[test]
#[ignore = "requires local Cursor install"]
fn cursor_adapter_scan_count() {
    let path = std::path::PathBuf::from(std::env::var("APPDATA").unwrap())
        .join("Cursor/User/globalStorage/state.vscdb");
    let conn = tokens::util::sqlite_ext::open_foreign_db(&path).unwrap();
    let mut ok = 0i64;
    let mut err = 0i64;
    let mut stmt = conn
        .prepare(
            "SELECT key, value FROM cursorDiskKV WHERE key LIKE 'bubbleId:%' LIMIT 500",
        )
        .unwrap();
    let rows = stmt.query_map([], |row| {
        let key: String = row.get(0)?;
        let as_bytes: Result<Vec<u8>, _> = row.get(1);
        let as_text: Result<String, _> = row.get(1);
        Ok((key, as_bytes.is_ok(), as_text.is_ok()))
    });
    let mut bytes_ok = 0i64;
    let mut text_ok = 0i64;
    for r in rows.unwrap() {
        match r {
            Ok((_, b, t)) => {
                ok += 1;
                if b {
                    bytes_ok += 1;
                }
                if t {
                    text_ok += 1;
                }
            }
            Err(_) => err += 1,
        }
    }
    eprintln!("bubble sample ok={ok} err={err} bytes_ok={bytes_ok} text_ok={text_ok}");

    let adapter = tokens::adapters::adapter_by_id("cursor").unwrap();
    let filter = tokens::scan_filter::ScanFilter::parse_all();
    let events = adapter
        .scan(chrono::Utc::now().timestamp_millis(), &filter)
        .unwrap();
    eprintln!("cursor scan events: {}", events.len());
    if let Some(e) = events.first() {
        eprintln!(
            "first: in={} out={} quality={:?}",
            e.input_tokens, e.output_tokens, e.quality
        );
    }
}

#[test]
#[ignore = "requires local Cursor install"]
fn cursor_vscdb_tables() {
    let path = std::path::PathBuf::from(std::env::var("APPDATA").unwrap())
        .join("Cursor")
        .join("User")
        .join("globalStorage")
        .join("state.vscdb");
    if !path.exists() {
        return;
    }
    let conn = tokens::util::sqlite_ext::open_foreign_db(&path).unwrap();
    let tables = tokens::util::sqlite_ext::list_tables(&conn).unwrap();
    eprintln!("tables: {:?}", tables);
    for t in &tables {
        let cols = tokens::util::sqlite_ext::table_columns(&conn, t).unwrap();
        eprintln!("  {t}: {cols:?}");
        if t == "cursorDiskKV" || t == "ItemTable" {
            let sql = format!("SELECT COUNT(*) FROM \"{t}\"");
            let n: i64 = conn.query_row(&sql, [], |r| r.get(0)).unwrap();
            eprintln!("    rows: {n}");
            let sample = format!(
                "SELECT key FROM \"{t}\" WHERE key LIKE 'bubbleId:%' OR key LIKE 'composerData:%' LIMIT 3"
            );
            if let Ok(mut stmt) = conn.prepare(&sample) {
                let keys: Vec<String> = stmt
                    .query_map([], |r| r.get(0))
                    .unwrap()
                    .filter_map(|r| r.ok())
                    .collect();
                eprintln!("    sample keys: {keys:?}");
            }
            let bubble_n: i64 = conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM \"{t}\" WHERE key LIKE 'bubbleId:%'"),
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            eprintln!("    bubbleId rows: {bubble_n}");
            if t == "cursorDiskKV" {
                if let Ok((key, text)) = conn.query_row(
                    "SELECT key, value FROM cursorDiskKV WHERE key LIKE 'bubbleId:%' AND value LIKE '%inputTokens%' LIMIT 1",
                    [],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                ) {
                    let preview: String = text.chars().take(500).collect();
                    eprintln!("    sample bubble key={key} preview={preview}");
                }
                if let Ok((key, text)) = conn.query_row(
                    "SELECT key, value FROM cursorDiskKV WHERE key LIKE 'composerData:%' LIMIT 1",
                    [],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                ) {
                    let preview: String = text.chars().take(500).collect();
                    eprintln!("    sample composer key={key} preview={preview}");
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        eprintln!(
                            "    composer model={:?}",
                            tokens::util::json_extract::extract_model(&v)
                        );
                    }
                }
                let model_keys: Vec<String> = conn
                    .prepare(
                        "SELECT DISTINCT key FROM cursorDiskKV WHERE key LIKE '%model%' OR key LIKE '%Model%' LIMIT 15",
                    )
                    .unwrap()
                    .query_map([], |r| r.get(0))
                    .unwrap()
                    .filter_map(|r| r.ok())
                    .collect();
                eprintln!("    keys with model: {model_keys:?}");
            }
            if t == "ItemTable" {
                let item_keys: Vec<String> = conn
                    .prepare(
                        "SELECT key FROM ItemTable WHERE key LIKE '%model%' OR key LIKE '%composer%' OR key LIKE 'cursor%' LIMIT 20",
                    )
                    .unwrap()
                    .query_map([], |r| r.get(0))
                    .unwrap()
                    .filter_map(|r| r.ok())
                    .collect();
                eprintln!("    ItemTable keys: {item_keys:?}");
            }
        }
    }
}
