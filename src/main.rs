mod db;
mod hook;
mod mcp;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "stow", about = "Local FTS5 search-and-store for large tool output")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as an MCP server (stdio JSON-RPC)
    Mcp,
    /// Search stored captures
    Search {
        query: String,
        #[arg(short, long, default_value_t = 10)]
        limit: i64,
    },
    /// Show the full content of a capture by ID
    Show { id: i64 },
    /// Store stdin as a capture (for piping command output)
    Store {
        #[arg(short, long, default_value = "stdin")]
        source: String,
        #[arg(short, long, default_value = "bash")]
        tool: String,
    },
    /// Hook entry points
    Hook {
        #[command(subcommand)]
        event: HookEvent,
    },
}

#[derive(Subcommand)]
enum HookEvent {
    /// SessionStart hook: inject one-time guidance (non-blocking, adds context only)
    Sessionstart,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Mcp => mcp::run()?,
        Commands::Search { query, limit } => {
            let conn = db::open()?;
            let results = db::search(&conn, &query, limit)?;
            if results.is_empty() {
                println!("No matches found.");
            } else {
                for c in results {
                    println!("#{} [{}, {} bytes, {}] {}: {}", c.id, c.tool, c.byte_len, c.created_at, c.source, c.snippet);
                }
            }
        }
        Commands::Show { id } => {
            let conn = db::open()?;
            match db::show(&conn, id)? {
                Some((content, source, tool, created_at)) => {
                    println!("[#{} — {} ({}), captured {}]\n", id, source, tool, created_at);
                    print!("{}", content);
                }
                None => println!("No capture found with id {}", id),
            }
        }
        Commands::Store { source, tool } => {
            let mut content = String::new();
            io::stdin_wrapper(&mut content)?;
            let conn = db::open()?;
            let id = db::insert(&conn, &content, &source, &tool)?;
            mcp::breadcrumb(id, &source, &tool, &content);
            println!("[stowed #{} — {} bytes from {} ({})]", id, content.len(), source, tool);
        }
        Commands::Hook { event } => match event {
            HookEvent::Sessionstart => hook::session_start()?,
        },
    }
    Ok(())
}

mod io {
    use anyhow::Result;
    use std::io::Read;
    pub fn stdin_wrapper(buf: &mut String) -> Result<()> {
        std::io::stdin().read_to_string(buf)?;
        Ok(())
    }
}
