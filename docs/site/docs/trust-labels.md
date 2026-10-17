# Trust labels

Every session carries a label that tells how much its record can be trusted. A label that is not Local always comes with the reason.

## The labels

Local
: Recorded normally on this PC and unchanged since.

Suspicious
: Something about the record does not add up: the system clock jumped while the game ran, or the record was changed outside Vaultime. The session keeps its time, but the label stays.

Recovered
: Vaultime was closed while the game ran, for example after a crash or a power cut, and closed the session on the next start. It keeps the time up to Vaultime's last note, at most 30 seconds before the crash.

Edited
: Tracked, then corrected by you. The old times and your reason stay in the session's log.

Manual
: Added by you for play Vaultime did not see, like a session on another PC. It counts as active time.

There is no Verified label. Vaultime saves it for a check done by a server, and no such check exists yet. Nothing on your own PC can prove that a record is genuine, so Vaultime does not claim it.

## What makes a session Suspicious

- The system clock went back by more than a second while the game ran.
- The system clock and Vaultime's own steady clock drifted apart by more than 20 seconds between two checks, or by more than 45 seconds over the whole session.
- The session's log or its totals were changed outside Vaultime, so they no longer match.
- The session names another game or another PC than when it started.
- Something was added to the session after it ended, other than a correction.
- A correction would have added time.
- The PC's [ledger](ledger.md) does not know the session, or holds a different record of it.
- The session came from [another PC](several-pcs.md) and did not pass its check with the keys this PC trusts for that PC.

A suspicious session stays Suspicious when you correct it, so a correction can never hide a flag. Clock changes from time zones and daylight saving time do not count, as Vaultime keeps its times in UTC.

## Where labels show

- **The journal** shows the label next to each session and the reason under it.
- **The library** marks a game when one of its sessions from the last 30 days is Suspicious or Recovered.
- **A game's page** counts all of its Suspicious and Recovered sessions, and its event log shows when and why a session was flagged.

## What the labels can and cannot do

The labels show changes. They cannot prevent them. Someone with access to your PC can rewrite both the history and the ledger, and a clever enough edit leaves no trace. Treat the labels as hints about your own records, not as proof for anyone else. [The ledger and integrity](ledger.md) explains how the checks work and where they end.
