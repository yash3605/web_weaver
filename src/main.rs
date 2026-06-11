mod crawler;
mod storage;
use crate::crawler::fetch_and_parse;
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url_to_fetch: Arc<Mutex<VecDeque<Url>>> = Arc::new(Mutex::new(VecDeque::new()));
    let visited_url: Arc<Mutex<HashSet<Url>>> = Arc::new(Mutex::new(HashSet::new()));

    if let Err(e) = fetch_and_parse(
        Url::parse("https://rust-lang.org").unwrap(),
        visited_url,
        url_to_fetch,
    )
    .await
    {
        eprintln!("Error: {}", e);
    }
    Ok(())
}
