use std::io::Write;
use std::{fs, io};
use url::Url;

fn append_to_file(path: String, content: &str) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)?;

    file.write_all(content.as_bytes())?;
    Ok(())
}

pub fn saving_to_file(curr_url: Url, response: &str) -> io::Result<()> {
    if let Err(e) = fs::create_dir_all("crawled") {
        eprintln!("Error creating directory: {}", e);
    }

    let url = curr_url.path();
    let url = url.replace("/", "_");

    let host = curr_url.host().unwrap();
    let path = format!("./crawled/{}{}.txt", host, url);

    let file_state = append_to_file(path, response);

    match file_state {
        Ok(_) => println!("File Written Successfully"),
        Err(_) => println!("Error writing File"),
    };

    Ok(())
}
