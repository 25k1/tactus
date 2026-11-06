# Tactus

An accessible music player.

Tactus currently supports Yandex Music only.

> **Disclaimer:** Tactus is an unofficial client and is not affiliated with or endorsed by Yandex. It comes with no warranty, so use it at your own risk. I have used it daily for a long time without any problems.

## Download

Get `tactus.exe` from the [latest release](https://github.com/25k1/tactus/releases/latest). It's portable: put it in any folder you can write to and run it. Settings and downloads are kept next to it.

## Sign in

Choose **Get verification code**, open the verification page, enter the code, and Tactus signs in automatically.

## Help

Press **F1** in Tactus to see all keyboard shortcuts.

## Winamp plugins

Tactus also comes as two plugins for Winamp 5.x, so you can play Yandex Music in Winamp with its equalizer, DSP plugins, visualizations and global hotkeys.

1. Get `tactus-…-winamp-plugins.zip` from the [latest release](https://github.com/25k1/tactus/releases/latest).
2. Close Winamp, copy `in_yandex.dll` and `gen_yandex.dll` into Winamp's `Plugins` folder, and start Winamp again.
3. Open Winamp's main menu with **Alt+F**, choose **Yandex Music**, then **Settings**.
4. Choose **Sign in**. The plugin copies a code to the clipboard and opens the verification page; enter the code there and the plugin signs in by itself.
5. Open the music browser from the same submenu with **Open Yandex Music**.

In the Yandex Music window you can search, browse My Wave stations, your collection, playlists, podcasts and history, and play, enqueue or play next anything you find. Press **Enter** to play or open the selected item, **Backspace** or **Alt+Left** to go back, **Ctrl+F** to search, **F5** to refresh, and the context menu key for more actions. A Yandex Music link pasted into the search box opens what it points to.

My Wave keeps adding tracks to the Winamp playlist while it plays and tells Yandex which tracks you finished or skipped. The **Yandex Music** submenu, and **Preferences → Global Hotkeys**, also let you start My Wave, like or dislike the current track, start a wave from it, and hear what is playing now.

**Settings** in the **Yandex Music** submenu, the **Settings** button in the Yandex Music window, or **Configure** for either plugin under **Preferences → Plug-ins**, sets the streaming quality and the plugin language, signs you in or out, and lists the actions you can give global hotkeys. **File info** (**Alt+3**) on a Yandex Music track shows its details, with buttons to copy its link or open it in your browser.

Yandex Music tracks are saved in Winamp playlists as `yandexmusic://` links, and **Play URL** (**Ctrl+L**) accepts music.yandex.ru track links. The plugins use Winamp's proxy setting and keep their sign-in in `Plugins\tactus-yandex` inside Winamp's settings folder.

## License

MIT
