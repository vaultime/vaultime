# How playtime is counted

Every five seconds Vaultime looks at which of your games run, whether one of them is in front and whether you are at your PC. From that it keeps three kinds of time apart.

## Three kinds of time

| Time | Meaning |
|---|---|
| Runtime | How long the game was running |
| Active | Runtime while the game was in front and you were at your PC |
| Idle | Runtime while the game sat in the background or you were away |

Runtime is always active and idle time together. Each session in the journal and each game's totals show all three.

## When you count as away

You count as away after 5 minutes without keyboard, mouse or controller input. Change it under Settings, Tracking, **Count as idle after**. The buttons step by a minute, and you can type any value from 5 seconds up to a day.

Vaultime only asks the system when your last input happened. It does not record what you typed or clicked.

### Controllers

Controller input counts like the keyboard and mouse.

- **Windows.** Xbox controllers and controllers that act as one, read through XInput four times a second.
- **Linux.** Every device the system reports as a gamepad or joystick. Vaultime opens only those, never a keyboard or mouse.

A stick or trigger has to move past a small dead zone, so a resting controller does not keep you active. Some controllers are not seen on Windows, see [Troubleshooting](troubleshooting.md#controller-play-counts-as-idle).

## Games in the background

A game turns idle 15 seconds after it leaves the front, so a quick look at another window does not cost you active time. If you play in a window next to other things, switch on **Count background games as active** under Settings, Tracking.

## Sleep and pauses

Time while your PC sleeps is never counted. When the time between two checks is longer than a minute, or the clock that keeps running through sleep jumps ahead of the one that does not, that stretch is left out and the session's log says so.

## How a game is recognized

Vaultime knows a game by the full path of its program. Games from launchers that give every game a folder of its own (Steam, Epic, GOG, Heroic, Battle.net, Riot, HoYoPlay, Ubisoft Connect, EA, Rockstar, the Xbox app, Amazon Games and itch.io) also count when any game program in their install folder runs, since these launchers often start a game in several steps. A game's page says which of the two applies. Vaultime goes by the file name alone only when Windows hides the full path or the game runs through Wine.

Ignored programs never count, and games that came from [another PC](several-pcs.md) are never tracked here.

## Several games at once

Each game that runs gets a session of its own. Time played counts clock time once, so two games side by side for an hour add one hour to the day: in the journal's day and week totals, the hours played on the stats page, the library's last seven days and today's time in the live bar and the tray. Each game's own totals still count all of its time.

### One game, two installs

A game installed twice, like through Steam and GOG, can count as one game. Both installs keep their own sessions, and when both run at the same time, or the game runs on two PCs at once, that time counts once in every total: the library, the game's page, the journal and the stats. A game can only be played once at a time.

### Launchers that step aside

Some launchers and game clients stay open while you play the game they started, like the League of Legends client. Switch on **Count only when no other game runs** on the game's page. Its time beside another game is then set aside: it stays in the session's record but counts nowhere, and switching it off brings it back. The game's page suggests the switch when other games often started while it was open.

## Midnight

Time counts on the day it happened. A session from 23:00 to 02:00 counts one hour on the first day and two on the next, in the stats, the journal's day totals, the library and the live bar. It counts where it happened on the clock, from the session's own notes: a session from 23:00 to 02:00 that slept from 23:30 to 01:30 counts half an hour on each day, not an even share of its span. Days played and streaks count every day with time on it. The journal lists the session under the day it started and shows it as one session that runs on, see [Past midnight](journal-and-stats.md#past-midnight).

## Checkpoints and crashes

While a game runs, Vaultime notes its time whenever it switches between active and idle, and at least every 30 seconds. If Vaultime or the PC goes down while a game runs, the session is closed on the next start and labeled [Recovered](trust-labels.md). It keeps its time up to the last note, so at most 30 seconds are lost.

## Playtime from before Vaultime

Settings, Earlier playtime, **Import from Steam** reads how long Steam says you played each of your Steam games. That time counts toward each game's total and shows as **Before Vaultime** on its page, minus the time Steam and Vaultime both counted, so nothing counts twice.

- Steam's number is a snapshot of the moment it was read. Sessions of a Steam copy that ended before then come off, also ones that come in later from [another PC](several-pcs.md). Sessions after it count on top.
- A session you corrected comes off at the length Vaultime first tracked, so a cut stays cut when you read Steam again.
- A session you added by hand only comes off when you marked it as **Played through Steam**.
- Imported time stays out of the journal and the stats, because Steam does not say when you played.

**Read again** updates the numbers later, and **Remove** takes the imported time out.

## When the rules change

Vaultime keeps every session's full record, not just its totals. The stats are worked out from that record in quarter hours, and when a counting rule changes they are built again from the record. History is never changed to fit a rule.
