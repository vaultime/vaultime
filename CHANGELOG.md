# Changelog

All notable changes to Vaultime will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Library

- **Journal colors from the real artwork.** A game's color in the journal is now the main color of its cover, so a red logo stays red instead of a mix of all its colors. Black, white and grey covers get a grey, not a made-up color, and their game pages stay grey too. A color only shifts when it would look like another game of the same week.
- **Make it yours.** Settings has a new Appearance section. Pick dark, light or the system's mode, one of five ground tones, an accent color from six swatches or any color you like, and a background picture of your own or a game's art, with dim and blur so text stays readable. The look stays on this PC, and backups leave it out.
- **The window wears your colors too.** The icon in the tray, the taskbar and the title bar takes your accent color, signed in to cloud backup or not. On Windows 11 the title bar takes the tone of the page and the window border your accent.
- **Covers framed your way.** Adding an image opens a frame to drag and zoom it, and Adjust frames the cover in use again. Zoom out to show a wide logo whole. Photos get a soft, dimmed copy of themselves behind them, logos a plain backdrop in their own color, so a black logo stays visible.
- **Any picture works.** Covers and the background picture also take GIF, TIFF, TGA, QOI, PNM and SVG files, besides PNG, JPEG, WebP, BMP and ICO.
- **A window that keeps its size.** Like a game client, Vaultime opens at one fixed size: Compact, Standard, Large or Extra large, picked in Settings under Window. Standard is the default. Sizes too large for your screen are marked, and the window steps down to one that fits. Free lets you resize and maximize the window as before. The size stays with this PC when you restore a backup.

## [0.2.0] - 2026-10-02

### Library

- **A new look for the type.** Titles and the wordmark use a wide Mona Sans, sentences about play the same face at normal width, and the interface Geist, with Geist Mono for times and numbers. The website and the logo match.
- **Simpler idle setting.** The idle time in Settings steps a minute at a time with minus and plus, can still be typed, and saves on its own like the switches next to it.
- **Room for longer names.** The library rail is a little wider, so more of each game title fits, and it shows the copyright next to the version. Settings lists it under About too.
- **Journal colors that stand apart.** Each game in a week of the journal gets its own clear color, and its name in the entries takes the color of its bar. Games keep the color of their cover where it does not clash with another game, and violet stays free for active time.
- **Sessions told in more ways.** Session lines vary their wording and notice more: a first look at a game, a return after weeks away, your longest session yet, a game left open, play past midnight and another round on the same day. A session with less than a minute of idle time reads "all of it active", and one that sat idle the whole time "all of it idle". Each session keeps its sentence wherever it shows up, and weeks are summed up in more than one way.

### Backups

- **Plain terms for the beta.** The terms of service now say clearly that cloud backup is a free private beta that comes without warranty and is used at your own risk.
- **Clearer cloud forms.** The button that shows a password works every time, not only while you first type it. Each cloud form says under the field what is wrong, like a password that is too short or a passphrase that does not match, and a wrong password shows right above the button you pressed.

## [0.1.0] - 2026-09-30

The first public release.

### Tracking

- **Automatic tracking.** Once a game is in your library, start it however you like and Vaultime records the session. Runtime, active playtime and idle time are counted apart.
- **Keeps running in the tray.** Closing the window keeps tracking, and Vaultime starts with Windows or when you log in on Linux. Both can be turned off in Settings. The tray menu shows the game running with its time and how long you played today.
- **Light on your PC.** A check for running games takes a few milliseconds every five seconds, and the window stops drawing while it sits in the tray.
- **Sleep aware.** Time while your PC sleeps is not counted.
- **Controller play counts.** Buttons and sticks count as input, so a game played on a controller does not turn idle. On Windows this covers Xbox controllers and controllers that act as one.
- **Nothing extra to install on Linux.** Vaultime asks the X server directly which window is in front and when you last used the keyboard or mouse. On Wayland, GNOME reports the idle time. Games that keep the screensaver off still turn idle when you leave.
- **Careful matching.** Games are matched by their full path, so two games with the same file name stay apart. Libraries behind junctions or symlinks and games that run through Wine still match.
- **Honest trust labels.** Every session keeps a hash-linked event log. Sessions with a clock jump or a record changed outside Vaultime are marked Suspicious, sessions rebuilt after an unclean exit Recovered, and both say what happened. The library marks a game for such sessions from the last 30 days, and its page counts all of them.

### Library

- **Library home.** Opens on the game you play right now or last, in colors taken from its cover, with the last seven days in numbers, a shelf of recent games and every game below.
- **Game pages.** The last two weeks at a glance, every session as a sentence, totals and the cover you pick.
- **Journal.** Your play week by week, with a 24 hour strip for each day. Games that ran side by side are hatched on the strip and count once in the day and week totals.
- **Where each game stands.** Mark a game as Backlog, Playing, Finished or Dropped on its page and filter the library by it. The journal notes the day, for example "You finished Hades II after forty-two hours."
- **Notes on sessions.** Write a line on any session in the journal or on the game page. Notes never change tracked time.
- **Honest corrections.** Cut a session short when a game was left running, take out a session that was no play at all, or add play from another PC. Changed sessions are labeled Edited or Manual and keep the old times and your reason in their log.
- **Playtime from before Vaultime.** Settings reads the playtime Steam counted for the games in your library, shows what it adds and counts it toward each game's total. Time Vaultime already tracked counts once, and reading Steam again keeps your corrections and the sessions you added by hand.
- **Stats.** Your year in play: a calendar of every day with your streaks, when in the week you play, your games with active and idle time apart, the months, how long your sessions run and the games you leave running.
- **Your games, sorted your way.** The list on the left sorts by recent play, title or playtime and keeps the choice.
- **Live bar and search.** The running game and its timer stay at the bottom of every page, and Ctrl+K jumps to any game, page or action.
- **Game discovery.** Reads what Steam, the Xbox app, Epic, GOG, EA, Ubisoft Connect, Battle.net, Riot, Rockstar, Amazon Games, itch.io, HoYoPlay, Legacy Games, Big Fish, Heroic and Lutris have installed, and scans common game folders on every drive for the rest. Any other game can be added by hand.
- **Steam covers.** Steam games get the cover art Steam already keeps on your PC.
- **Artwork.** Picks up cover art from your game folders, or add your own.
- **Hidden games.** Hide a game from the library without losing it. It stays tracked, its sessions still count in the journal and your totals, and Settings lists hidden games to bring them back.

### Backups

- **Local backups** to any folder, with a checksum for every file and a preview before a restore replaces anything. A restore only takes the files a backup may hold and keeps this PC's backup folder setting. Damaged artwork is skipped instead of stopping the restore.
- **Automatic backups** once a day and when Vaultime quits, the newest seven kept, in Vaultime's data folder or a folder you pick. Artwork that did not change is stored once for all of them. Signed in PCs also back up to the cloud once a day.
- **Invite-only cloud backup** on our own server. Free, with the same limits for every account: 1 GiB for backups and artwork together and up to 30 backups. Backups are encrypted on your PC before upload, and artwork is uploaded once and shared by every backup. At 30 backups a new upload replaces the oldest. When the storage is full, an upload is refused until you delete older backups. A backup passphrase that does not open your earlier cloud backups is refused. The server slows down password guessing and ends a session whose sign-in was copied.
- **Signed in at a glance.** The logo, the tray icon and the taskbar icon turn violet while the PC is signed in to cloud backup.
- **Password change** on the Cloud page. Your other PCs are signed out, and the backup passphrase stays the same.
- **Export.** Save every finished session as CSV for a spreadsheet or as JSON for other tools, with notes, trust labels and, in JSON, each game's status and earlier playtime.

### Platforms

- **Windows** with an installer for your user account.
- **Linux** as .deb, .rpm and AppImage. Every release is installed and started on Debian, Ubuntu, Fedora, Arch and openSUSE before it is published.
