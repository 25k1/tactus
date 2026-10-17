use yandex_music::error::ClientError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Network(#[from] reqwest::Error),
    #[error("{0}")]
    Client(#[from] ClientError),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{status}: {message}")]
    Status { status: u16, message: String },
    #[error("{0} was not found")]
    NotFound(String),
    #[error("authorization is pending")]
    AuthorizationPending,
    #[error("the sign-in code expired")]
    Expired,
    #[error("not signed in")]
    SignedOut,
    #[error("{0}")]
    Audio(String),
    #[error("{0}")]
    Api(String),
    #[error("this is not supported")]
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    Network,
    SignedOut,
    Expired,
    NotFound,
    Subscription,
    Region,
    RateLimited,
    Server,
    Audio,
    Storage,
    Unsupported,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub kind: ErrorKind,
    pub detail: String,
}

fn status_kind(status: u16) -> ErrorKind {
    match status {
        401 => ErrorKind::SignedOut,
        402 | 403 => ErrorKind::Subscription,
        404 => ErrorKind::NotFound,
        429 => ErrorKind::RateLimited,
        451 => ErrorKind::Region,
        500.. => ErrorKind::Server,
        _ => ErrorKind::Other,
    }
}

fn network_kind(error: &reqwest::Error) -> ErrorKind {
    error.status().map_or(ErrorKind::Network, |status| status_kind(status.as_u16()))
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Network(error) | Error::Client(ClientError::RequestError { error }) => network_kind(error),
            Error::Client(ClientError::YandexMusicError { error }) if error.name.contains("not-found") => {
                ErrorKind::NotFound
            }
            Error::Status { status, .. } => status_kind(*status),
            Error::NotFound(_) => ErrorKind::NotFound,
            Error::AuthorizationPending | Error::SignedOut => ErrorKind::SignedOut,
            Error::Expired => ErrorKind::Expired,
            Error::Audio(_) => ErrorKind::Audio,
            Error::Io(_) => ErrorKind::Storage,
            Error::Unsupported => ErrorKind::Unsupported,
            Error::Client(_) | Error::Json(_) | Error::Api(_) => ErrorKind::Other,
        }
    }

    pub fn failure(&self) -> Failure {
        Failure { kind: self.kind(), detail: self.to_string() }
    }
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        error.failure()
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_http_statuses() {
        let kind = |status| Error::Status { status, message: String::new() }.kind();
        assert_eq!(kind(401), ErrorKind::SignedOut);
        assert_eq!(kind(403), ErrorKind::Subscription);
        assert_eq!(kind(404), ErrorKind::NotFound);
        assert_eq!(kind(429), ErrorKind::RateLimited);
        assert_eq!(kind(451), ErrorKind::Region);
        assert_eq!(kind(503), ErrorKind::Server);
        assert_eq!(kind(400), ErrorKind::Other);
    }

    #[test]
    fn failures_keep_the_detail() {
        let failure = Error::NotFound("album 5".into()).failure();
        assert_eq!(failure, Failure { kind: ErrorKind::NotFound, detail: "album 5 was not found".into() });
        assert_eq!(Error::Expired.kind(), ErrorKind::Expired);
    }
}
