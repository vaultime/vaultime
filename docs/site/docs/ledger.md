# The ledger and integrity

Each session keeps a log that only grows, and each PC keeps a signed ledger over those logs. Together they make changes made outside Vaultime visible. They cannot stop them.

## The session log

A session is not a single number that gets overwritten. It is a list of events: tracking started, the times saved along the way, sleep left out, the end, and later any correction you make. Each event carries a fingerprint of the one before it, so the events form a chain. The session's totals must match its newest event that counts time.

Changing an event, removing one or editing the totals breaks the chain or the match, and the session turns [Suspicious](trust-labels.md). Corrections are events too. They name the old values and your reason, so nothing is overwritten without a trace. You can read every session's log in the event log on its game's page.

## The ledger

A chain inside the database can be rebuilt by someone who rewrites the whole database. That is what the ledger is for.

Each PC keeps a ledger, a second chain that it signs with a key of its own. The key is in the file `device-key` next to the database. It never goes into a backup and never leaves the PC through Vaultime. The ledger:

- pins a session's record when the session starts, when it ends and when it is corrected,
- notes every session you remove, together with its game,
- notes every restore and every session taken in from another PC.

The file `device-keys` lists every public key the PC has signed with, so older entries still count as its own after a new key.

A session that was rewritten outside Vaultime, even with its own log rebuilt to match, no longer matches what the ledger pinned and turns Suspicious. So does a session no ledger knows. Sessions recorded before the ledger existed were taken in as they were when it began.

## Checking the ledger

Settings, This PC shows the state of the ledger:

In order
: Every session matches what the ledger holds.

Sessions missing
: Sessions in the ledger are gone from the history without a note that you removed them.

Changed
: The ledger itself was changed outside Vaultime, so the sessions it covers are marked Suspicious.

## What the checks catch

- A session's times or events edited in the database.
- A session whose log was rebuilt to look consistent.
- Sessions deleted outside Vaultime.
- Clock changes while a game runs.
- Sessions that turn up without this PC or a trusted PC having recorded them.
- Backups that are damaged or hold database parts Vaultime never writes. Those are refused before anything is merged or restored.

## Where the checks end

> [!CAUTION]
> The ledger shows changes. It does not prevent them, and it is no proof for anyone else.

- Someone with your PC and its key file can rewrite the history and the ledger together, and nothing will look wrong.
- Removing the newest sessions together with the newest ledger entries leaves a shorter ledger that still checks out.
- Vaultime only knows what happened while it ran. Play while it was closed is not recorded at all.
- No server checks your history. That is why there is no Verified label.

All of this helps you trust your own records and notice when something went wrong. It is not anti-cheat.

## When the key is lost

If `device-key` goes missing, for example after a fresh install without the old data folder, Vaultime makes a new key and goes on. The PC's earlier entries still count through `device-keys` when that file survived. Other PCs that merge from this one see a new key and count its sessions only once you confirm the backup comes from this PC, see [Several PCs](several-pcs.md#new-keys).

A copy of the whole data folder keeps the key. A backup made in Vaultime does not, on purpose.
