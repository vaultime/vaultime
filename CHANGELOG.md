# Changelog

All notable changes to Vaultime will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixes

- **A cloud restore asks before it takes over.** A fresh install that restores a cloud backup only continues as the backup's PC when the restore showed that choice. It used to take over even when the check behind the choice failed, or when the backup did not name its PC and the choice never showed. Restore waits until that check is done.
- **A quick restart stays one session.** A game has to be missing at two checks in a row, about ten seconds, before its session ends. A game that restarted itself after an update, or a single check that missed it, used to split the session in two.
- **Tracking keeps going after an error.** An unexpected error while Vaultime checks your games goes to the log, and the next check starts over. It used to stop tracking until Vaultime restarted, while Settings still said tracking was running. A session the database could not close is closed at the next check instead of staying open until the next start.
- **Cloud requests give up after 30 seconds.** A cloud server that stops answering no longer leaves the Cloud page waiting for good or holds up the daily backup.
- **An offline start says so.** When the cloud server cannot be reached as Vaultime starts, the Cloud page says that, instead of reporting that registering this PC failed. This PC registers once the server answers again.
- **The library is current after a cloud restore.** The library, journal and stats show the restored history right away, as after a local restore. They used to show the old history until a game started or stopped.

## [0.5.0] - 2026-10-17

### Library

- **One game, two launchers.** A game installed twice, like Cyberpunk 2077 through Steam and GOG or a game in the Xbox app and on Steam, can count as one game. On a game's page, Counts as links it to another game of your library. Both installs stay tracked, each with its own program, and the library, journal, stats, live bar and tray show one game with one history. Its page lists the installs that count for it, each with its program, a way to edit it and Unlink, which brings it back as its own game with its own sessions. Each session says which install recorded it. The game keeps the status you gave it, and an install keeps its own for when it is unlinked. Deleting the game, also from the library, says that its installs go along, and marking it as no game ignores the programs of all of them. Nothing in the record changes, so linking and unlinking can be undone at any time.
- **Discovery knows a second copy.** When a scan finds a game from another launcher with the title of a game in your library, the Discover dialog shows it as the same game and links it when you add it. Untick it to add it as a game of its own.
- **Counted once.** Two installs of one game running at the same time count once everywhere, in the library and its sorting, on the game's page, in the journal's week and in the stats, a session added by hand cannot overlap play of a linked install, and Steam's earlier playtime only leaves out time a Steam copy tracked before Steam was read, also play that comes in later from another PC. Exports name the game shown and the install that recorded each session.
- **Linked on every PC.** A merge brings another PC's linked installs along as one game, counting as whatever its main copy counts as here. A restore of this PC's backup keeps its links.
- **New games find you.** When Vaultime starts, it reads what Steam and your other launchers installed and offers the games that are not in your library yet. It offers each game only once, so games you leave out stay quiet, and Discover in the library still lists everything. When Vaultime starts in the tray, it waits until you open the window. Settings, In the background turns it off.

### Journal

- **Counted where it happened.** The journal now counts each session exactly where it happened on the clock, like the stats always did, so a day in the journal and the same day in the stats show the same time. A session that slept through midnight, or went idle for an hour, puts its time on the day and the hours it really fell in instead of spreading it evenly over its span. The library's last seven days, the live bar and the tray count today the same way, and days played and streaks count every day with play on it, also the day after midnight.
- **One session past midnight reads as one.** Play still counts on the day it happened, but a session that runs past midnight no longer looks like two. Its bar runs out at the right edge of the first day and in from the left edge of the next, and the next day shows a quiet line like "Still in Elden Ring from Saturday night" instead of a second entry. The entry reads "22:40 to Sun 02:15" with the whole session's time and how much of it counts after midnight, the line says how much of the whole falls on its day, and each leads to the other, also into the next or last week. Pointing at either marks every part of the session, and pointing at a bar shows each day's part and the whole. A week that opens with the end of a session from the week before says so.
- **Status changes count up to their moment.** "You finished Hades II after two hundred hours" now counts the game's time exactly up to the change, as the stats count it. A session that ran across the change counts only up to it, linked installs count and time a launcher set aside does not.

### Stats

- **The calendar matches the journal.** Each day of the year calendar is as bright as the time the journal shows for that day, games side by side counted once, instead of active time per game, and pointing at a day leads with that time. The calendar on a game's page counts that game's time played the same way.
- **Share your year.** Share your year, on the Stats page and in the command palette, makes a picture of the year to post: your hours of active play, the days you played, the calendar, your most played games with their covers and journal colors, the busiest month and a line about the year. It comes wide for X, Discord and Mastodon, square for feeds or tall for stories, dark or light in your ground and accent. Game titles and covers, the calendar and the name of your PC can stay off, and hidden games never show. It counts play exactly as the Stats page does. Vaultime draws the picture on your PC and uploads nothing. Save it as a PNG, or copy it to paste it straight into a post where your system allows it.

### Trust

- **Another PC's running session stays its own.** Restoring a backup that another PC made while a game ran there leaves that session open, exactly as that PC recorded it, instead of closing it here as Recovered. It counts up to its last saved time, says it was still running on that PC, and never shows in the live bar or the tray. Merging a later backup of that PC brings its real end. Closing it here added to a record only that PC may extend, so its real end clashed with it later and most of the session was lost. Only this PC's own sessions are closed after a crash or a restore, and only they keep a restore waiting.
- **Recovered time is pinned too.** While a game runs, the ledger now also holds on to the session's record once every five minutes while it runs, idle time included, about twelve times an hour. After a crash, a session still keeps its time up to Vaultime's last note, but only five minutes past the last note the ledger holds count. A session whose notes ran further, as when the last one was raised outside Vaultime before a restart or in a backup, is counted up to those five minutes and turns Suspicious, also when it comes from another PC. Notes in between used to be held by no ledger, so a raised one counted in full. Sessions recorded before this version keep their time as it was.
- **A new install carries on as your PC.** A fresh install with no history of its own that restores a backup of another PC, like your own PC's last backup after setting it up again, now continues as that PC. The restore asks first with a switch that is on by default, and a local restore names that PC. Turn it off to keep this PC as it is and take in the backup's history as another PC's, like when you only want to look at your desktop's history on a new laptop. That PC's sessions are this PC's own from then on, the keys its backup names for it count beside the new key, and the ledger notes the takeover. Sessions that PC played after its last backup and another PC took in come back as this PC's own with the next merge of that PC. They used to come in Suspicious, as only the old key could vouch for them. A PC with history of its own restores another PC's backup as before, and the key file still never goes into a backup.

### Fixes

- **Starts the version you installed.** Starting with the system now always opens the copy of Vaultime you last ran. A login item made by an older copy in another folder used to keep starting that copy after every restart.
- **A broken record stays small.** A session whose record spans more than a year on the clock, which only a damaged or edited record does, now counts in the quarter hour it ended in. Merging or restoring such a session used to fill every quarter hour of those years and could stall Vaultime.
- **A merged PC stays merged.** Restoring a backup of a PC whose sessions were merged here keeps those sessions correctable only on that PC. The restore used to forget the merge, so a correction here could split a session from its record there.
- **PC names stay short.** A PC name that comes along with a merged or restored backup is cleaned the way a rename cleans it: control characters go and it is cut to 40 characters. A backup used to bring any name along.
- **Impossible times are caught.** A session whose record holds a time below zero or longer than ten years, which only a damaged or made up record does, now turns Suspicious, and the stats count it as ten years at most. Such a record used to throw the sums off.

### Website

- **Documentation.** The website explains how Vaultime counts playtime, what the trust labels and the ledger mean, how several PCs and backups work, where your data lives and what to know on Linux, at [vaultime.codfishcloud.de/docs](https://vaultime.codfishcloud.de/docs/).

## [0.4.0] - 2026-10-03

### Tracking

- **Today means today.** The live bar and the tray count today's play the way the journal does: split at midnight, games side by side once, and a launcher only where it ran alone. The live bar used to add up every session that started today, counting games side by side twice.
- **Honest matching notes.** A game's page says when its launcher lets any game program in its folder count for it, instead of claiming it is matched by its full path alone. Clearing the install folder in Edit now clears it.
- **A lighter, sharper session record.** Vaultime still checks your games every five seconds, but it now notes a session's time whenever it switches between active and idle, and at least every 30 seconds, instead of every five seconds. The record grows about six times slower, so backups stay small for years, and each note now marks a real change. After a crash, a session keeps its time up to the last note, at most 30 seconds before the crash.
- **Launchers step aside.** A new switch on a game's page, Count only when no other game runs, is made for launchers and game clients like the League of Legends client. Its time beside another game is set aside: it stays in the session's record but counts nowhere, and switching it off brings it back. The journal draws such a game only where it ran alone and says how much was set aside, and a game's page suggests the switch when games often started while it was open and ran beside it.

### Corrections

- **Corrections take out exactly what happened.** Counting a session only until a time now removes exactly the active and idle time after it, read from the session's own record, instead of taking idle time first. The dialog shows the session's start, end and counted time, and what the correction would leave, worked out the same way as the correction itself.
- **Only ever less.** A correction can only take time out. Vaultime refuses one that would add time, move the end later or change nothing, and a recorded correction that added time marks the session Suspicious.
- **Your reason in quotes.** A corrected session reads "Cut short by you from 2 h 08 (“Fell asleep”)", so a short reason never runs into a time.

### Trust

- **A ledger for every PC.** Vaultime keeps a ledger of the sessions played on each PC, signed with a key that never leaves it, not even in a backup. It holds on to each session's record when the session starts, ends or is corrected, and notes the sessions you remove with their game and every restore. A session rewritten outside Vaultime, even with its own record rebuilt to match, or one no ledger knows, turns Suspicious. Settings, This PC tells whether sessions went missing. Sessions from before this version are taken in as they are. Like the labels, the ledger shows changes but cannot prevent them.
- **Name your PC.** Settings, This PC holds a name for the PC, its own name to begin with.
- **Several PCs, one history.** Settings, Other PCs merges the sessions of another PC from one of its backups. You see what comes in before anything changes, and each game that comes along can count as the same game here. Sessions come over exactly as recorded there, and this PC's ledger vouches only for those that pass their check with the keys it trusts for the PC that recorded them. A session that fails comes in Suspicious. A PC this one has never seen is taken at its word, as Vaultime cannot know it yet, and a PC it trusts vouches for the sessions it took in from PCs this one has no keys for. A backup that is damaged, holds database parts Vaultime never writes, or reads differently from one moment to the next, is refused. The merge names sessions you added by hand that overlap sessions coming in, as they may be the same play. Games from another PC are never tracked here, a launcher only steps aside for games on its own PC, and a session can only be corrected on the PC that recorded it. A backup is saved before each merge.
- **Only this PC's keys count.** A ledger that merely claims to be this PC vouches for nothing, also in a backup that says it comes from this PC. A new key for a PC this one has seen before, also one seen only through another PC, counts only once you confirm the backup comes from that PC. Then sessions that came in Suspicious because of that key pass too. A restore takes in only sessions that pass with the keys this PC trusts, or that this PC vouched for before in the same state. A session still running in a restored backup is closed as Recovered at its last saved times, which no ledger pins, and a backup of a PC this one has never seen is taken at its word like a first merge.
- **Nothing after the end.** Once a session ended, only corrections follow in its record. A session that goes on after its end turns Suspicious.
- **One game on two PCs.** A session added by hand cannot overlap play of the same game on another PC once the games are linked, and play of another PC's game is added on that PC.

### Backups

- **Old cloud backups make room.** When your cloud storage is full, a new backup replaces the oldest ones once it is safely stored, instead of being turned away. Only a backup larger than the whole storage is refused. This needs the updated server.
- **A restored history is checked right away.** After a restore, Vaultime checks the record of every restored session again before it shows it, instead of relying on the checks it made of the replaced history until the next start.

### Stats

- **See what you played on any day.** Point at a day in the year calendar or at a month to see each game you played and for how long, in its own color, with idle time below. Click a day, or move there with the arrow keys and press Enter, to read that week in the journal.
- **Time on the day it happened.** A session past midnight counts on both days, in the stats and in the journal. Stats now come from where each session's time actually fell, a quarter hour at a time, so active and idle time land in the right hours too.

### Game pages

- **A game's whole history.** The last two weeks became a history: the last 30 days, 12 months, every year, or every day of this year. Point at a bar or a day to see its time.

### Library

- **More from Ctrl+K.** The command palette saves a backup, exports your sessions, checks for updates and jumps straight to any part of Settings. On a game's page it also sets the game's status, hides it and switches whether it counts only when no other game runs.
- **Ignore what is no game.** A program that turns up in Discover but is no game, like a tool or a benchmark, can be ignored there or when you delete its game. Discover never offers it again and Vaultime never tracks it, also inside a game's folder. Settings lists ignored programs, each with Allow again. Hiding a game stays what it was: the game is still tracked, only the library leaves it out.
- **Find a game.** A search field above your games filters them as you type, ignoring accents and the order of words. Ctrl+F jumps into it, and opens the search on other pages. The command palette finds games the same way.

### Updates

- **Check for updates yourself.** Settings, About shows whether Vaultime is up to date and has a button to check now. A running Vaultime also looks for a new version every six hours, not only when it starts.

### Journal

- **Go to any day.** A calendar button beside the week arrows opens a date picker and shows the week of the day you pick.
- **Play past midnight on the next day too.** The next day's strip shows the rest of the session, with a short line in the journal, and each day's total counts only its own hours.
- **Games side by side, drawn cleanly.** Sessions that overlap on the day strip now form one rounded block. The stripes run on without breaks from one stretch to the next, and every change between games leans along them. A client left open through several matches, like Teamfight Tactics in the League client, reads as one block with the matches striped inside it.
- **The right games side by side.** The note under the strip names only the games that actually ran together, one line for each group. When one game ran through all of it, like a launcher, the note says which games ran alongside it. Before, it listed every game that overlapped any other that day as if they had all run at once.

## [0.3.0] - 2026-10-02

### Appearance

- **Make it yours.** Settings has a new Appearance section. Pick dark, light or the system's mode, one of six ground tones, an accent color from six swatches or any color you like, and a background picture of your own or a game's art, with dim and blur so text stays readable. The look stays on this PC, and backups leave it out.
- **The window wears your colors too.** The icon in the tray, the taskbar and the title bar takes your accent color, signed in to cloud backup or not. On Windows 11 the title bar takes the tone of the page and the window border your accent. On Linux the tray icon follows, and the window icon on X11.

### Window

- **A window that keeps its size.** Like a game client, Vaultime opens at one fixed size: Compact, Standard, Large or Extra large, picked in Settings under Window. Standard is the default. Sizes too large for your screen are marked, and the window steps down to one that fits. Free lets you resize and maximize the window as before. The size stays with this PC when you restore a backup.
- **Made for small windows too.** In narrow windows the year of days in Stats spans the full width, the notes under the numbers wrap instead of being cut off, and the game list fits more games in a low window.

### Journal

- **Journal colors from the real artwork.** A game's color in the journal is now the main color of its cover, a little more vivid, so a red logo stays red instead of a mix of all its colors. Black, white and grey covers get a grey, not a made-up color, and their game pages stay grey too. Covers with two strong colors, like a gold and blue logo, show both: the time bars and the game's name blend from one into the other. When a game's first color is taken that week, it uses its second one, and a color only shifts when neither fits.

### Covers

- **Covers framed your way.** Adding an image opens a frame to drag and zoom it, and Adjust frames the cover in use again. Zoom out to show a wide logo whole. Photos get a soft, dimmed copy of themselves behind them, logos a plain backdrop in their own color, so a black logo stays visible. An image you frame again counts as one of yours.
- **Tidy up your covers.** The cover section lists the images you added apart from the ones Vaultime found. Delete any of them, and the next one takes over if it was the cover. The file itself stays where it is, so Scan folder finds a deleted image from the game folder again.
- **Scans keep your cover.** Scan folder keeps the images it already knows and the cover you chose, where it used to replace them and could switch the cover. Images that changed on disk are read again.
- **More picture formats.** Add image and the background picture also take GIF, TIFF, TGA, QOI, PNM and SVG files, besides PNG, JPEG, WebP, BMP and ICO. Animations show their first frame, and text and pictures inside an SVG file are left out.

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
