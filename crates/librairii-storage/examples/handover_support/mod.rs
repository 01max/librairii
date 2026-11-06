use librairii_storage::{database::Database, platform::hash_stream};
use rusqlite::{Connection, types::ValueRef};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
pub type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
pub struct OwnedChild(pub Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub fn wait(child: &mut Child) -> Result<std::process::ExitStatus> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err("application probe deadline".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
}
pub fn probe(helper: &Path, mode: &str, profile: &Path, root: &Path) -> Result<Value> {
    let mut child = OwnedChild(
        Command::new(helper)
            .env_clear()
            .arg(mode)
            .arg(profile)
            .arg(root)
            .arg(profile.join("unused-receipt"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let status = wait(&mut child.0)?;
    let mut out = Vec::new();
    child
        .0
        .stdout
        .take()
        .ok_or("stdout")?
        .take(65537)
        .read_to_end(&mut out)?;
    let mut err = Vec::new();
    child
        .0
        .stderr
        .take()
        .ok_or("stderr")?
        .take(8193)
        .read_to_end(&mut err)?;
    if !status.success() || out.len() > 65536 {
        return Err(format!(
            "application {mode} failed: {status}: {}",
            String::from_utf8_lossy(&err)
        )
        .into());
    }
    Ok(serde_json::from_slice(&out)?)
}
pub fn files(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut out = BTreeMap::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(dir) = directories.pop() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let meta = fs::symlink_metadata(entry.path())?;
            if meta.is_dir() {
                directories.push(entry.path())
            } else {
                if !meta.is_file() || meta.file_type().is_symlink() {
                    return Err("nonregular fixture file".into());
                }
                out.insert(
                    entry.path().strip_prefix(root)?.to_path_buf(),
                    fs::read(entry.path())?,
                );
            }
        }
    }
    Ok(out)
}
pub fn seal(root: &Path) -> Result<Value> {
    let mut items = Vec::new();
    for (path, bytes) in files(root)? {
        items.push(json!({"path":path,"bytes":bytes.len(),"sha256":checksum(&root.join(path))?}));
    }
    Ok(Value::Array(items))
}
pub fn checksum(path: &Path) -> Result<String> {
    Ok(
        hash_stream(&mut fs::File::open(path)?, u64::MAX, &Default::default())?
            .checksum()
            .to_string(),
    )
}
fn rows(c: &Connection, sql: &str) -> librairii_storage::Result<Vec<Value>> {
    let mut s = c.prepare(sql)?;
    let names = s
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut cursor = s.query([])?;
    let mut out = Vec::new();
    while let Some(row) = cursor.next()? {
        let mut object = serde_json::Map::new();
        for (index, name) in names.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(v) => json!(v),
                ValueRef::Real(v) => json!(v),
                ValueRef::Text(v) => Value::String(
                    std::str::from_utf8(v)
                        .map_err(|_| librairii_storage::StorageError::IntegrityFailed)?
                        .into(),
                ),
                ValueRef::Blob(v) => json!({"blob":v}),
            };
            object.insert(name.clone(), value);
        }
        out.push(Value::Object(object));
    }
    Ok(out)
}
pub fn snapshot(db: &Database<'_>) -> Result<Value> {
    Ok(db.barrier_read(|tx| snapshot_connection(tx))?.wait()?)
}
pub fn snapshot_connection(tx: &Connection) -> librairii_storage::Result<Value> {
    let mut tables = serde_json::Map::new();
    for table in rows(
        tx,
        "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )? {
        let name = table["name"]
            .as_str()
            .ok_or(librairii_storage::StorageError::IntegrityFailed)?;
        tables.insert(
            name.into(),
            json!(rows(
                tx,
                &format!(
                    "SELECT * FROM \"{}\" ORDER BY rowid",
                    name.replace('"', "\"\"")
                )
            )?),
        );
    }
    Ok(
        json!({"tables":tables,"schema":rows(tx,"SELECT type,name,tbl_name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name")?,"integrity":tx.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0))?,"foreignKeyViolations":rows(tx,"PRAGMA foreign_key_check")?}),
    )
}
