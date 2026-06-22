Web Weaver
===========

Web Weaver is a small, single-binary web crawler and full-text indexer written in Rust. It crawls pages (same-host only), stores raw HTML on disk and indexed metadata in a SQLite database (with an FTS5 virtual table), and provides a simple CLI to crawl sites and search the indexed content.

Highlights
- Simple, focused crawler that respects robots.txt (basic matching using the `robotstxt` crate)
- Rate limiting and concurrency controls (using `governor` and Tokio `Semaphore`)
- Stores pages in a SQLite table and exposes full-text search via FTS5
- Writes raw HTML files into a `crawled/` directory for inspection

When to use
- Quickly crawl a single hostname and build an on-disk archive + searchable index
- Experimenting with crawling techniques, simple indexing and searching with SQLite FTS

Not for production
- This is a demonstration / utility level crawler. It does not provide advanced features needed for large-scale production crawlers (distributed frontier, robust retry/backoff per-host, content deduplication across domains, dynamic JS rendering, sitemaps ingestion, etc.).

Repository layout
- Cargo.toml - project manifest and dependencies
- src/
  - main.rs - CLI parsing and entry point that wires components together
  - cli.rs - CLI definitions (crawl and search subcommands)
  - crawler.rs - core crawling logic: robots.txt checks, fetching, parsing links, rate limiting, concurrency and inserting pages into DB
  - db.rs - database schema creation and helper functions (creates pages table, FTS table and trigger, performs searches)
  - storage.rs - writes raw HTML files to ./crawled/
  - types.rs - (empty placeholder)
- crawled/ - (created at runtime) raw HTML files saved during a crawl
- crawled.db - SQLite database created at runtime (default name)

Build requirements
- Rust toolchain (edition 2024 is declared in Cargo.toml — use a recent stable toolchain)
- sqlite3 development headers if your platform's SQLite does not include FTS5. On Debian/Ubuntu: `sudo apt install libsqlite3-dev`.

Dependencies (from Cargo.toml)
- governor (rate limiting)
- sqlx (SQLite driver; features: runtime-tokio, sqlite)
- tokio (async runtime)
- reqwest (HTTP client)
- clap (CLI parsing)
- scraper (HTML parsing)
- url (URL parsing/manipulation)
- robotstxt (robots.txt matching)
- tracing / tracing-subscriber (logging)

Quick start

1. Build the project

   cargo build --release

2. Crawl a site

   cargo run --release -- crawl -u https://example.com -c 20 -r 50

   - `-u, --url` url to start crawling (required for the `crawl` subcommand)
   - `-c, --concurrent-parse` number of concurrent page fetch/parse tasks (default: 20)
   - `-r, --rate-limiter` requests per second the global rate limiter allows (default: 50)

   The crawler will:
   - Respect robots.txt using the user-agent string `web_weaver`.
   - Fetch pages from the starting host only (links that change host are skipped).
   - Save raw HTML to `./crawled/{host}{path}.txt` (slashes in path replaced with underscores).
   - Insert page metadata and raw HTML into `crawled.db` (table `pages`).

3. Search the index

   cargo run --release -- search -q "your query" -p 0

   - `-q, --query` the FTS query
   - `-p, --page` pagination page (10 results per page, 0-based)

   Example output prints URL, title and description for each matched page.

Database schema and indexing
- pages table (created by db::create_table)
  - id INTEGER PRIMARY KEY AUTOINCREMENT
  - url TEXT NOT NULL UNIQUE
  - title TEXT
  - description TEXT
  - keywords TEXT
  - raw_html TEXT
  - crawled_at DATETIME DEFAULT CURRENT_TIMESTAMP

- pages_fts (created by db::create_fts_table)
  - Virtual FTS5 table: url, title, description, keywords
  - A trigger (pages_ai) inserts rows into pages_fts after a new row in pages is added

Notes about search implementation
- The search command runs a MATCH query against the FTS5 virtual table and limits results to 10 per page.
- Known issue: the SQL uses `ORDER BY rank` but FTS5 does not provide a built-in `rank` column by that name by default. If you get SQL errors related to `rank`, replace the ORDER BY clause with an appropriate ranking expression (for example using `bm25(pages_fts)` or a custom rank function). See the `Known issues` section below.

Data storage details
- Raw HTML file naming: `./crawled/{host}{path}.txt` where path slashes are replaced with underscores. Examples:
  - URL `https://example.com/` -> `./crawled/example.com_.txt`
  - URL `https://example.com/articles/1` -> `./crawled/example.com_articles_1.txt`
- The code appends to files (OpenOptions::append). Re-crawling the same path will append more HTML to an existing file rather than overwrite it.

Design and behavior details
- The crawler is single-process and keeps its frontier and visited set in memory (HashSet and VecDeque). If the process stops, the frontier is lost.
- Only same-host links are enqueued. External links are ignored.
- Robots.txt is fetched once per host and cached for the lifetime of the process.
- The global rate limiter is applied before every HTTP request. Concurrency is also limited with a Tokio `Semaphore`.
- Duplicate URLs are prevented in the database using a UNIQUE constraint on the `url` column. The insertion code catches UNIQUE constraint failures and skips them.

Configuration and tuning
- concurrent_parse: increase to speed higher-latency fetches, but watch CPU and memory. Default: 20.
- rate_limiter: global requests-per-second. Lower this to be more polite. Default: 50.
- For politeness: consider reducing rate_limiter per-host or adding per-host rate limits.

Troubleshooting
- sqlite3/FTS5 errors: If creating the FTS5 virtual table fails, your system SQLite may not have FTS5 enabled. Install a SQLite build with FTS5 (for many Linux distributions, libsqlite3-dev contains FTS5). Alternatively use a Rust build that bundles SQLite with FTS enabled.
- Reqwest errors: network failures will be logged; the crawler will skip failed fetches. Check your network and proxies.
- Duplicate or concatenated files in crawled/: Because raw HTML is appended, you may see repeated content if the crawler visits the same path multiple times.

Security considerations
- Fetching arbitrary URLs can be dangerous. Do not run the crawler on untrusted networks or on systems where fetching malicious content could cause harm.
- The program does not sanitize or execute page content — it merely stores and indexes raw HTML. Be careful when opening stored files locally.


Contributing
- Fork the repository and open a pull request for changes.
- If you make changes to database schema or queries, update this README and add tests where possible.

License
- No license is provided in the repository. If you plan to reuse or distribute this code, add a LICENSE file (for example MIT or Apache-2.0) and note it here.

Contact / help
- Open an issue in the repository describing what you tried and include verbose logs (run with RUST_BACKTRACE=1 and examine tracing output) and steps to reproduce.

Enjoy exploring web content with Web Weaver — build, crawl, and search!
