use crate::storage::saving_to_file;
use governor::RateLimiter;
use governor::clock::{QuantaClock, QuantaInstant};
use governor::middleware::NoOpMiddleware;
use governor::state::{InMemoryState, NotKeyed};
use reqwest;
use scraper::{Html, Selector};
use std::collections::{HashMap, HashSet, VecDeque};
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
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    url_to_fetch.lock().unwrap().push_back(url);

    let mut join_set: JoinSet<Result<(), Box<dyn std::error::Error + Send + Sync>>> =
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
            let url_to_fetch_clone = Arc::clone(&url_to_fetch);
            let visited_url_clone = Arc::clone(&visited_url);

            let semaphore = semaphore.clone();
            tracing::info!("Crawling: {}", curr_url);
            join_set.spawn(async move {
                if !visited_url_clone.lock().unwrap().contains(&curr_url) {
                    let _permit = semaphore.acquire_owned().await.unwrap();
                    rate_limiter.until_ready().await;
                    let response = match reqwest::get(curr_url.clone()).await {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!("Request failed: {}", e);
                            return Ok(());
                        }
                    };
                    let resp = match response.text().await {
                        Ok(t) => t,
                        Err(e) => {
                            tracing::error!("Failed to read body: {}", e);
                            return Ok(());
                        }
                    };
                    let resp = resp.as_str();

                    if let Err(e) = saving_to_file(curr_url.clone(), resp) {
                        tracing::error!("Error writing File: {}", e);
                    }

                    let fragment = Html::parse_fragment(resp);
                    let selector = Selector::parse("a").unwrap();

                    let base = format!(
                        "{}://{}",
                        curr_url.scheme(),
                        curr_url.host_str().unwrap_or("")
                    );

                    for links in fragment.select(&selector) {
                        let Some(href) = links.attr("href") else {
                            continue;
                        };
                        let url = Url::parse(href);

                        match url {
                            Err(_e) => {
                                let comp_url = Url::parse(&base).unwrap().join(href).unwrap();
                                if visited_url_clone.lock().unwrap().contains(&comp_url) {
                                    continue;
                                } else {
                                    url_to_fetch_clone.lock().unwrap().push_back(comp_url);
                                }
                            }
                            Ok(t) => {
                                if t.host_str() != curr_url.host_str() {
                                    continue;
                                } else {
                                    if visited_url_clone.lock().unwrap().contains(&t) {
                                        continue;
                                    } else {
                                        url_to_fetch_clone.lock().unwrap().push_back(t);
                                    }
                                }
                            }
                        };
                    }
                    visited_url_clone.lock().unwrap().insert(curr_url);
                }
                Ok(())
            });
        }

        if join_set.is_empty() {
            break;
        }

        if let Some(result) = join_set.join_next().await {
            if let Err(e) = result {
                tracing::error!("Task Error: {:?}", e);
            }
        }
    }

    Ok(())
}
