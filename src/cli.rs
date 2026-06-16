use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub url: String,
    #[arg(short, long, default_value_t = 20)]
    pub concurrent_parse: u16,
    #[arg(short, long, default_value_t = 50)]
    pub rate_limiter: u16,
}
