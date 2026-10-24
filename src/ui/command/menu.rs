use super::*;

pub const PLAYLIST_LIMIT: usize = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuId {
    Command(Command),
    Section(Section),
    Action(ItemAction),
    Playlist(usize),
    Sleep(SleepChoice),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SleepChoice {
    Off,
    Timer(SleepTimer),
    Custom,
}

impl SleepChoice {
    pub const ALL: [SleepChoice; 9] = [
        SleepChoice::Off,
        SleepChoice::Timer(SleepTimer::Minutes(15)),
        SleepChoice::Timer(SleepTimer::Minutes(30)),
        SleepChoice::Timer(SleepTimer::Minutes(45)),
        SleepChoice::Timer(SleepTimer::Minutes(60)),
        SleepChoice::Timer(SleepTimer::Minutes(90)),
        SleepChoice::Timer(SleepTimer::EndOfTrack),
        SleepChoice::Timer(SleepTimer::EndOfQueue),
        SleepChoice::Custom,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|choice| *choice == self).unwrap_or_default()
    }

    fn label(self) -> String {
        match self {
            SleepChoice::Off => fl!("sleep-off"),
            SleepChoice::Timer(SleepTimer::Minutes(minutes)) => fl!("sleep-minutes", minutes = minutes),
            SleepChoice::Timer(SleepTimer::EndOfTrack) => fl!("sleep-end-of-track"),
            SleepChoice::Timer(SleepTimer::EndOfQueue) => fl!("sleep-end-of-queue"),
            SleepChoice::Custom => fl!("sleep-custom"),
        }
    }
}

impl MenuId {
    pub fn id(self) -> Id {
        match self {
            MenuId::Command(command) => command.id(),
            MenuId::Section(section) => SECTION_BASE + section.index() as Id,
            MenuId::Action(action) => ACTION_BASE + action.index() as Id,
            MenuId::Playlist(index) => PLAYLIST_BASE + index as Id,
            MenuId::Sleep(choice) => SLEEP_BASE + choice.index() as Id,
        }
    }

    pub fn from_id(id: Id) -> Option<MenuId> {
        let offset = |base: Id| usize::try_from(id - base).ok();
        match id {
            id if id >= PLAYLIST_BASE => {
                offset(PLAYLIST_BASE).filter(|index| *index < PLAYLIST_LIMIT).map(MenuId::Playlist)
            }
            id if id >= ACTION_BASE => {
                offset(ACTION_BASE).and_then(|index| ItemAction::ALL.get(index)).copied().map(MenuId::Action)
            }
            id if id >= SLEEP_BASE => {
                offset(SLEEP_BASE).and_then(|index| SleepChoice::ALL.get(index)).copied().map(MenuId::Sleep)
            }
            id if id >= SECTION_BASE => {
                offset(SECTION_BASE).and_then(|index| Section::ALL.get(index)).copied().map(MenuId::Section)
            }
            id if id >= COMMAND_BASE => {
                offset(COMMAND_BASE).and_then(|index| Command::ALL.get(index)).copied().map(MenuId::Command)
            }
            _ => None,
        }
    }
}

pub fn escape(text: &str) -> String {
    text.replace('&', "&&")
}

fn entry(label: String, key: Option<String>) -> String {
    match key {
        Some(key) => format!("{label}\t{key}"),
        None => label,
    }
}

pub fn menu_bar(keys: &Keymap) -> MenuBar {
    use Command::*;
    let menu = |commands: &[Option<Command>]| {
        let menu = Menu::builder().build();
        for command in commands {
            match command {
                Some(command) => {
                    let kind = if *command == Shuffle { ItemKind::Check } else { ItemKind::Normal };
                    let text = entry(command.label(), keys.key(Binding::Command(*command)));
                    menu.append(command.id(), &text, "", kind);
                }
                None => {
                    menu.append_separator();
                }
            }
        }
        menu
    };
    let file =
        menu(&[Some(NewPlaylist), None, Some(Settings), Some(PromoCode), Some(SignOut), None, Some(Hide), Some(Quit)]);
    let edit = menu(&[Some(Undo), None, Some(SelectAll), Some(FocusFilter), None, Some(Find), Some(Listen)]);
    let playback = menu(&[
        Some(PlayPause),
        Some(Stop),
        Some(Next),
        Some(Previous),
        None,
        Some(SeekForward),
        Some(SeekBack),
        Some(VolumeUp),
        Some(VolumeDown),
        None,
        Some(SpeedUp),
        Some(SpeedDown),
        Some(SpeedReset),
        None,
        Some(Shuffle),
        Some(Repeat),
        None,
        Some(Devices),
        None,
        Some(MoveUp),
        Some(MoveDown),
    ]);
    let sleep = Menu::builder().build();
    for choice in SleepChoice::ALL {
        sleep.append(MenuId::Sleep(choice).id(), &choice.label(), "", ItemKind::Normal);
    }
    playback.append_separator();
    playback.append_submenu(sleep, &fl!("menu-sleep"), "");
    let track = menu(&[
        Some(NowPlaying),
        Some(SpeakNowPlaying),
        Some(TrackInfo),
        None,
        Some(Like),
        Some(Dislike),
        Some(AddToPlaylist),
        Some(Download),
        None,
        Some(Wave),
        Some(Similar),
        Some(GoToAlbum),
        Some(GoToArtist),
        Some(CopyLink),
    ]);
    let go = Menu::builder().build();
    for section in Section::ALL {
        go.append(
            MenuId::Section(section).id(),
            &entry(escape(&section.label()), keys.key(Binding::Section(section))),
            "",
            ItemKind::Normal,
        );
    }
    go.append_separator();
    for command in [Back, Forward, Refresh, NextPane, FocusSearch, OpenUser] {
        go.append(command.id(), &entry(command.label(), keys.key(Binding::Command(command))), "", ItemKind::Normal);
    }
    let help = menu(&[
        Some(Shortcuts),
        Some(VoiceHelp),
        None,
        Some(OpenDataFolder),
        Some(OpenLog),
        None,
        Some(CheckUpdates),
        Some(About),
    ]);
    MenuBar::builder()
        .append(file, &fl!("menu-file"))
        .append(edit, &fl!("menu-edit"))
        .append(playback, &fl!("menu-playback"))
        .append(track, &fl!("menu-track"))
        .append(go, &fl!("menu-go"))
        .append(help, &fl!("menu-help"))
        .build()
}

pub(super) fn plain(label: &str) -> String {
    label.replace("&&", "\0").replace('&', "").replace('\0', "&").trim_end_matches('…').to_owned()
}

pub fn shortcuts(keys: &Keymap, global: &[(GlobalAction, String)]) -> String {
    let line = |label: &str, key: &str| format!("{}: {key}", plain(label));
    let mut lines: Vec<String> =
        Binding::all().into_iter().filter_map(|binding| Some(line(&binding.label(), &keys.key(binding)?))).collect();
    if !global.is_empty() {
        lines.push(String::new());
        lines.push(fl!("shortcuts-global"));
        lines.extend(global.iter().map(|(action, key)| line(&Command::from_global(*action).label(), key)));
    }
    lines.join("\r\n")
}

pub fn searchable(keys: &Keymap) -> Vec<(String, MenuId)> {
    let sleep = plain(&fl!("menu-sleep"));
    Binding::all()
        .into_iter()
        .map(|binding| {
            let label = match keys.key(binding) {
                Some(key) => format!("{} ({key})", binding.label()),
                None => binding.label(),
            };
            (label, binding.menu_id())
        })
        .chain(
            SleepChoice::ALL.into_iter().map(|choice| (format!("{sleep}: {}", choice.label()), MenuId::Sleep(choice))),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_ids_round_trip() {
        for command in Command::ALL {
            assert_eq!(MenuId::from_id(command.id()), Some(MenuId::Command(command)));
        }
        for section in Section::ALL {
            assert_eq!(MenuId::from_id(MenuId::Section(section).id()), Some(MenuId::Section(section)));
        }
        for action in ItemAction::ALL {
            assert_eq!(MenuId::from_id(MenuId::Action(action).id()), Some(MenuId::Action(action)));
        }
        assert_eq!(MenuId::from_id(MenuId::Playlist(5).id()), Some(MenuId::Playlist(5)));
        for choice in SleepChoice::ALL {
            assert_eq!(MenuId::from_id(MenuId::Sleep(choice).id()), Some(MenuId::Sleep(choice)));
        }
        assert!(SLEEP_BASE + (SleepChoice::ALL.len() as Id) < ACTION_BASE);
        assert_eq!(MenuId::from_id(5), None);
        assert!(Command::ALL.len() < (SECTION_BASE - COMMAND_BASE) as usize);
    }

    #[test]
    fn shortcuts_list_every_accelerator() {
        let text = shortcuts(&Keymap::default(), &[(GlobalAction::PlayPause, "Ctrl+Alt+Shift+P".into())]);
        assert!(text.contains("F1") && text.contains("Ctrl+Alt+Shift+P"));
        assert!(!text.contains('&'));
        assert_eq!(plain("Rock && &roll…"), "Rock & roll");
    }
}
