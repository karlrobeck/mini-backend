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
    pub cid: i64,
    pub name: String,
    pub r#type: String,
    pub notnull: bool,
    pub dflt_value: String,
    pub pk: bool,
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

pub struct SerializeRow<'a, R: Row>(pub (&'a Vec<TableInfo>, R));

impl<'r, R: Row> Serialize for &'r SerializeRow<'_, R>
where
    R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
    <R as Row>::Database: 'r,
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
        use sqlx::{Column, ValueRef};

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
                    // Process types according to the column definition
                    let type_str = col_def.r#type.to_lowercase();

                    // Handle the column type based on the type string
                    self.process_column_type(&mut map, key, &type_str, raw_value)?;
                }
                _ => map.serialize_entry(key, &())?,
            }
        }

        map.end()
    }
}

impl<'r, R: Row> SerializeRow<'_, R>
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

    fn process_column_type<'s, M: SerializeMap>(
        &'s self,
        map: &mut M,
        key: &str,
        type_str: &str,
        raw_value: sqlx::sqlite::SqliteValueRef<'r>,
    ) -> Result<(), M::Error> {
        // First check for primitive types
        match type_str {
            // Handle primitive sqlite types directly
            "text" => return Self::map_serialize::<_, sqlx::Sqlite, &str>(map, key, raw_value),
            "integer" | "int4" => {
                return Self::map_serialize::<_, sqlx::Sqlite, i32>(map, key, raw_value);
            }
            "bigint" | "int8" => {
                return Self::map_serialize::<_, sqlx::Sqlite, i64>(map, key, raw_value);
            }
            "real" => return Self::map_serialize::<_, sqlx::Sqlite, f64>(map, key, raw_value),
            "boolean" => return Self::map_serialize::<_, sqlx::Sqlite, bool>(map, key, raw_value),
            _ => {} // Continue to compound type handling
        }

        // If not a primitive, handle compound types
        let mut split = type_str.split('_');
        let main_type = split.next().unwrap_or("");
        let fallback_type = split.next().unwrap_or("");

        // Try to handle based on main type
        match main_type {
            "uuid" => return Self::map_serialize::<_, sqlx::Sqlite, Uuid>(map, key, raw_value),
            "datetime" => {
                return Self::map_serialize::<_, sqlx::Sqlite, DateTime<Utc>>(map, key, raw_value);
            }
            "password" | "email" => {
                return Self::map_serialize::<_, sqlx::Sqlite, &str>(map, key, raw_value);
            }
            "json" => {
                return Self::map_serialize::<_, sqlx::Sqlite, serde_json::Value>(
                    map, key, raw_value,
                );
            }
            _ => {} // Continue to fallback type handling
        }

        // Fall back to secondary type if needed
        match fallback_type {
            "text" => Self::map_serialize::<_, sqlx::Sqlite, &str>(map, key, raw_value),
            "integer" | "int4" => Self::map_serialize::<_, sqlx::Sqlite, i32>(map, key, raw_value),
            "bigint" | "int8" => Self::map_serialize::<_, sqlx::Sqlite, i64>(map, key, raw_value),
            "real" => Self::map_serialize::<_, sqlx::Sqlite, f64>(map, key, raw_value),
            "boolean" => Self::map_serialize::<_, sqlx::Sqlite, bool>(map, key, raw_value),
            _ => Self::map_serialize::<_, sqlx::Sqlite, &str>(map, key, raw_value),
        }
    }
}

pub trait SqlxJsonExt<'r, R>
where
    R: Row,
    R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
    for<'a> &'a SerializeRow<'a, R>: Serialize,
{
    fn to_json(
        self,
        table_info: &Vec<TableInfo>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>>
    where
        R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
        for<'a> &'a SerializeRow<'a, R>: Serialize;
}

impl<'r, R: Row> SqlxJsonExt<'r, R> for R
where
    R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
    for<'a> &'a SerializeRow<'a, R>: Serialize,
{
    fn to_json(
        self,
        table_info: &Vec<TableInfo>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>>
    where
        R::Database: sqlx::Database<ValueRef<'r> = sqlx::sqlite::SqliteValueRef<'r>>,
        for<'a> &'a SerializeRow<'a, R>: Serialize,
    {
        let serialize_row = SerializeRow((table_info, self));
        let val = serde_json::to_value(&serialize_row)?;
        Ok(val)
    }
}
