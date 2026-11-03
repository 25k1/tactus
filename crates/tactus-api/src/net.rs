use crate::proxy::{self, Proxy};
use crate::sync::Guard;
use reqwest::header::{ACCEPT_LANGUAGE, HeaderMap, HeaderValue};
use std::future::Future;
use std::net::TcpStream;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::Duration;

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("tactus-net")
        .enable_all()
        .build()
        .expect("network runtime")
});

static LANGUAGE: OnceLock<String> = OnceLock::new();
static PROXY: OnceLock<Proxy> = OnceLock::new();

pub fn set_language(language: &str) {
    let _ = LANGUAGE.set(language.to_owned());
}

pub fn language() -> &'static str {
    LANGUAGE.get().map_or("en", String::as_str)
}

pub fn set_proxy(proxy: &Proxy) {
    let _ = PROXY.set(proxy.clone());
}

fn active_proxy() -> Option<&'static Proxy> {
    PROXY.get().filter(|proxy| proxy.active())
}

pub fn open_stream(host: &str, port: u16) -> std::io::Result<TcpStream> {
    match active_proxy() {
        Some(proxy) => proxy.tunnel(host, port),
        None => proxy::connect(host, port),
    }
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    RUNTIME.handle().block_on(future)
}

pub fn spawn<F: Future<Output = ()> + Send + 'static>(future: F) {
    RUNTIME.spawn(future);
}

pub async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

pub fn spawn_blocking(task: impl FnOnce() + Send + 'static) {
    RUNTIME.spawn_blocking(task);
}

fn route(account: Option<&Proxy>) -> Option<&Proxy> {
    match account {
        Some(proxy) => proxy.active().then_some(proxy),
        None => active_proxy(),
    }
}

pub fn builder(mut headers: HeaderMap, account: Option<&Proxy>) -> reqwest::ClientBuilder {
    if let Ok(language) = HeaderValue::from_str(language()) {
        headers.insert(ACCEPT_LANGUAGE, language);
    }
    let builder = reqwest::Client::builder()
        .user_agent(concat!("Tactus/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(20))
        .tcp_keepalive(Duration::from_secs(30))
        .default_headers(headers);
    match route(account).and_then(|proxy| reqwest::Proxy::all(proxy.url()).ok()) {
        Some(proxy) => builder.proxy(proxy),
        None => builder,
    }
}

pub fn client() -> &'static reqwest::Client {
    static CLIENT: LazyLock<reqwest::Client> =
        LazyLock::new(|| builder(HeaderMap::new(), None).build().unwrap_or_default());
    &CLIENT
}

pub fn client_for(account: Option<&Proxy>) -> reqwest::Client {
    static CLIENTS: Mutex<Vec<(Proxy, reqwest::Client)>> = Mutex::new(Vec::new());
    let Some(proxy) = account else {
        return client().clone();
    };
    let mut clients = CLIENTS.guard();
    if let Some((_, client)) = clients.iter().find(|(known, _)| known == proxy) {
        return client.clone();
    }
    let client = builder(HeaderMap::new(), Some(proxy)).build().unwrap_or_default();
    clients.push((proxy.clone(), client.clone()));
    client
}
