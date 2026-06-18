use sqlx::{self, Error, Result, SqlitePool};

pub async fn create_table(pool: SqlitePool) -> Result<(), Error> {
    let mut conn = pool.acquire().await.unwrap();

    let table_created = sqlx::query(
        "CREATE TABLE IF NOT EXISTS pages (                
                     id INTEGER PRIMARY KEY AUTOINCREMENT,        
                      url TEXT NOT NULL UNIQUE,                    
                      title TEXT,                                  
                      description TEXT,                            
                      keywords TEXT,                               
                      raw_html TEXT,                               
                      crawled_at DATETIME DEFAULT CURRENT_TIMESTAMP
                  )",
    )
    .execute(&mut *conn)
    .await;

    match table_created {
        Ok(_) => {}
        Err(e) => return Err(e),
    }

    Ok(())
}

pub async fn insert_page(
    pool: &SqlitePool,
    url: &str,
    title: Option<String>,
    description: Option<String>,
    keywords: Option<String>,
    raw_html: String,
) -> Result<(), Error> {
    let mut conn = pool.acquire().await.unwrap();

    sqlx::query(
        "INSERT INTO pages (url, title, description, keywords, raw_html) VALUES (?1, ?2, ?3, ?4, ?5)"
    )
        .bind(url)
        .bind(title)
        .bind(description)
        .bind(keywords)
        .bind(raw_html)
    .execute(&mut *conn).await?;

    Ok(())
}
