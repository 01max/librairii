//! Lossless native paths at SQLite TEXT and optional native-control boundaries.
//! UTF-8 control records retain the existing string wire representation.
use librairii_app::ports::operations::OperationPath;
use rusqlite::{
    ToSql,
    types::{ToSqlOutput, ValueRef},
};
use std::path::Path;
pub(crate) struct SqlPath<'a>(pub &'a Path);
impl ToSql for SqlPath<'_> {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(
            self.0.as_os_str().as_encoded_bytes(),
        )))
    }
}
pub(crate) fn from_bytes(bytes: &[u8]) -> Result<OperationPath, std::str::Utf8Error> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(std::ffi::OsString::from_vec(bytes.to_vec()).into())
    }
    #[cfg(not(unix))]
    {
        Ok(std::str::from_utf8(bytes)?.into())
    }
}
pub(crate) fn optional_sql_path(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<OperationPath>> {
    match row.get_ref(index)? {
        ValueRef::Null => Ok(None),
        ValueRef::Text(bytes) => from_bytes(bytes).map(Some).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                Box::new(e),
            )
        }),
        _ => Err(rusqlite::Error::InvalidColumnType(
            index,
            "export_destination".into(),
            row.get_ref(index)?.data_type(),
        )),
    }
}
pub(crate) mod optional_wire {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    #[derive(Serialize)]
    #[serde(untagged)]
    enum Ref<'a> {
        Legacy(&'a str),
        Unix { unix_bytes: &'a [u8] },
    }
    #[derive(Deserialize)]
    #[serde(untagged, deny_unknown_fields)]
    enum Owned {
        Legacy(String),
        Unix { unix_bytes: Vec<u8> },
    }
    pub fn serialize<S: Serializer>(
        path: &Option<OperationPath>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        path.as_ref()
            .map(|p| match p.to_str() {
                Some(p) => Ref::Legacy(p),
                None => Ref::Unix {
                    unix_bytes: p.as_os_str().as_encoded_bytes(),
                },
            })
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<OperationPath>, D::Error> {
        Option::<Owned>::deserialize(deserializer)?
            .map(|p| match p {
                Owned::Legacy(p) => Ok(p.into()),
                Owned::Unix { unix_bytes } => {
                    #[cfg(unix)]
                    {
                        from_bytes(&unix_bytes).map_err(serde::de::Error::custom)
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = unix_bytes;
                        Err(serde::de::Error::custom("foreign native path encoding"))
                    }
                }
            })
            .transpose()
    }
}
