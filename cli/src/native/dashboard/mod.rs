mod api;
mod assets;
mod commands;
mod control;
mod details;
mod follow;
mod guard;
mod open;
mod screenshots;
mod stream;
mod timeline;

use std::io::ErrorKind;
use std::net::{Ipv4Addr, Ipv6Addr, TcpListener};
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::{get, post};
use tokio::sync::{oneshot, watch};

use crate::native::control::Hold;
use crate::native::store::SessionFolder;
use crate::native::terminal::Terminal;
use crate::protocol::{Dashboard, ErrorBody, HumanInput, Reply, Request, Sessions, TabId};

pub trait Backend: Send + Sync + 'static {
    fn call(&self, request: Request) -> Result<Reply, ErrorBody>;
    fn sessions(&self) -> Sessions;
    fn terminal(&self, session: &str, tab: TabId) -> Option<Arc<Terminal>>;
    fn folder(&self, session: &str) -> Option<Arc<SessionFolder>>;
    fn changes(&self) -> watch::Receiver<u64>;
    fn take_control(&self, session: &str, tab: TabId) -> Result<Hold, ErrorBody>;
    fn holds(&self, session: &str, tab: TabId, hold: &Hold) -> bool;
    fn type_as_human(&self, session: &str, tab: TabId, hold: &Hold, input: &HumanInput) -> Result<(), ErrorBody>;
}

pub struct Server {
    port: u16,
    origins: Vec<String>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(port: u16, origins: Vec<String>, backend: Arc<dyn Backend>) -> Result<Self> {
        let v4 = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|e| refused(port, e))?;
        let port = v4.local_addr().context("read the dashboard's port")?.port();
        let v6 = TcpListener::bind((Ipv6Addr::LOCALHOST, port))
            .inspect_err(|e| eprintln!("dashboard: listening on [::1]:{port} failed, serving 127.0.0.1 only: {e}"))
            .ok();
        for listener in std::iter::once(&v4).chain(&v6) {
            listener.set_nonblocking(true).context("make the dashboard's socket non-blocking")?;
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .context("start the dashboard's runtime")?;
        let app = router(guard::Pages { port, origins: origins.clone() }, backend);
        let (stop, stopped) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("dashboard".into())
            .spawn(move || {
                runtime.block_on(serve(app, v4, v6, stopped));
                runtime.shutdown_background();
            })
            .context("start the dashboard's thread")?;
        Ok(Self { port, origins, stop: Some(stop), thread: Some(thread) })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn allows(&self, origin: &str) -> bool {
        self.origins.iter().any(|allowed| allowed == origin)
    }

    pub fn info(&self) -> Dashboard {
        Dashboard { url: format!("http://localhost:{}", self.port), port: self.port }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take()
            && stop.send(()).is_err()
        {
            eprintln!("dashboard: the server had already stopped");
        }
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            eprintln!("dashboard: the server thread panicked");
        }
    }
}

fn refused(port: u16, error: std::io::Error) -> anyhow::Error {
    if error.kind() == ErrorKind::AddrInUse {
        anyhow::anyhow!(
            "port {port} is in use by another program; pick another with `agent-terminal dashboard start --port <n>`"
        )
    } else {
        anyhow::Error::new(error).context(format!("listen on 127.0.0.1:{port}"))
    }
}

async fn serve(app: Router, v4: TcpListener, v6: Option<TcpListener>, stopped: oneshot::Receiver<()>) {
    let listen = |listener: TcpListener| {
        let app = app.clone();
        async move {
            let address = listener.local_addr().map(|a| a.to_string()).unwrap_or_else(|e| e.to_string());
            let served = match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => axum::serve(listener, app).await,
                Err(e) => Err(e),
            };
            if let Err(e) = served {
                eprintln!("dashboard: serving {address} failed: {e}");
            }
        }
    };
    let v6 = async {
        match v6 {
            Some(listener) => listen(listener).await,
            None => std::future::pending().await,
        }
    };
    tokio::select! {
        () = listen(v4) => {}
        () = v6 => {}
        _ = stopped => {}
    }
}

fn router(pages: guard::Pages, backend: Arc<dyn Backend>) -> Router {
    Router::new()
        .route("/api/sessions", get(api::sessions).post(open::open))
        .route("/api/sessions/stream", get(api::sessions_stream))
        .route("/api/sessions/close", post(api::close_all))
        .route("/api/session/{session}/{tab}/stream", get(stream::tab))
        .route("/api/session/{session}/{tab}/timeline", get(timeline::tab))
        .route("/api/session/{session}/{tab}/activity", get(details::activity))
        .route("/api/session/{session}/{tab}/output", get(details::output))
        .route("/api/session/{session}/{tab}/info", get(details::info))
        .route("/api/session/{session}/screenshots/{path}", get(details::screenshot))
        .route("/api/session/{session}/screenshots/stream", get(screenshots::stream))
        .route("/api/session/{session}/screenshots/compare", post(screenshots::compare))
        .route("/api/session/{session}/{tab}/restart", post(api::restart))
        .route("/api/session/{session}/{tab}/close", post(api::close))
        .route("/api/session/{session}/{tab}/screenshot", post(api::screenshot))
        .route("/api/session/{session}/{tab}/control", get(control::take))
        .route("/api/session/{session}/{tab}/commands/stream", get(commands::stream))
        .route("/api/session/{session}/{tab}/commands/{id}", get(commands::output))
        .fallback(assets::serve)
        .with_state(backend)
        .layer(axum::middleware::from_fn_with_state(Arc::new(pages), guard::same_origin))
}

fn tab_id(tab: &str) -> Option<TabId> {
    tab.parse().ok()
}
