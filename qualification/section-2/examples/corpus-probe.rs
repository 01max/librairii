//! Observations from explicitly exported synthetic fixtures; no product classifier/recovery.
use rusqlite::{Connection, OpenFlags, backup::Backup, types::ValueRef};
use serde_json::{Map, Value, json};
use std::io::{Cursor, Read};
use std::path::Path;
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const CEILING: u64 = 1 << 20;
const FIXED_TIME: &str = "2000-01-01 00:00:00";

fn read_json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn rows(conn: &Connection, sql: &str) -> Result<Value> {
    let mut statement = conn.prepare(sql)?;
    let names: Vec<String> = statement
        .column_names()
        .iter()
        .map(|n| n.to_string())
        .collect();
    let mut query = statement.query([])?;
    let mut output = Vec::new();
    while let Some(row) = query.next()? {
        let mut object = Map::new();
        for (index, name) in names.iter().enumerate() {
            // Match the frozen producer's JSON representation, retaining NULL and row order.
            let value = match row.get_ref(index)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(n) => json!(n),
                ValueRef::Real(n) => json!(n),
                ValueRef::Text(b) | ValueRef::Blob(b) => json!(std::str::from_utf8(b)?),
            };
            object.insert(name.clone(), value);
        }
        output.push(Value::Object(object));
    }
    Ok(json!(output))
}

fn table_names(conn: &Connection) -> Result<Vec<String>> {
    Ok(rows(conn, "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?
        .as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap().to_owned()).collect())
}

fn snapshot(conn: &Connection) -> Result<Value> {
    let mut tables = Map::new();
    for name in table_names(conn)? {
        tables.insert(
            name.clone(),
            rows(
                conn,
                &format!("SELECT * FROM {} ORDER BY rowid", quoted(&name)),
            )?,
        );
    }
    Ok(json!({
        "tables": tables,
        "schema": rows(conn, "SELECT type,name,tbl_name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name")?,
        "integrity": conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))?,
        "foreignKeyViolations": rows(conn, "PRAGMA foreign_key_check")?,
    }))
}

fn fixture_times(conn: &Connection) -> Result<()> {
    // Exact producer fixTimes columns; no identity, query, revision or ordering normalization.
    for table in table_names(conn)? {
        for column in rows(conn, &format!("PRAGMA table_info({})", quoted(&table)))?
            .as_array()
            .unwrap()
        {
            let name = column["name"].as_str().unwrap();
            if matches!(
                name,
                "created_at"
                    | "updated_at"
                    | "applied_at"
                    | "added_at"
                    | "accepted_at"
                    | "started_at"
                    | "finished_at"
            ) {
                conn.execute(
                    &format!(
                        "UPDATE {} SET {}=?1 WHERE {} IS NOT NULL AND {}!=?1",
                        quoted(&table),
                        quoted(name),
                        quoted(name),
                        quoted(name)
                    ),
                    [FIXED_TIME],
                )?;
            }
        }
    }
    Ok(())
}

fn database(root: &Path, file: &str, prefix: Option<usize>, migrations: &[Value]) -> Result<Value> {
    let source = Connection::open_with_flags(root.join(file), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let before = snapshot(&source)?;
    let temp = tempfile::tempdir()?;
    let mut copy = Connection::open(temp.path().join("copy.sqlite3"))?;
    Backup::new(&source, &mut copy)?.run_to_completion(32, Duration::from_millis(1), None)?;
    let backup = snapshot(&copy)?;
    copy.execute_batch("PRAGMA foreign_keys=ON;")?;
    if let Some(prefix) = prefix {
        for (index, migration) in migrations.iter().enumerate().skip(prefix) {
            let tx = copy.transaction()?;
            tx.execute_batch(&std::fs::read_to_string(
                root.join(migration["path"].as_str().unwrap()),
            )?)?;
            tx.execute(
                "INSERT INTO schema_migrations(version,applied_at) VALUES(?1,?2)",
                rusqlite::params![i64::try_from(index + 1)?, FIXED_TIME],
            )?;
            tx.commit()?;
        }
        fixture_times(&copy)?;
    }
    Ok(
        json!({"file":file, "prefix":prefix, "before":before, "backup":backup, "afterSqlOnly":snapshot(&copy)?}),
    )
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

fn payload(input: &mut dyn Read, remaining: &mut u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    input.take(*remaining + 1).read_to_end(&mut bytes)?;
    let size = bytes.len() as u64;
    if size > *remaining {
        return Err(std::io::Error::other(
            "qualification-only 1MiB delivered-byte ceiling",
        ));
    }
    *remaining -= size;
    Ok(bytes)
}

fn container(bytes: &[u8]) -> Result<Value> {
    let mut remaining = CEILING;
    let mut entries = Vec::new();
    if bytes.starts_with(b"7z\xbc\xaf\x27\x1c") {
        let mut reader =
            sevenz_rust2::ArchiveReader::new(Cursor::new(bytes), sevenz_rust2::Password::empty())?;
        reader.set_thread_count(1);
        let archive = reader.archive();
        let solid = archive.is_solid;
        let blocks: Vec<_> = archive.blocks.iter().map(|b| json!({
            "unpackSize":b.get_unpack_size(), "hasCrc":b.has_crc, "crc":b.crc,
            "coders":b.ordered_coder_iter().map(|(_, c)| json!({"idHex":hex(c.encoder_method_id()), "propertiesHex":hex(c.properties())})).collect::<Vec<_>>()
        })).collect();
        reader.for_each_entries(|entry, input| {
            let bytes = payload(input, &mut remaining)?;
            if bytes.len() as u64 != entry.size { return Err(std::io::Error::other("decoded length mismatch").into()); }
            entries.push(json!({"name":entry.name, "size":entry.size, "hasCrc":entry.has_crc,
                "crc":entry.crc, "directory":entry.is_directory, "antiItem":entry.is_anti_item,
                "windowsAttributes":entry.windows_attributes, "hasWindowsAttributes":entry.has_windows_attributes,
                "payloadHex":hex(&bytes)}));
            Ok(true)
        })?;
        Ok(
            json!({"container":"7z", "solid":solid, "blocks":blocks, "entries":entries, "decodedBytes":CEILING-remaining}),
        )
    } else {
        // Dispatch on bytes, deliberately allowing unsupported suffixes to expose policy gaps.
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let mut metadata = json!({"name":entry.name()?, "rawNameHex":hex(entry.name_raw()),
                "size":entry.size(), "compressedSize":entry.compressed_size(), "crc":entry.crc32(),
                "method":format!("{:?}",entry.compression()), "directory":entry.is_dir(),
                "enclosedNameAvailable":entry.enclosed_name().is_some(), "unixMode":entry.unix_mode()});
            let bytes = payload(&mut entry, &mut remaining)?; // Full EOF read includes candidate CRC validation.
            if bytes.len() as u64 != entry.size() {
                return Err("decoded length mismatch".into());
            }
            metadata["payloadHex"] = json!(hex(&bytes));
            entries.push(metadata);
        }
        Ok(json!({"container":"zip", "entries":entries, "decodedBytes":CEILING-remaining}))
    }
}

fn observe(root: &Path, file: &str) -> Result<Value> {
    let observation = match container(&std::fs::read(root.join(file))?) {
        Ok(value) => value,
        Err(error) => json!({"containerError":error.to_string()}),
    };
    Ok(json!({"file":file,"observation":observation}))
}

fn main() -> Result<()> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("expected explicitly exported synthetic corpus root")?;
    let root = Path::new(&root);
    let provenance = read_json(&root.join("databases/provenance.json"))?;
    assert_eq!(provenance["synthetic"], true);
    assert_eq!(provenance["productionData"], false);
    let migrations = provenance["migrations"].as_array().unwrap();
    assert_eq!(migrations.len(), 18);
    let mut databases = Vec::new();
    for item in provenance["prefixes"].as_array().unwrap() {
        databases.push(database(
            root,
            item["file"].as_str().unwrap(),
            Some(item["prefix"].as_u64().unwrap() as usize),
            migrations,
        )?);
    }
    databases.push(database(
        root,
        "libraries/schema018/db/librairii.sqlite3",
        None,
        migrations,
    )?);
    let mut archives = Vec::new();
    for expected in read_json(&root.join("inspection.json"))?
        .as_array()
        .unwrap()
    {
        let file = expected["file"].as_str().unwrap();
        archives.push(observe(root, file)?);
    }
    let mut codecs = Vec::new();
    let codec_manifest = root.join("independent-codecs.json");
    if codec_manifest.exists() {
        for fixture in read_json(&codec_manifest)?.as_array().unwrap() {
            if fixture.get("producerError").is_none() {
                codecs.push(observe(root, fixture["file"].as_str().unwrap())?);
            }
        }
    }
    println!(
        "{}",
        json!({"sqliteVersion":rusqlite::version(),"databases":databases,"archives":archives,"independentCodecs":codecs})
    );
    Ok(())
}
