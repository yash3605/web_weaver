use sqlx::{self, Error, Result, SqlitePool, sqlite::SqliteRow};

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

pub async fn create_fts_table(pool: SqlitePool) -> Result<(), Error> {
    let mut conn = pool.acquire().await?;

    sqlx::query(
        "CREATE VIRTUAL TABLE IF NOT EXISTS pages_fts USING fts5(
                url, title, description, keywords, content='pages', content_rowid='id'
            )",
    )
    .execute(&mut *conn)
    .await?;

    sqlx::query(
        "CREATE TRIGGER IF NOT EXISTS pages_ai AFTER INSERT ON pages BEGIN
    INSERT INTO pages_fts(rowid, url, title, description, keywords)
    VALUES (new.id, new.url, new.title, new.description, new.keywords);
END;",
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

pub async fn search(pool: SqlitePool, query: &str, page: u32) -> Result<Vec<SqliteRow>, Error> {
    let mut conn = pool.acquire().await?;

    let offset = page * 10;
    let search_results = sqlx::query(
        "
            SELECT p.url, p.title, p.description
            FROM pages p
            JOIN pages_fts ON pages_fts.rowid = p.id
            WHERE pages_fts MATCH ?1
            ORDER BY rank
            LIMIT 10 OFFSET ?2; 
        ",
    )
    .bind(query)
    .bind(offset)
    .fetch_all(&mut *conn)
    .await?;

    Ok(search_results)
}
