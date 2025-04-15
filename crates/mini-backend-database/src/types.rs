use chrono::{DateTime, Utc};
use serde::{Serialize, ser::SerializeMap};
use sqlx::{Database, Decode, Row, prelude::FromRow};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Password(String);

#[derive(Debug, Clone, Serialize)]
pub struct Email(String);

#[derive(FromRow, Debug)]
pub struct TableInfo {
    cid: i64,
    name: String,
    r#type: String,
    notnull: bool,
    dflt_value: String,
    pk: bool,
}

#[derive(Debug, Clone, Serialize)]
pub enum DatabaseTypes {
    Email(Email),
    Password(Password),
    Text(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Blob(Vec<u8>),
    Json(serde_json::Value),
}

pub struct SerializeRow<R: Row>(pub (Vec<TableInfo>, R));

impl<'r, R: Row> Serialize for &'r SerializeRow<R>
where
    R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
    usize: sqlx::ColumnIndex<R>,
    &'r str: sqlx::Decode<'r, <R as Row>::Database>,
    f64: sqlx::Decode<'r, <R as Row>::Database>,
    i64: sqlx::Decode<'r, <R as Row>::Database>,
    bool: sqlx::Decode<'r, <R as Row>::Database>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use sqlx::{Column, TypeInfo, ValueRef};

        let (table_info, row) = &self.0;

        let columns = row.columns();

        let mut map = serializer.serialize_map(Some(columns.len()))?;

        for col in columns {
            let key = col.name();
            let col_def = match table_info.iter().find(|col| col.name == key) {
                Some(col_def) => col_def,
                None => continue,
            };

            match row.try_get_raw(col.ordinal()) {
                Ok(raw_value) if !raw_value.is_null() => {
                    match col_def.r#type.to_lowercase().as_str() {
                        // sqlite primitive types
                        "text" => SerializeRow::map_serialize::<_, sqlx::Sqlite, &str>(
                            &mut map, key, raw_value,
                        ),
                        "integer" | "int4" => SerializeRow::map_serialize::<_, sqlx::Sqlite, i32>(
                            &mut map, key, raw_value,
                        ),
                        "bigint" | "int8" => SerializeRow::map_serialize::<_, sqlx::Sqlite, i64>(
                            &mut map, key, raw_value,
                        ),
                        "real" => SerializeRow::map_serialize::<_, sqlx::Sqlite, f64>(
                            &mut map, key, raw_value,
                        ),
                        "boolean" => SerializeRow::map_serialize::<_, sqlx::Sqlite, bool>(
                            &mut map, key, raw_value,
                        ),
                        col_type => {
                            let mut split = col_type.split('_');
                            let main_type = split.next().unwrap_or("");
                            let fallback_type = split.next().unwrap_or("");
                            match main_type {
                                "uuid" => SerializeRow::map_serialize::<_, sqlx::Sqlite, Uuid>(
                                    &mut map, key, raw_value,
                                ),
                                "datetime" => {
                                    SerializeRow::map_serialize::<_, sqlx::Sqlite, DateTime<Utc>>(
                                        &mut map, key, raw_value,
                                    )
                                }
                                "password" => SerializeRow::map_serialize::<_, sqlx::Sqlite, &str>(
                                    &mut map, key, raw_value,
                                ),
                                "email" => SerializeRow::map_serialize::<_, sqlx::Sqlite, &str>(
                                    &mut map, key, raw_value,
                                ),
                                "json" => {
                                    SerializeRow::map_serialize::<_, sqlx::Sqlite, serde_json::Value>(
                                        &mut map, key, raw_value,
                                    )
                                }
                                _ => SerializeRow::map_serialize::<_, sqlx::Sqlite, Vec<u8>>(
                                    &mut map, key, raw_value,
                                ),
                            }
                        }
                    }
                }
                _ => map.serialize_entry(key, &()),
            }?
        }

        map.end()
    }
}

impl<'r, R: Row> SerializeRow<R>
where
    R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
    usize: sqlx::ColumnIndex<R>,
    &'r str: sqlx::Decode<'r, <R as Row>::Database>,
    f64: sqlx::Decode<'r, <R as Row>::Database>,
    i64: sqlx::Decode<'r, <R as Row>::Database>,
    bool: sqlx::Decode<'r, <R as Row>::Database>,
{
    fn map_serialize<M: SerializeMap, DB: Database, T: Decode<'r, DB> + Serialize>(
        map: &mut M,
        key: &str,
        raw_value: <DB as Database>::ValueRef<'r>,
    ) -> Result<(), M::Error> {
        let val = T::decode(raw_value).map_err(serde::ser::Error::custom)?;
        map.serialize_entry(key, &val)
    }
}

pub trait SqlxJsonExt<'r, R>
where
    R: Row,
    R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
    for<'a> &'a SerializeRow<R>: Serialize,
{
    fn to_json(
        self,
        table_info: Vec<TableInfo>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>>
    where
        R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
        for<'a> &'a SerializeRow<R>: Serialize;
}

impl<'r, R: Row> SqlxJsonExt<'r, R> for R
where
    R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
    for<'a> &'a SerializeRow<R>: Serialize,
{
    fn to_json(
        self,
        table_info: Vec<TableInfo>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>>
    where
        R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
        for<'a> &'a SerializeRow<R>: Serialize,
    {
        let serialize_row = SerializeRow((table_info, self));
        let val = serde_json::to_value(&serialize_row)?;
        Ok(val)
    }
}

pub fn to_json<'r, R: Row>(
    row: R,
    table_info: Vec<TableInfo>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>>
where
    R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
    for<'a> &'a SerializeRow<R>: Serialize,
{
    let serialize_row = SerializeRow((table_info, row));
    let val = serde_json::to_value(&serialize_row)?;
    Ok(val)
}
