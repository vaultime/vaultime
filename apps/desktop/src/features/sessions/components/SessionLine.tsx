// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState, type FormEvent } from "react";
import { NotebookPen, Scissors } from "lucide-react";
import { IntegrityBadge } from "@/components/status/IntegrityBadge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { CorrectSessionDialog } from "@/features/sessions/components/CorrectSessionDialog";
import { SESSION_NOTE_MAX_CHARS } from "@/lib/constants";
import { describeSession, sessionAmounts, sessionTrustNote } from "@/lib/sentences";
import { formatSessionStart } from "@/lib/time";
import type { Session, SessionEvent } from "@/lib/types";
import { cn, describeError } from "@/lib/utils";

/** One session as a sentence, with its numbers, a note of its own and the trust label. */
export function SessionLine({
  session,
  events,
  gameTitle,
  when,
  bordered = true,
  note,
  onSaveNote,
  onCorrected,
}: {
  session: Session;
  /** Events of this session or more, used to explain flags and skipped time. */
  events: SessionEvent[];
  /** Set when the list mixes games. */
  gameTitle?: string;
  /** Replaces the start day and time in the left column. */
  when?: string;
  bordered?: boolean;
  note?: string;
  /** Lets the player write a note. An empty note removes it. */
  onSaveNote?: (note: string) => Promise<void>;
  /** Lets the player correct a finished session, then reloads. */
  onCorrected?: () => void;
}) {
  const trustNote = sessionTrustNote(session, events);
  const live = !session.ended_at_wall;
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [correcting, setCorrecting] = useState(false);

  function startEditing() {
    setDraft(note ?? "");
    setError(null);
    setEditing(true);
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    if (!onSaveNote) return;
    setSaving(true);
    try {
      await onSaveNote(draft);
      setEditing(false);
    } catch (saveError) {
      setError(describeError(saveError));
    } finally {
      setSaving(false);
    }
  }

  return (
    <article className={cn("group flex items-baseline gap-5", bordered && "border-b border-rule py-3.5")}>
      <span className="w-[20ch] shrink-0 font-mono text-[13px] text-faint">
        {when ?? formatSessionStart(session.started_at_wall)}
      </span>
      <div className="min-w-0 flex-1">
        <div className={cn("font-display text-[22px] leading-snug", live && "text-violet")}>
          {describeSession(session, gameTitle)}
        </div>
        <div className="mt-1 text-[13px] text-faint">
          {sessionAmounts(session)}
          {live ? " so far." : "."}
          {trustNote && <span className="text-soft"> {trustNote}</span>}
        </div>
        {editing ? (
          <form className="mt-2.5 flex flex-wrap items-center gap-2" onSubmit={(event) => void save(event)}>
            <Input
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Escape") setEditing(false);
              }}
              maxLength={SESSION_NOTE_MAX_CHARS}
              placeholder="A line about this session"
              aria-label="Note on this session"
              autoFocus
              className="h-9 max-w-[480px] min-w-0 flex-1"
            />
            <Button type="submit" size="sm" disabled={saving}>
              Save
            </Button>
            <Button type="button" variant="ghost" size="sm" onClick={() => setEditing(false)}>
              Cancel
            </Button>
            {error && <p className="w-full text-[13px] text-amber">{error}</p>}
          </form>
        ) : (
          note && <p className="font-display mt-1.5 text-[17px] leading-snug text-soft italic">“{note}”</p>
        )}
      </div>
      {!editing && (onSaveNote || (onCorrected && !live)) && (
        <span className="flex self-center opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
          {onSaveNote && (
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={note ? "Edit the note" : "Add a note"}
              title={note ? "Edit the note" : "Add a note"}
              onClick={startEditing}
              className="text-faint"
            >
              <NotebookPen className="size-3.5" />
            </Button>
          )}
          {onCorrected && !live && (
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Correct the time"
              title="Correct the time"
              onClick={() => setCorrecting(true)}
              className="text-faint"
            >
              <Scissors className="size-3.5" />
            </Button>
          )}
        </span>
      )}
      <IntegrityBadge status={session.integrity_status} />
      {onCorrected && correcting && (
        <CorrectSessionDialog
          session={session}
          gameTitle={gameTitle}
          open={correcting}
          onOpenChange={setCorrecting}
          onCorrected={onCorrected}
        />
      )}
    </article>
  );
}
