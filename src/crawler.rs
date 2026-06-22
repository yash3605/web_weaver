use crate::db;
use crate::storage::saving_to_file;
use governor::RateLimiter;
use governor::clock::{QuantaClock, QuantaInstant};
use governor::middleware::NoOpMiddleware;
use governor::state::{InMemoryState, NotKeyed};
use reqwest;
use scraper::{Html, Selector};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use url::Url;

pub async fn fetch_and_parse(
    url: Url,
    visited_url: Arc<Mutex<HashSet<Url>>>,
    url_to_fetch: Arc<Mutex<VecDeque<Url>>>,
    rate_limiter: Arc<
        RateLimiter<NotKeyed, InMemoryState, QuantaClock, NoOpMiddleware<QuantaInstant>>,
    >,
    semaphore: Arc<Semaphore>,
    pool: SqlitePool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    url_to_fetch.lock().unwrap().push_back(url);

    let mut join_set: JoinSet<Result<Option<(Url, String)>, Box<dyn Error + Send + Sync>>> =
        JoinSet::new();
    let mut robots_map: HashMap<String, String> = HashMap::new();

    loop {
        while let Some(curr_url) = url_to_fetch.lock().unwrap().pop_front() {
            let host = curr_url.host_str().unwrap_or("Unknown").to_string();

            if !robots_map.contains_key(&host) {
                tracing::info!("Fetching robots.txt for: {}", host);
                let robots = match reqwest::get(format!(
                    "{}://{}/robots.txt",
                    curr_url.scheme(),
                    curr_url.host_str().unwrap_or("")
                ))
                .await
                {
                    Ok(r) => r.text().await.unwrap_or_default(),
                    Err(_) => String::new(),
                };
                robots_map.insert(host.clone(), robots);
            }

            let curr_robot = robots_map[&host].clone();
            let url_str = curr_url.to_string();

            let allowed = robotstxt::DefaultMatcher::default().one_agent_allowed_by_robots(
                &curr_robot,
                "web_weaver",
                &url_str,
            );

            if !allowed {
                tracing::info!("Skipping (robots.txt): {}", curr_url);
                continue;
            }

            let rate_limiter = rate_limiter.clone();
            let _url_to_fetch_clone = Arc::clone(&url_to_fetch);
            let visited_url_clone = Arc::clone(&visited_url);

            let semaphore = semaphore.clone();
            tracing::info!("Crawling: {}", curr_url);
            join_set.spawn(async move {
                if visited_url_clone.lock().unwrap().contains(&curr_url) {
                    return Ok(None);
                }

                let _permit = semaphore.acquire_owned().await.unwrap();
                rate_limiter.until_ready().await;
                let response = match reqwest::get(curr_url.clone()).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("Request failed: {}", e);
                        return Ok(None);
                    }
                };
                let resp = match response.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::error!("Failed to read body: {}", e);
                        return Ok(None);
                    }
                };
                Ok(Some((curr_url, resp)))
            });
        }

        if join_set.is_empty() {
            break;
        }

        let pool = pool.clone();
        if let Some(result) = join_set.join_next().await {
            match result {
                Err(e) => tracing::error!("Task Error: {:?}", e),
                Ok(Err(e)) => tracing::error!("Task error: {:?}", e),
                Ok(Ok(None)) => {}
                Ok(Ok(Some((url, resp)))) => {
                    if let Err(e) = saving_to_file(url.clone(), resp.as_str()) {
                        tracing::error!("Error writing File: {}", e);
                    }

                    let fragment = Html::parse_fragment(resp.clone().as_str());
                    let title = fragment
                        .select(&Selector::parse("title").unwrap())
                        .next()
                        .map(|el| el.text().collect::<String>().trim().to_string());
                    let keywords = fragment
                        .select(&Selector::parse("meta[name='keywords']").unwrap())
                        .next()
                        .and_then(|el| el.value().attr("content"))
                        .map(|s| s.to_string());
                    let description = fragment
                        .select(&Selector::parse("meta[name='description']").unwrap())
                        .next()
                        .and_then(|el| el.value().attr("content"))
                        .map(|s| s.to_string());
                    let selector = Selector::parse("a").unwrap();

                    let base = format!("{}://{}", url.scheme(), url.host_str().unwrap_or(""));

                    let links: Vec<String> = fragment
                        .select(&selector)
                        .filter_map(|el| el.value().attr("href"))
                        .map(|s| s.to_string())
                        .collect();

                    drop(selector);
                    drop(fragment);
                    for href in links {
                        let parsed_url = Url::parse(href.as_str());

                        match parsed_url {
                            Err(_e) => {
                                let comp_url =
                                    Url::parse(&base).unwrap().join(href.as_str()).unwrap();
                                if visited_url.lock().unwrap().contains(&comp_url) {
                                    continue;
                                } else {
                                    url_to_fetch.lock().unwrap().push_back(comp_url);
                                }
                            }
                            Ok(t) => {
                                if t.host_str() != url.host_str() {
                                    continue;
                                } else {
                                    if visited_url.lock().unwrap().contains(&t) {
                                        continue;
                                    } else {
                                        url_to_fetch.lock().unwrap().push_back(t);
                                    }
                                }
                            }
                        };
                    }
                    let insert_page =
                        db::insert_page(&pool, url.as_str(), title, description, keywords, resp)
                            .await;
                    if let Err(e) = insert_page {
                        if e.to_string().contains("UNIQUE constraint failed") {
                            tracing::debug!("URL already in DB, skipping: {}", url);
                        } else {
                            tracing::error!("Error inserting into DB: {}", e);
                        }
                    };
                    visited_url.lock().unwrap().insert(url);
                }
            }
        }
    }
    Ok(())
}
