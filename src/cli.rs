use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Crawl {
        #[arg(short, long)]
        url: String,
        #[arg(short, long, default_value_t = 20)]
        concurrent_parse: u16,
        #[arg(short, long, default_value_t = 50)]
        rate_limiter: u16,
    },

    Search {
        #[arg(short, long)]
        query: String,
        #[arg(short, long, default_value_t = 0)]
        page: u32,
    },
}
