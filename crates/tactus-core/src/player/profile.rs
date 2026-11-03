use super::*;
use crate::accounts::AccountEntry;

pub struct Profile {
    pub entry: AccountEntry,
    pub account: Option<Account>,
    pub account_settings: AccountSettings,
    pub library: Library,
    pub playlists: Vec<Playlist>,
    pub pages: HashMap<View, PageState>,
    pub disconnected: bool,
    pub session: Session,
    pub(super) provider: SharedProvider,
    pub(super) service: Option<Service>,
    pub(super) last_connect: Option<Instant>,
    pub(super) shelf: Queue,
}

impl Profile {
    pub fn new(entry: AccountEntry, provider: SharedProvider) -> Self {
        Self {
            entry,
            account: None,
            account_settings: AccountSettings::new(),
            library: Library::default(),
            playlists: Vec::new(),
            pages: HashMap::new(),
            disconnected: false,
            session: Session::default(),
            provider,
            service: None,
            last_connect: None,
            shelf: Queue::default(),
        }
    }

    pub fn id(&self) -> &str {
        &self.entry.id
    }

    pub fn kind(&self) -> ProviderKind {
        self.provider.kind()
    }

    pub fn capabilities(&self) -> Capabilities {
        self.provider.capabilities()
    }

    pub fn is_signed_in(&self) -> bool {
        self.service.is_some()
    }

    pub fn label(&self) -> String {
        let name = self.account.as_ref().map(|account| account.name.as_str()).filter(|name| !name.is_empty());
        match (self.entry.alias.trim(), name) {
            ("", Some(name)) => format!("{} — {name}", self.kind().name()),
            ("", None) => self.kind().name().to_owned(),
            (alias, _) => format!("{} — {alias}", self.kind().name()),
        }
    }

    pub(super) fn load_session(&mut self) {
        self.session = Session::load(&self.entry);
        self.shelf.replace(self.session.tracks.clone(), self.session.index);
    }
}
