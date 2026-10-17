#[derive(Clone, Debug, Default, PartialEq)]
pub struct Account {
    pub uid: u64,
    pub login: String,
    pub name: String,
    pub has_plus: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccountOption {
    ExplicitForbidden,
    ChildMode,
    AutoPlayRadio,
    PublicCollection,
    AddToPlaylistTop,
    SyncQueue,
    ReduceAiContent,
}

impl AccountOption {
    pub const ALL: [AccountOption; 7] = [
        AccountOption::ExplicitForbidden,
        AccountOption::ChildMode,
        AccountOption::AutoPlayRadio,
        AccountOption::PublicCollection,
        AccountOption::AddToPlaylistTop,
        AccountOption::SyncQueue,
        AccountOption::ReduceAiContent,
    ];

    pub fn key(self) -> &'static str {
        match self {
            AccountOption::ExplicitForbidden => "explicitForbidden",
            AccountOption::ChildMode => "childModEnabled",
            AccountOption::AutoPlayRadio => "autoPlayRadio",
            AccountOption::PublicCollection => "userMusicVisibility",
            AccountOption::AddToPlaylistTop => "addNewTrackOnPlaylistTop",
            AccountOption::SyncQueue => "syncQueueEnabled",
            AccountOption::ReduceAiContent => "aiContentReductionEnabled",
        }
    }

    pub fn encode(self, enabled: bool) -> &'static str {
        match (self, enabled) {
            (AccountOption::PublicCollection, true) => "PUBLIC",
            (AccountOption::PublicCollection, false) => "PRIVATE",
            (_, true) => "true",
            (_, false) => "false",
        }
    }
}

pub type AccountSettings = Vec<(AccountOption, bool)>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    pub expires_in: u64,
    pub interval: u64,
}
