use crate::storage::saving_to_file;
use reqwest;
use scraper::{Html, Selector};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::task::JoinSet;
use url::Url;

pub async fn fetch_and_parse(
    url: Url,
    visited_url: Arc<Mutex<HashSet<Url>>>,
    url_to_fetch: Arc<Mutex<VecDeque<Url>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    url_to_fetch.lock().unwrap().push_back(url);

    let mut join_set: JoinSet<Result<(), Box<dyn std::error::Error + Send + Sync>>> =
        JoinSet::new();

    loop {
        while let Some(curr_url) = url_to_fetch.lock().unwrap().pop_front() {
            let url_to_fetch_clone = Arc::clone(&url_to_fetch);
            let visited_url_clone = Arc::clone(&visited_url);

            join_set.spawn(async move {
                if !visited_url_clone.lock().unwrap().contains(&curr_url) {
                    let response = match reqwest::get(curr_url.clone()).await {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("Request failed: {}", e);
                            return Ok(());
                        }
                    };
                    let resp = match response.text().await {
                        Ok(t) => t,
                        Err(e) => {
                            eprintln!("Failed to read body: {}", e);
                            return Ok(());
                        }
                    };
                    let resp = resp.as_str();

                    if let Err(e) = saving_to_file(curr_url.clone(), resp) {
                        eprintln!("Error writing File: {}", e);
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
                eprintln!("Task Error: {:?}", e);
            }
        }
    }

    Ok(())
}
