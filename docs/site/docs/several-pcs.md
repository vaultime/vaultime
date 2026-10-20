# Several PCs

Play on more than one PC and bring it together in one history. Each PC keeps recording its own sessions, and a merge copies the sessions of another PC from one of its backups.

## Merge another PC

1. On the other PC, pick **Save a backup** under Settings, Local backups, and bring the backup folder over, on a USB stick or through a folder that syncs.
2. On this PC, go to Settings, Other PCs and pick **Merge a backup of another PC**.
3. Pick the backup. Vaultime shows what would come in before anything changes: new sessions, sessions that grew since the last merge, sessions that fail their check and the games that come along.
4. Decide for each game whether it counts as the same game as one on this PC.
5. Pick **Merge**.

Vaultime saves a backup of this PC before each merge, so restoring that backup undoes it. It is one of the automatic backups, so it is deleted once seven newer ones are there. Merge again later and only what is new comes in. Sessions you removed here never come back.

> [!NOTE]
> A merge is not a sync. The other PC never changes, and a merge from a folder happens only when you start it.

### From the cloud

With cloud backup on both PCs, Settings, Other PCs lists the newest cloud backup of each of your other PCs, with its name. Pick **Merge** and the same dialog shows what would come in. After that first merge, newer backups of that PC come in by themselves within the hour:

- A game that is new here counts as one of yours only when it comes from the same launcher with the same launcher id. Otherwise it stays on its own, and you can link it on its game page.
- A backup that brings a key this PC does not trust, or sessions that fail their check, waits for you. The row says why, and **Merge** opens the dialog.

Each PC uploads its own backup once a day, and the server always keeps the newest backup of every PC, so the others can merge it.

## Which sessions count

Sessions come over exactly as the other PC recorded them, byte for byte. This PC's ledger then vouches only for those that pass their check with the keys it trusts for the PC that recorded them. A session that fails comes in Suspicious and stays visible, so nothing is laundered by a merge.

- The first time Vaultime sees a PC, it takes that PC at its word, since it cannot know it yet. Its keys are trusted from then on. From the cloud it counts only the keys that PC registered with the cloud server, which keeps every key a PC ever registered. Any other key needs you to confirm it, as described under New keys.
- A PC this one trusts also vouches for the sessions it took in from PCs this one has no keys for.
- Only this PC's own keys count for its own sessions. A backup of this PC cannot be merged, and when you restore one, its keys vouch for nothing.

## New keys

When a PC signs with a new key, for example after its key file was lost, its new sessions do not count right away. The merge dialog asks you to confirm with **This backup comes from my** and the PC's name. Once you confirm, the sessions that came in Suspicious because of that key pass as well.

Only confirm when you know the backup really comes from that PC.

## Running sessions

A session that was still running on the other PC when its backup was made does not come in with a merge. It waits for a later backup, once it has ended there.

Restoring a backup another PC made while a game ran there leaves that session open, exactly as that PC recorded it. Only the PC that recorded a session may close it, so it is never closed here as Recovered. It counts up to its last saved time, says it was still running on that PC, and never shows in the live bar or the tray. The next merge of that PC brings its real end.

## Setting a PC up again

When you set a PC up again, Vaultime makes a new key for it. If the first thing you do on the new install is restore that PC's last backup, the new install continues as that PC. The restore asks first with a switch that is on by default. A local restore names that PC, like "This is Desktop, set up again", and a cloud restore asks whether this PC is the one that made the backup, set up again.

Turn the switch off when this PC is not that PC, for example when you restore your desktop's backup on a new laptop just to look at its history. This PC then keeps its own id and keys, and the backup's sessions come in as the other PC's, the same as on a PC with history of its own. To set up a second PC for good, [merge](#merge-another-pc) the other PC's backup under Settings, Other PCs instead.

With the switch on:

- Its sessions are this PC's own again, and the keys the backup names for that PC count as this PC's own beside the new key.
- The ledger notes the takeover with the keys it took over, so it shows and can be checked.
- Sessions the PC played after that backup, which another PC took in, come back as this PC's own with the next merge of that PC, as long as that PC checked them only with keys this PC counts as its own, the old ones or the new one.

Only a fresh install takes over: one without sessions, merges or restores of its own. A PC with history of its own restores another PC's backup as described under [Backups](backups.md#restoring). Restart Vaultime after the restore to finish. Until then it does not track. Automatic backups still run and already carry the PC it continues as.

> [!CAUTION]
> Taking over trusts the backup the way a new PC trusts any PC it has never seen. The same goes for the first merge after it: a PC this one has never seen is taken at its word, also for the sessions it says it took in from this PC. A made up backup can therefore bring in made up play that reads as this PC's own, just as it could bring in made up play of a new PC on any install.

If that backup was made while a game ran, the session is closed as Recovered at its last saved time, since this PC now is the PC that recorded it. A longer end that another PC took in later stays out, and the shorter copy here stays.

## Games from another PC

A game that comes along with a merge is never tracked on this PC. You can let it count as the same game as one you have here, and Vaultime then shows the two as one game in the journal, the stats and on the game's page.

If you added play from the other PC here by hand before, the merge names the sessions that overlap what comes in, so you can take them out and nothing counts twice.

## Corrections

Only the PC that recorded a session can correct it. Correct it there, then merge again, and the correction comes along. A launcher [steps aside](counting.md#launchers-that-step-aside) only for games on its own PC.

## What is refused

- A backup of this PC itself.
- Any merge while this PC's own ledger fails its check, as after it was changed outside Vaultime.
- A backup of a PC without a ledger, or with a ledger that was changed.
- A backup that is damaged, holds database parts Vaultime never writes or reads differently from one moment to the next.
