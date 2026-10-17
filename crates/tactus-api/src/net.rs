use reqwest::header::{ACCEPT_LANGUAGE, HeaderMap, HeaderValue};
use std::future::Future;
use std::sync::{LazyLock, OnceLock};
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

pub fn set_language(language: &str) {
    let _ = LANGUAGE.set(language.to_owned());
}

pub fn language() -> &'static str {
    LANGUAGE.get().map_or("en", String::as_str)
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    RUNTIME.handle().block_on(future)
}

pub fn spawn<F: Future<Output = ()> + Send + 'static>(future: F) {
    RUNTIME.spawn(future);
}

pub fn spawn_blocking(task: impl FnOnce() + Send + 'static) {
    RUNTIME.spawn_blocking(task);
}

pub fn builder(mut headers: HeaderMap) -> reqwest::ClientBuilder {
    if let Ok(language) = HeaderValue::from_str(language()) {
        headers.insert(ACCEPT_LANGUAGE, language);
    }
    reqwest::Client::builder()
        .user_agent(concat!("Tactus/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .default_headers(headers)
}

pub fn client() -> &'static reqwest::Client {
    static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| builder(HeaderMap::new()).build().unwrap_or_default());
    &CLIENT
}
