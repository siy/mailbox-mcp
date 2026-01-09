use clap::Parser;
use mailbox_mcp::{Database, MailboxServer};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(unix)]
const INSTALL_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/siy/mailbox-mcp/master/scripts/install.sh";
#[cfg(windows)]
const INSTALL_SCRIPT_URL_WINDOWS: &str =
    "https://raw.githubusercontent.com/siy/mailbox-mcp/master/scripts/install.ps1";

#[derive(Parser)]
#[command(name = "mailbox-mcp")]
#[command(about = "A minimalistic MCP server for agent-to-agent communication")]
#[command(version)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "3000")]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Upgrade to the latest version
    #[arg(long)]
    upgrade: bool,
}

fn upgrade() -> anyhow::Result<()> {
    println!("Upgrading mailbox-mcp to the latest version...");

    #[cfg(unix)]
    {
        use std::process::Command;
        let status = Command::new("sh")
            .arg("-c")
            .arg(format!("curl -fsSL {} | sh", INSTALL_SCRIPT_URL))
            .status()?;

        if !status.success() {
            anyhow::bail!("Upgrade failed");
        }
    }

    #[cfg(windows)]
    {
        use std::process::Command;
        let status = Command::new("powershell")
            .arg("-Command")
            .arg(format!("iwr -useb {} | iex", INSTALL_SCRIPT_URL_WINDOWS))
            .status()?;

        if !status.success() {
            anyhow::bail!("Upgrade failed");
        }
    }

    println!("Upgrade complete. Please restart the server.");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.upgrade {
        return upgrade();
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db = Database::new()?;
    let server = MailboxServer::new(db);

    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let app = axum::Router::new().nest_service("/mcp", service);
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Mailbox MCP server listening on http://{}/mcp", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
