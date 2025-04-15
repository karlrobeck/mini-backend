use serde::Serialize;
use sqlx::{
    Column, Decode, Row,
    prelude::FromRow,
    sqlite::{SqliteRow, SqliteTypeInfo},
};
use types::{SerializeRow, TableInfo};

pub mod types;

#[cfg(test)]
pub mod test {
    use chrono::Utc;
    use serde_json::json;
    use sqlx::{Pool, Sqlite};
    use uuid::Uuid;

    use crate::types::{SerializeRow, SqlxJsonExt, TableInfo, to_json};

    #[sqlx::test]
    async fn test_conversion_to_json(pool: Pool<Sqlite>) -> Result<(), Box<dyn std::error::Error>> {
        let test_table_query = r#"
            create table if not exists temp_table (
                id uuid_text not null primary key,
                name text not null,
                email email_text not null,
                password password_text not null,
                email_verified boolean not null,
                age integer not null,
                weight real not null,
                metadata json_text not null,
                created datetime_text not null,
                updated datetime_text not null
            );
        "#;

        sqlx::query(test_table_query).execute(&pool).await?;

        sqlx::query(
            "insert into temp_table(id,name,email,password,email_verified,age,weight,metadata,created,updated) values (?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(Uuid::new_v4())
        .bind("john doe")
        .bind("johndoe@email.com")
        .bind("my-randomPassword1")
        .bind(true)
        .bind(16)
        .bind(50.7)
        .bind(json!({
            "profile":"http://localhost:443/profile.jpeg"
        }))
        .bind(Utc::now())
        .bind(Utc::now())
        .execute(&pool)
        .await?;

        let table_info = sqlx::query_as::<_, TableInfo>("pragma table_info('temp_table')")
            .fetch_all(&pool)
            .await?;

        let row = sqlx::query("select * from temp_table")
            .fetch_one(&pool)
            .await?
            .to_json(table_info)?;

        println!("{:#?}", row);

        Ok(())
    }
}
