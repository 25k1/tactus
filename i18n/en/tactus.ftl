language-name = English

tracks-count = { $count ->
    [one] { $count } track
   *[other] { $count } tracks
}

nav-search = Search
nav-wave = My Wave
nav-for-you = For You
nav-collection = Collection
nav-playlists = Playlists
nav-radio = Radio
nav-podcasts = Podcasts and books
nav-history = History
nav-queue = Queue
nav-lyrics = Lyrics
nav-downloads = Downloads
sidebar-name = Sections

login-title = Sign in to Yandex Music
login-description = Tactus signs in with a one-time code. Get a code, open the verification page, and enter the code there.
login-get-code = &Get verification code
login-requesting = Requesting a code…
login-instruction = Enter this code on the verification page. Tactus signs in automatically once you confirm.
login-code = Verification code
login-copy-code = &Copy code
login-open-page = &Open verification page
login-code-copied = Code copied

menu-file = &File
menu-playback = &Playback
menu-track = &Track
menu-go = &Go
menu-new-playlist = &New playlist…
menu-settings = &Settings…
menu-promo-code = &Activate promo code…
menu-sign-out = Sign &out
menu-hide = &Minimize to tray
menu-quit = &Quit
menu-play-pause = &Play or pause
menu-stop = &Stop
menu-next = &Next track
menu-previous = P&revious track
menu-forward = &Forward 10 seconds
menu-back = &Back 10 seconds
menu-volume-up = Volume &up
menu-volume-down = Volume &down
menu-speed-up = Speed u&p
menu-speed-down = Speed do&wn
menu-speed-reset = &Normal speed
menu-shuffle = S&huffle
menu-repeat = &Repeat: off, all, one
menu-now-playing = &What's playing
menu-speak-now-playing = &Speak what's playing
menu-track-info = Track &information
menu-like = &Like or unlike
menu-dislike = &Dislike
menu-add-to-playlist = &Add to playlist…
menu-download = Do&wnload
menu-radio = My &Wave from this track
menu-similar = Si&milar tracks
menu-go-to-album = Go to al&bum
menu-go-to-artist = Go to ar&tist
menu-copy-link = &Copy link
menu-move-up = Move up in the &queue or playlist
menu-move-down = Move down in the q&ueue or playlist
menu-go-back = &Back
menu-go-forward = &Forward
menu-refresh = &Refresh
menu-next-pane = Next &pane
menu-focus-search = &Search box

search-label = Search
search-button = &Search
search-hint = Type what you are looking for and press Enter.

page-description = Description
page-show = Sh&ow
page-list = Items
page-loading = { $title }: loading…
page-failed = { $title }: could not load. { $error }
page-empty = { $title }: nothing here yet.

heading-album = { $title } by { $artist }
heading-artist = { $name }
heading-playlist = { $title } by { $owner }, { $count ->
    [one] { $count } track
   *[other] { $count } tracks
}

section-all = All results
section-tracks = Tracks
section-albums = Albums
section-appears-on = Appears on
section-artists = Artists
section-similar-artists = Similar artists
section-playlists = Playlists
section-podcasts = Podcasts and books
section-episodes = Episodes
section-suggestions = Suggested tracks
section-disliked-tracks = Disliked tracks
section-disliked-artists = Disliked artists

station-genres = Genres
station-moods = Moods
station-activities = Activities
station-eras = Eras
station-local = Local

column-title = Title
column-artist = Artist
column-details = Details
column-duration = Duration
column-action = Action
column-shortcut = Shortcut

kind-album = Album
kind-single = Single
kind-podcast = Podcast
kind-audiobook = Audiobook
kind-artist = Artist
kind-playlist = Playlist
kind-station = Radio
kind-collection = Collection

queue-current = { $title } (playing)

button-play = &Play
button-shuffle = S&huffle
button-radio = &Radio
button-like = &Like
button-unlike = Un&like
button-download = &Download
button-refresh = R&efresh
button-clear-queue = &Clear upcoming
button-save-queue = &Save as playlist
button-open-folder = &Open folder
button-clear-downloads = &Clear finished

action-open = Open
action-play = Play
action-play-next = Play next
action-add-to-queue = Add to queue
action-like = Like
action-unlike = Unlike
action-dislike = Dislike
action-undislike = Remove dislike
action-add-to-playlist = Add to playlist
action-add-to-this-playlist = Add to this playlist
action-remove-from-playlist = Remove from playlist
action-remove-from-queue = Remove from queue	Delete
action-move-up = Move up	Alt+Up
action-move-down = Move down	Alt+Down
action-download = Download
action-radio = Start radio
action-similar = Similar tracks
action-go-to-album = Go to album
action-go-to-artist = Go to artist
action-track-info = Track information
action-copy-link = Copy link
action-rename = Rename…
action-make-public = Make public
action-make-private = Make private
action-delete = Delete…
action-open-file = Open file
action-show-file = Show in folder

wave-character = Character
wave-mood = Mood
wave-language = Language
wave-any = Any
wave-favorite = Favorite
wave-discover = Unfamiliar
wave-popular = Popular
wave-active = Energetic
wave-fun = Cheerful
wave-calm = Calm
wave-sad = Sad
wave-russian = Russian
wave-foreign = Foreign
wave-instrumental = Without words
wave-start = Start &My Wave

player-previous = Pre&vious
player-play = Pl&ay
player-pause = P&ause
player-next = Ne&xt
player-position = Position
player-volume = Volume
player-repeat = Repeat
player-shuffle = Shu&ffle
repeat-off = Off
repeat-one = One track
repeat-all = All tracks
now-playing-nothing = Nothing is playing
state-playing = Playing
state-paused = Paused

lyrics-empty = Lyrics appear here for the playing track.
lyrics-unavailable = No lyrics for { $title }.
lyrics-synced = Lyrics for { $title }. The current line follows playback; press Enter on a line to jump to it.
lyrics-plain = Lyrics for { $title }.

info-title = Title
info-version = Version
info-artists = Artists
info-album = Album
info-album-type = Album type
info-year = Year
info-release-date = Release date
info-genre = Genre
info-duration = Duration
info-labels = Labels
info-explicit = Explicit
info-lyrics = Lyrics
info-lyrics-synced = Synchronized
info-lyrics-text = Text only
info-link = Link
info-yes = Yes
info-no = No

download-queued = Queued
download-progress = Downloading, { $percent }%
download-finished = Downloaded
download-failed = Failed: { $error }

announce-volume = Volume { $percent }%
announce-speed = Speed { $speed }
announce-shuffle-on = shuffle on
announce-shuffle-off = shuffle off

notice-liked = Liked
notice-unliked = Like removed
notice-disliked = Disliked
notice-undisliked = Dislike removed
notice-playlist-created = Playlist { $title } created
notice-playlist-renamed = Playlist renamed
notice-playlist-deleted = Playlist deleted
notice-playlist-public = The playlist is now public
notice-playlist-private = The playlist is now private
notice-added-to-playlist = { $count ->
    [one] Added to { $title }
   *[other] { $count } tracks added to { $title }
}
notice-removed-from-playlist = { $count ->
    [one] Removed from the playlist
   *[other] { $count } tracks removed from the playlist
}
notice-play-next = { $count ->
    [one] Plays next
   *[other] { $count } tracks play next
}
notice-queued = { $count ->
    [one] Added to the queue
   *[other] { $count } tracks added to the queue
}
notice-removed-from-queue = { $count ->
    [one] Removed from the queue
   *[other] { $count } tracks removed from the queue
}
notice-queue-cleared = Upcoming tracks cleared
notice-download-queued = { $count ->
    [one] Download started
   *[other] { $count } downloads started
}
notice-downloaded = Downloaded { $title }
notice-download-failed = Could not download { $title }: { $error }
notice-wave-started = Radio started
notice-search-results = { $count ->
    [0] Nothing found
    [one] 1 result
   *[other] { $count } results
}
notice-account-updated = Account settings saved
notice-link-copied = Link copied

playlist-new-prompt = Playlist name:
playlist-rename-prompt = New playlist name:
playlist-delete-confirm = Delete the playlist { $title }?
playlist-choose-prompt = Add { $title } to:
promo-code-prompt = Promo code:
sign-out-confirm = Sign out of Yandex Music?

error-network = Could not connect to Yandex Music. Check your internet connection.
error-signed-out = You are signed out. Sign in to Yandex Music again.
error-expired = The verification code expired. Get a new one.
error-not-found = This is no longer available on Yandex Music.
error-subscription = This needs a Yandex Plus subscription.
error-region = This is not available in your region.
error-rate-limited = Yandex Music is receiving too many requests. Try again in a minute.
error-server = Yandex Music is having problems. Try again later.
error-audio = The audio could not be played. Check the output device in Settings.
error-storage = A file could not be read or written. Check that the Tactus folder is writable.
error-unsupported = This action is not supported.
error-other = Something went wrong: { $detail }
status-loading = Loading…
status-signed-in = Signed in as { $name }
remote-listening = Remote control API is listening on port { $port }
remote-error = Remote control API could not start: { $error }
hotkeys-failed = These global hotkeys are taken by another program: { $keys }

tray-play = Play
tray-pause = Pause
tray-next = Next track
tray-previous = Previous track
tray-like = Like or unlike
tray-show = Show Tactus
tray-hide = Hide Tactus
tray-quit = Quit

dialog-ok = OK
dialog-cancel = Cancel
dialog-close = &Close

settings-title = Settings
settings-general = General
settings-playback = Playback
settings-speech = Speech
settings-hotkeys = Hotkeys
settings-downloads = Downloads
settings-remote = Remote control
settings-account = Account
settings-language = &Language
settings-language-hint = A new language takes effect after restarting Tactus.
settings-language-restart = Restart Tactus to switch the language.
settings-tray = Keep Tactus in the &tray when the window is closed
settings-minimize-to-tray = &Minimize to the tray
settings-start-minimized = Start minimi&zed
settings-autostart = Start with &Windows
settings-resume = &Resume the last queue and position on start
settings-autoplay = Start &playing on start
settings-media-controls = Windows &media controls and media keys
settings-stream-quality = Streaming &quality
settings-output-device = &Output device
settings-default-device = System default
settings-speed = Playback &speed
settings-continue-wave = &Continue with My Wave when the queue ends
settings-sync-queue = &Sync the queue with Yandex Music on other devices
settings-announce = &Announce
settings-track-format = &Track announcement
settings-track-format-hint = Placeholders: {"{"}title{"}"}, {"{"}artist{"}"}, {"{"}album{"}"}, {"{"}year{"}"}, {"{"}genre{"}"}, {"{"}duration{"}"}.
settings-sound-cues = Play &sound cues
settings-speech-engine = Speech goes through { $backend }.
settings-speech-unavailable = No screen reader or speech engine was found.
settings-hotkeys-enabled = &Enable global hotkeys (work in any program)
settings-hotkeys-hint = Write shortcuts like Ctrl+Alt+Shift+P. Global hotkeys need at least one modifier, except function and media keys.
settings-download-folder = Download &folder
settings-download-quality = Download &quality
settings-download-template = File &names
settings-download-template-hint = Use / for folders. Placeholders: {"{"}artist{"}"}, {"{"}album{"}"}, {"{"}title{"}"}, {"{"}number{"}"}, {"{"}year{"}"}, {"{"}genre{"}"}.
settings-embed-cover = Embed &cover art
settings-remote-enabled = &Enable the remote control API (HTTP and WebSocket on this computer)
settings-remote-port = &Port
settings-remote-token-required = Require an &access token
settings-remote-token = Access token
settings-remote-generate = &Generate new token
settings-remote-copy = &Copy token
settings-remote-hint = Send the token as "Authorization: Bearer <token>", or add ?token=<token> to the address.
settings-account-unavailable = Yandex account settings are not available yet. Try again after signing in.

account-explicit-forbidden = Hide explicit content
account-child-mode = Child mode
account-autoplay-radio = Continue with radio when music ends (Yandex apps)
account-public-collection = Make my collection public
account-add-to-top = Add new tracks to the top of playlists
account-sync-queue = Sync the play queue between devices
account-reduce-ai = Play less AI-generated music

announce-kind-track = Track changes
announce-kind-playback = Play and pause
announce-kind-volume = Volume
announce-kind-seek = Seeking
announce-kind-modes = Repeat, shuffle and speed
announce-kind-likes = Likes and dislikes
announce-kind-queue = Queue changes
announce-kind-playlists = Playlists and account
announce-kind-downloads = Downloads
announce-kind-search = Search results
announce-kind-lyrics = Lyrics lines while playing
announce-kind-errors = Errors

hotkey-change = &Change…
hotkey-clear = C&lear
hotkey-reset = &Reset to defaults
hotkey-none = None
hotkey-prompt = Shortcut for { $action }:
hotkey-invalid = { $key } is not a valid global hotkey.
hotkey-show-window = Show or hide Tactus

quality-lossless = Lossless (FLAC) when available
quality-high = High (MP3 320)
quality-low = Low (saves traffic)

menu-sleep = Sleep &timer
sleep-off = &Off
sleep-minutes = { $minutes } minutes
sleep-end-of-track = At the end of this &track
sleep-end-of-queue = At the end of the &queue
sleep-custom = &Custom…
sleep-prompt = Stop playback after how many minutes?
sleep-remaining = Sleep timer: { $time } left
notice-sleep-off = Sleep timer off
notice-sleep-minutes = { $minutes ->
    [one] Playback stops in 1 minute
   *[other] Playback stops in { $minutes } minutes
}
notice-sleep-end-of-track = Playback stops after this track
notice-sleep-end-of-queue = Playback stops at the end of the queue
notice-sleep-soon = Playback stops in one minute
notice-sleep-stopped = Sleep timer stopped playback
settings-crossfade = &Crossfade between tracks, seconds (0 is off)
settings-normalize = &Even out loudness between tracks
settings-equalizer = Equalizer
settings-equalizer-enabled = &Use the equalizer
settings-equalizer-preset = &Preset
settings-equalizer-hint = Changes are heard right away; Cancel restores the previous sound.
band-hz = { $frequency } Hz
band-khz = { $frequency } kHz
preset-custom = Custom
preset-flat = Flat
preset-bass = Bass boost
preset-treble = Treble boost
preset-vocal = Vocal
preset-speech = Speech and podcasts
preset-rock = Rock
preset-pop = Pop
preset-jazz = Jazz
preset-classical = Classical
preset-electronic = Electronic
notice-more-loaded = { $count ->
    [one] 1 more item loaded
   *[other] { $count } more items loaded
}
menu-open-user = &Open a user's playlists…
user-prompt = Yandex login of the user:
heading-user = Playlists of { $name }
heading-track = { $title } by { $artist }
section-similar = Similar tracks
section-versions = Other versions
section-editions = Other editions
section-links = Links
section-charts = Charts
kind-link = Link
kind-chart = Chart
chart-russia = Russia chart
chart-world = World chart
chart-kazakhstan = Kazakhstan chart
chart-belarus = Belarus chart
chart-uzbekistan = Uzbekistan chart
chart-georgia = Georgia chart
action-edit-description = Edit description…
action-duplicate = Save a copy…
playlist-description-prompt = Playlist description:
playlist-copy-title = { $title } (copy)
notice-playlist-described = Playlist description saved
notice-moved-in-playlist = Track moved
page-filter = &Filter
page-sort = S&ort
list-filtered = { $name } ({ $shown } of { $total })
menu-focus-filter = &Filter the list
sort-original = Original order
sort-title = Title
sort-artist = Artist
sort-duration = Longest first
sort-year = Newest first
settings-notifications = Show a &notification for each new track while Tactus is hidden

menu-help = &Help
menu-shortcuts = &Keyboard shortcuts
menu-open-data-folder = Open the &data folder
menu-open-log = Open the &log
menu-check-updates = &Check for updates
menu-about = &About Tactus
shortcuts-global = Global hotkeys
about-text = Tactus { $version }
    An accessible Yandex Music player for Windows.
    https://github.com/{ $repository }
    Folder: { $folder }
settings-check-updates = Check for &updates at startup
notice-up-to-date = Tactus is up to date
update-found = Tactus { $version } is available
update-available = Tactus { $version } is available. You have { $current }. Download and install it now? Tactus will restart.
update-downloading = Downloading the update…
update-restart = The update was installed, but Tactus could not restart. Start it again manually.
update-installed = Tactus was updated to { $version }

menu-edit = &Edit
menu-undo = &Undo
menu-select-all = Select &all
menu-find-command = &Find a command…
find-command-prompt = Type to filter
list-selected-all = { $count ->
    [one] 1 item selected
   *[other] All { $count } items selected
}
notice-undone = Undone
notice-nothing-to-undo = Nothing to undo

settings-lists = Lists
settings-columns = Columns, in the order they are read
settings-columns-hint = Checked columns are shown. A screen reader reads them in this order.
settings-move-up = Move &up
settings-move-down = Move &down
settings-shortcuts = Shortcuts
settings-shortcuts-hint = Write shortcuts like Ctrl+Shift+K or F7. Every shortcut except function keys needs Ctrl, Alt or Shift.
shortcut-invalid = { $key } is not a valid shortcut.
shortcut-taken = { $key } is already used by { $action }. Use it here instead?
track-explicit = Explicit
artist-listeners = { $count } listeners last month
artist-likes = { $count } likes
artist-since = Since { $year }
section-followed-releases = New from artists you like
section-genre-releases = New in your genres
section-recommended-artists = Artists for you
section-genres = Genres and moods
kind-genre = Genre
button-clear-searches = Clear &search history
settings-spoken-speed = Speed for &podcasts and audiobooks
track-played = Played
action-mark-played = Mark as &played
action-mark-unplayed = Mark as &unplayed
button-hide-played = &Hide played
button-show-played = &Show played
notice-played = { $count ->
    [one] Marked as played
   *[other] { $count } episodes marked as played
}
notice-unplayed = { $count ->
    [one] Marked as unplayed
   *[other] { $count } episodes marked as unplayed
}
section-new-episodes = New episodes
settings-episode-notifications = Notify me about new &episodes of podcasts I follow
toast-new-episodes = { $count ->
    [one] A new episode
   *[other] { $count } new episodes
}
section-pinned = Pinned
action-pin = Pi&n to the collection
action-unpin = U&npin
notice-pinned = Pinned
notice-unpinned = Unpinned
button-upload = &Upload MP3 files…
upload-title = Choose MP3 files to upload to this playlist
upload-wildcard = MP3 files (*.mp3)|*.mp3
notice-uploaded = { $count ->
    [one] Uploaded. The track appears after Yandex processes it.
   *[other] { $count } tracks uploaded. They appear after Yandex processes them.
}
playlist-collective = collaborative
notice-offline = No connection. Downloaded music still plays, and Tactus keeps trying to reconnect.
notice-online = Connected again
status-offline = Offline
track-downloaded = Downloaded
menu-devices = &Devices…
devices-prompt = Choose a device
devices-disabled = Turn on Ynison in Settings, on the Playback tab, to see and control your other devices. It is experimental.
devices-none = No devices yet. Tactus is still connecting to Ynison.
devices-problem = Ynison is not available right now: { $problem }
device-active = , playing
device-this = , this computer
device-play-here = Play here
device-move-there = Move playback to { $device }
settings-ynison = Experimental: connect to &Ynison to see, control and hand off playback between your devices
settings-device-name = &Device name
settings-ynison-hint = Leave the name empty to use "{ $name }". With Ynison on, queue sync uses Ynison instead.
settings-voice = Voice
voice-enabled = Use &voice control
voice-language = Voice &language
voice-language-auto = Same as the interface (Russian for Georgian)
voice-language-ru = Russian
voice-language-en = English
voice-model = Speech &model for { $language }
voice-model-small = Small, { $megabytes } MB: fast, good for commands
voice-model-large = Large, { $megabytes } MB: more accurate, slow to load, needs about 4 GB of memory
voice-model-installed = The model is installed.
voice-model-missing = The model is not downloaded yet.
voice-model-downloading = Downloading in the background. Progress is shown in the status bar.
voice-download = &Download
voice-cancel-download = &Cancel download
voice-remove = &Remove
voice-phrases = Edit my &phrases
voice-microphone = &Microphone
voice-music = Music while &listening
voice-music-duck = Lower the volume
voice-music-pause = Pause
voice-music-keep = Keep playing
voice-silence = Stop listening after this much &silence, milliseconds
voice-wait = Give up when nothing is said for, &seconds
voice-limit = Longest &command, seconds
voice-wake-word = Listen for the wake &word "Tactus" (uses some processor time)
voice-log = &Log what voice control hears
voice-hint = Hold the voice hotkey to talk, or tap it to say one command. Set the key on the Hotkeys tab.
voice-download-confirm = Voice control needs a speech model ({ $megabytes } MB). Download it now?
voice-downloading = Downloading the speech model: { $percent }%
voice-phrases-header = # My voice phrases. They are added to the built-in ones.
    # Format: command = phrase | phrase
    # (a|b) picks one word, [word] is optional, {"{"}number{"}"} is a number, {"{"}name{"}"} is a name.
    # Example:
    # next = skip it | another one
notice-voice-ready = The speech model is ready
notice-voice-removed = The speech model was removed
notice-voice-cancelled = Speech model download cancelled
menu-listen = &Listen for a voice command
menu-voice-help = &Voice commands
hotkey-voice = Voice command (hold to talk, tap for one command)
announce-kind-voice = Voice control replies
voice-off = Voice control is off. Turn it on in Settings, on the Voice tab.
voice-starting = Voice control is starting. Try again in a moment.
voice-nothing = Nothing was heard
voice-not-understood = Not understood: { $text }
voice-cancelled = Cancelled
voice-failed = Voice control stopped: { $error }
voice-searching = Looking for { $name }…
voice-will-play = Playing { $title }. Say "no" or press the voice key to cancel.
voice-not-found = Nothing found for { $name }
voice-several = Several results for { $name }. They are on the search page.
voice-no-likes = You have no liked tracks yet
voice-no-playlist = You have no playlist called { $name }
voice-nothing-selected = Nothing is selected
voice-help-intro = Hold { $key } and speak, or tap it and say one command. You can say, for example:
