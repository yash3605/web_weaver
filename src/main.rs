mod cli;
mod crawler;
mod db;
mod storage;
use crate::cli::*;
use crate::crawler::fetch_and_parse;
use clap::Parser;
use governor::{Quota, RateLimiter};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::{HashSet, VecDeque};
use std::num::NonZeroU32;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let url_to_fetch: Arc<Mutex<VecDeque<Url>>> = Arc::new(Mutex::new(VecDeque::new()));
    let visited_url: Arc<Mutex<HashSet<Url>>> = Arc::new(Mutex::new(HashSet::new()));

    let cli = cli::Cli::parse();

    let options = SqliteConnectOptions::from_str("sqlite://crawled.db")?.create_if_missing(true);

    let pool = SqlitePoolOptions::new().connect_with(options).await?;
    db::create_table(pool.clone()).await?;
    db::create_fts_table(pool.clone()).await?;
    match cli.command {
        Command::Crawl {
            url,
            concurrent_parse,
            rate_limiter,
        } => {
            let url = Url::parse(&url).unwrap();

            let global_rate_limiter = Arc::new(RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(rate_limiter as u32).unwrap(),
            )));
            let semaphore = Arc::new(Semaphore::new(concurrent_parse as usize));

            if let Err(e) = fetch_and_parse(
                url,
                visited_url,
                url_to_fetch,
                global_rate_limiter,
                semaphore,
                pool,
            )
            .await
            {
                tracing::error!("Error: {}", e);
            }
        }
        Command::Search { query, page } => {
            let results = db::search(pool, &query, page).await?;
            for row in results {
                let url: String = row.get("url");
                let title: Option<String> = row.get("title");
                let description: Option<String> = row.get("description");

                println!("URL: {}", url);
                println!("Title: {}", title.unwrap_or_default());
                println!("Description: {}", description.unwrap_or_default());
                println!("-----------")
            }

            tracing::info!("Searched with the query is done");
        }
    }

    Ok(())
}
