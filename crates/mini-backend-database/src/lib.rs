pub mod types;

#[cfg(test)]
pub mod test {
    #![allow(clippy::approx_constant)]
    use chrono::Utc;
    use serde_json::json;
    use sqlx::{Pool, Sqlite};
    use uuid::Uuid;

    use crate::types::{SqlxJsonExt, TableInfo};

    /// Helper function to create the test table with all supported types
    async fn setup_test_table(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
        let test_table_query = r#"
            CREATE TABLE IF NOT EXISTS type_test (
                id UUID_TEXT PRIMARY KEY,
                simple_text TEXT,
                nullable_text TEXT NULL,
                email EMAIL_TEXT,
                password PASSWORD_TEXT,
                int_value INTEGER,
                big_int BIGINT,
                real_value REAL,
                bool_value BOOLEAN,
                json_data JSON_TEXT,
                datetime_value DATETIME_TEXT,
                blob_data BLOB
            );
        "#;

        sqlx::query(test_table_query).execute(pool).await?;
        Ok(())
    }

    /// Helper function to get table info for a given table
    async fn get_table_info(
        pool: &Pool<Sqlite>,
        table_name: &str,
    ) -> Result<Vec<TableInfo>, sqlx::Error> {
        sqlx::query_as::<_, TableInfo>(&format!("PRAGMA table_info('{}')", table_name))
            .fetch_all(pool)
            .await
    }

    #[sqlx::test]
    async fn test_basic_type_conversion(
        pool: Pool<Sqlite>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Setup test table
        setup_test_table(&pool).await?;

        // Generate test data
        let test_id = Uuid::new_v4();
        let test_time = Utc::now();
        let test_json = json!({"key": "value"});

        // Insert test data with basic types
        sqlx::query(
            r#"INSERT INTO type_test
               (id, simple_text, nullable_text, email, password, int_value, big_int, 
                real_value, bool_value, json_data, datetime_value, blob_data)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(test_id)
        .bind("Simple text")
        .bind::<Option<&str>>(None) // Test NULL handling
        .bind("test@example.com")
        .bind("password123")
        .bind(42)
        .bind(1234567890_i64)
        .bind(3.14)
        .bind(true)
        .bind(test_json.clone()) // Clone the JSON to avoid move issues
        .bind(test_time)
        .bind(vec![1, 2, 3])
        .execute(&pool)
        .await?;

        // Get table info for conversion
        let table_info = get_table_info(&pool, "type_test").await?;

        // Convert row to JSON
        let json_row = sqlx::query("SELECT * FROM type_test")
            .fetch_one(&pool)
            .await?
            .to_json(&table_info)?;

        // Basic type assertions
        assert_eq!(json_row["id"].as_str().unwrap(), test_id.to_string());
        assert_eq!(json_row["simple_text"].as_str().unwrap(), "Simple text");
        assert!(json_row["nullable_text"].is_null());
        assert_eq!(json_row["email"].as_str().unwrap(), "test@example.com");
        assert_eq!(json_row["password"].as_str().unwrap(), "password123");
        assert_eq!(json_row["int_value"].as_i64().unwrap(), 42);
        assert_eq!(json_row["big_int"].as_i64().unwrap(), 1234567890);
        assert!((json_row["real_value"].as_f64().unwrap() - 3.14).abs() < f64::EPSILON);
        assert!(json_row["bool_value"].as_bool().unwrap());
        assert_eq!(json_row["json_data"]["key"].as_str().unwrap(), "value");

        Ok(())
    }

    #[sqlx::test]
    async fn test_edge_cases(pool: Pool<Sqlite>) -> Result<(), Box<dyn std::error::Error>> {
        // Setup test table
        setup_test_table(&pool).await?;

        // Test edge case values
        let max_int64 = i64::MAX;
        let min_int64 = i64::MIN;
        let special_chars = "Special characters: !@#$%^&*()\n\t\"'\\";

        let deep_nested_json = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "level4": {
                            "array": [1, 2, 3, 4, 5],
                            "boolean": true,
                            "null": null
                        }
                    }
                }
            }
        });

        let empty_json = json!({});
        let json_array = json!([1, 2, 3, 4, 5]);
        let large_blob = vec![0u8; 1024]; // 1KB blob

        // Insert edge case values
        sqlx::query(
            r#"INSERT INTO type_test
               (id, simple_text, nullable_text, email, password, int_value, big_int, 
                real_value, bool_value, json_data, datetime_value, blob_data)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(Uuid::new_v4())
        .bind(special_chars)
        .bind(Some("")) // Empty string instead of NULL
        .bind("very.long.email.address.with.many.parts@example.com")
        .bind("extremely-long-password-that-exceeds-normal-length-limits-123456789")
        .bind(0) // Zero
        .bind(max_int64) // Max int64
        .bind(f64::MAX) // Max float
        .bind(false)
        .bind(deep_nested_json.clone()) // Clone to avoid move
        .bind(Utc::now())
        .bind(large_blob.clone())
        .execute(&pool)
        .await?;

        // Insert another edge case row
        sqlx::query(
            r#"INSERT INTO type_test
               (id, simple_text, nullable_text, email, password, int_value, big_int, 
                real_value, bool_value, json_data, datetime_value, blob_data)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(Uuid::new_v4())
        .bind("") // Empty string
        .bind(None::<String>) // NULL
        .bind("email@example") // Non-standard email
        .bind("") // Empty password
        .bind(-1) // Negative integer
        .bind(min_int64) // Min int64
        .bind(0.0) // Zero float
        .bind(false)
        .bind(empty_json.clone()) // Clone the empty JSON before using it
        .bind(Utc::now())
        .bind(vec![0u8; 0]) // Empty blob
        .execute(&pool)
        .await?;

        // Insert a row with JSON array
        sqlx::query(
            r#"INSERT INTO type_test
               (id, simple_text, nullable_text, email, password, int_value, big_int, 
                real_value, bool_value, json_data, datetime_value, blob_data)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(Uuid::new_v4())
        .bind("JSON Array Test")
        .bind(None::<String>)
        .bind("array@example.com")
        .bind("password")
        .bind(42)
        .bind(42_i64)
        .bind(4.2)
        .bind(true)
        .bind(json_array.clone()) // Clone the JSON array before using it
        .bind(Utc::now())
        .bind(vec![9, 8, 7])
        .execute(&pool)
        .await?;

        // Get table info for conversion
        let table_info = get_table_info(&pool, "type_test").await?;

        // Convert rows to JSON
        let json_rows = sqlx::query("SELECT * FROM type_test")
            .fetch_all(&pool)
            .await?
            .into_iter()
            .map(|row| row.to_json(&table_info))
            .collect::<Result<Vec<_>, _>>()?;

        // We should have 3 rows
        assert_eq!(json_rows.len(), 3, "Expected 3 rows");

        // Test first row with special values
        let row = &json_rows[0];
        assert_eq!(row["simple_text"].as_str().unwrap(), special_chars);
        assert_eq!(row["big_int"].as_i64().unwrap(), max_int64);
        assert!(row["json_data"]["level1"]["level2"]["level3"]["level4"]["array"].is_array());
        assert_eq!(
            row["json_data"]["level1"]["level2"]["level3"]["level4"]["array"][0]
                .as_i64()
                .unwrap(),
            1
        );

        // Test second row with empty/minimal values
        let row = &json_rows[1];
        assert_eq!(row["simple_text"].as_str().unwrap(), "");
        assert!(row["nullable_text"].is_null());
        assert_eq!(row["big_int"].as_i64().unwrap(), min_int64);

        // Create a new empty_json for comparison since the original was moved
        let empty_json_compare = json!({});
        assert_eq!(row["json_data"], empty_json_compare);

        // Test JSON array row
        let row = &json_rows[2];
        assert!(row["json_data"].is_array());
        assert_eq!(row["json_data"][0].as_i64().unwrap(), 1);
        assert_eq!(row["json_data"][4].as_i64().unwrap(), 5);

        Ok(())
    }

    #[sqlx::test]
    async fn test_multiple_rows_and_aggregation(
        pool: Pool<Sqlite>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create a simple table for testing multi-row operations
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS multi_row_test (
                id INTEGER PRIMARY KEY,
                name TEXT,
                value INTEGER
            );
        "#,
        )
        .execute(&pool)
        .await?;

        // Insert multiple rows
        for i in 1..=100 {
            sqlx::query("INSERT INTO multi_row_test (id, name, value) VALUES (?, ?, ?)")
                .bind(i)
                .bind(format!("Item {}", i))
                .bind(i * 10)
                .execute(&pool)
                .await?;
        }

        // Get table info
        let table_info = get_table_info(&pool, "multi_row_test").await?;

        // Test fetching many rows
        let all_rows = sqlx::query("SELECT * FROM multi_row_test")
            .fetch_all(&pool)
            .await?
            .into_iter()
            .map(|row| row.to_json(&table_info))
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(all_rows.len(), 100, "Should have 100 rows");

        // Test fetching a subset of rows with conditions
        let filtered_rows = sqlx::query("SELECT * FROM multi_row_test WHERE value > 500")
            .fetch_all(&pool)
            .await?
            .into_iter()
            .map(|row| row.to_json(&table_info))
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(
            filtered_rows.len(),
            50,
            "Should have 50 rows with value > 500"
        );

        // Test with aggregate query
        let aggregate_row = sqlx::query(
            "SELECT COUNT(*) as count, SUM(value) as sum, AVG(value) as avg FROM multi_row_test",
        )
        .fetch_one(&pool)
        .await?;

        // Create table info for the aggregate query
        let agg_table_info = vec![
            TableInfo {
                cid: 0,
                name: "count".to_string(),
                r#type: "INTEGER".to_string(),
                notnull: false,
                dflt_value: "".to_string(),
                pk: false,
            },
            TableInfo {
                cid: 1,
                name: "sum".to_string(),
                r#type: "INTEGER".to_string(),
                notnull: false,
                dflt_value: "".to_string(),
                pk: false,
            },
            TableInfo {
                cid: 2,
                name: "avg".to_string(),
                r#type: "REAL".to_string(),
                notnull: false,
                dflt_value: "".to_string(),
                pk: false,
            },
        ];

        let agg_json = aggregate_row.to_json(&agg_table_info)?;

        assert_eq!(agg_json["count"].as_i64().unwrap(), 100);
        assert_eq!(agg_json["sum"].as_i64().unwrap(), 50500); // Sum of 10*i for i from 1 to 100
        assert!((agg_json["avg"].as_f64().unwrap() - 505.0).abs() < f64::EPSILON);

        Ok(())
    }
}
