use std::net::SocketAddr;
use std::path::Path;

use anyhow::{Context, Result};
use axum::Router;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

pub async fn serve(bind: String, port: u16, root: String) -> Result<()> {
    let root = Path::new(&root);
    if !root.is_dir() {
        anyhow::bail!("static root is not a directory: {}", root.display());
    }
    let service = ServeDir::new(root).append_index_html_on_directories(true);
    let app = Router::new().fallback_service(service);
    let addr: SocketAddr = format!("{bind}:{port}")
        .parse()
        .with_context(|| format!("invalid bind address {bind}:{port}"))?;
    let listener = TcpListener::bind(addr).await?;
    eprintln!("hive-web listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
