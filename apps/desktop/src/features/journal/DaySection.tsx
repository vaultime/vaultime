// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { memo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { CARD_WIDTH_PX, cardPosition } from "@/components/charts/play-card";
import { PhraseText } from "@/components/media/PhraseText";
import { GameStatusIcon } from "@/components/status/GameStatusIcon";
import type { GameSummary } from "@/features/library/library-context";
import { SessionLine } from "@/features/sessions/components/SessionLine";
import { DAYS_PER_WEEK } from "@/lib/constants";
import { sideBySideSentence, statusSentence } from "@/lib/sentences";
import { afterMidnightLead, continuedAmounts, continuedPhrase, sessionAmounts } from "@/lib/session-lines";
import {
  formatClockTime,
  formatDate,
  formatHoursMinutes,
  formatSpanOnDay,
  parseVaultimeDate,
  toDayKey,
} from "@/lib/time";
import type { DayShare, PlayRun, Session, SessionEvent } from "@/lib/types";
import { cn } from "@/lib/utils";
import { PlayBlock } from "./PlayBlock";
import { BLEED_PX, HIT_SLOP_PX, hatch, OUTLINE_GAP_PX, placeOnDay, TICKS, type Place } from "./strip";
import { entryId, type JournalDay } from "./week";

/** A session's part on a day's strip, with whether it runs in from or out to midnight. */
interface StripPart {
  session: Session;
  place: Place;
  startMs: number;
  endMs: number;
  openStart: boolean;
  openEnd: boolean;
}

/** Local midnight at the start of the day a share is on. */
const dayOf = (share: DayShare) => parseVaultimeDate(share.day);

/**
 * What a session on the strip is, for the pointer: its game, when it ran,
 * and with play past midnight each day's part and the whole.
 */
function SessionCard({
  session,
  shares,
  day,
  title,
  mark,
  style,
}: {
  session: Session;
  shares: DayShare[];
  day: Date;
  title: string;
  mark: string;
  style: CSSProperties;
}) {
  const started = parseVaultimeDate(session.started_at_wall);
  const ended = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : null;
  const dayName = (share: DayShare) =>
    formatDate(dayOf(share), shares.length > DAYS_PER_WEEK ? { day: "numeric", month: "short" } : { weekday: "long" });
  return (
    <div
      aria-hidden="true"
      style={{ width: CARD_WIDTH_PX, ...style }}
      className="pointer-events-none absolute z-20 rounded-xl border border-hairline bg-surface px-4 py-3 shadow-lg shadow-scrim/40"
    >
      <div className="flex items-center gap-2.5 text-[13px]">
        <span className="size-2 shrink-0 rounded-full" style={{ background: mark }} />
        <span className="min-w-0 flex-1 truncate">{title}</span>
      </div>
      <div className="mt-1 font-mono text-xs text-faint">
        {formatSpanOnDay(started, ended, shares[0] ? dayOf(shares[0]) : day)}
      </div>
      {shares.length > 1 ? (
        <>
          <ul className="mt-2.5 flex flex-col gap-1">
            {shares.map((share) => {
              const here = share.day === toDayKey(day);
              return (
                <li key={share.day} className={cn("flex items-baseline gap-2.5 text-[13px]", !here && "text-faint")}>
                  <span className="min-w-0 flex-1 truncate">{dayName(share)}</span>
                  <span className="shrink-0 font-mono text-xs tabular-nums">{formatHoursMinutes(share.runtime_ms)}</span>
                </li>
              );
            })}
          </ul>
          <div className="mt-2 flex items-baseline gap-2.5 border-t border-rule pt-2 text-[13px] text-soft">
            <span className="min-w-0 flex-1">{ended ? "In all" : "So far"}</span>
            <span className="shrink-0 font-mono text-xs tabular-nums">{formatHoursMinutes(session.runtime_ms)}</span>
          </div>
        </>
      ) : (
        <div className="mt-1.5 text-xs text-soft">{sessionAmounts(session)}</div>
      )}
    </div>
  );
}

/** A link inside a sentence, to the other part of a session past midnight. */
function JumpLink({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className="cursor-pointer rounded-[2px] underline decoration-hairline-strong decoration-1 underline-offset-[0.2em] transition-colors hover:decoration-current focus-visible:decoration-current focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet/50"
    >
      {children}
    </button>
  );
}

/** "Thursday", "Thursday and Friday" or "Thursday to Sunday", each day a link to that day. */
function DayLinks({ shares, onJump }: { shares: DayShare[]; onJump: (day: Date) => void }) {
  const link = (share: DayShare) => {
    const name = formatDate(dayOf(share), { weekday: "long" });
    return (
      <JumpLink label={`${name}, where this session went on`} onClick={() => onJump(dayOf(share))}>
        {name}
      </JumpLink>
    );
  };
  const first = shares[0];
  const last = shares[shares.length - 1];
  if (shares.length === 1) return link(first);
  return (
    <>
      {link(first)}
      {shares.length === 2 ? " and " : " to "}
      {link(last)}
    </>
  );
}

/** One day of the journal: its strip of play and a sentence per session. */
export const DaySection = memo(function DaySection({
  day,
  byGame,
  sessionsByGame,
  sessionsById,
  sharesOf,
  markOf,
  fillOf,
  events,
  notes,
  onSaveNote,
  onCorrected,
  playedBefore,
  linked,
  onLink,
  onJump,
}: {
  day: JournalDay;
  byGame: Map<string, GameSummary>;
  sessionsByGame: Map<string, Session[]>;
  sessionsById: Map<string, Session>;
  /** A session cut at midnight by its id, each day with the time it counts there. */
  sharesOf: (sessionId: string) => DayShare[];
  /** The game's color in this week. */
  markOf: (gameId: string) => string;
  /** What a game's time bars are filled with. */
  fillOf: (gameId: string) => string;
  events: SessionEvent[];
  notes: Record<string, string>;
  onSaveNote: (sessionId: string, note: string) => Promise<void>;
  onCorrected: () => void;
  playedBefore: (gameId: string, moment: string) => number;
  /** The session marked on every day it ran. */
  linked: string | null;
  onLink: (sessionId: string | null) => void;
  /** Goes to a session's entry or line on a day, in another week too. */
  onJump: (sessionId: string, day: Date) => void;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const [card, setCard] = useState<{ sessionId: string; style: CSSProperties } | null>(null);
  const dayKey = toDayKey(day.start);
  const titleOf = (gameId: string) => byGame.get(gameId)?.game.title ?? "a removed game";
  const fillFor = (gameId: string) => (fillOf(gameId) === markOf(gameId) ? undefined : fillOf(gameId));

  // Each session's parts on the strip as the core placed them, and whether
  // they run in from the day before or out past midnight.
  const parts: StripPart[] = day.parts.flatMap((part) => {
    const session = sessionsById.get(part.session_id);
    if (!session) return [];
    return [
      {
        session,
        place: placeOnDay(new Date(part.start_ms), new Date(part.end_ms), day.start),
        startMs: part.start_ms,
        endMs: part.end_ms,
        openStart: part.open_start,
        openEnd: part.open_end,
      },
    ];
  });
  const runOpenStart = (run: PlayRun) => parts.some((part) => part.openStart && part.startMs === run.start_ms);
  const runOpenEnd = (run: PlayRun) => parts.some((part) => part.openEnd && part.endMs === run.end_ms);

  // Sessions that began on an earlier day get a quiet line that leads back
  // to their entry, one per session however many parts it has here.
  const startedToday = new Set(day.sessionIds);
  const continued = [...new Map(parts.map((part) => [part.session.id, part.session])).values()]
    .filter((session) => !startedToday.has(session.id))
    .sort((a, b) => a.started_at_wall.localeCompare(b.started_at_wall));

  /** Pointing at or focusing a session's line marks all its parts, on this day and others. */
  const linkProps = (sessionId: string) => ({
    onMouseEnter: () => onLink(sessionId),
    onMouseLeave: () => onLink(null),
    onFocus: () => onLink(sessionId),
    onBlur: () => onLink(null),
  });
  const showCard = (sessionId: string, anchor: Element) => {
    if (!frame.current) return;
    onLink(sessionId);
    setCard({ sessionId, style: cardPosition(anchor.getBoundingClientRect(), frame.current.getBoundingClientRect()) });
  };
  const cardSession = card ? sessionsById.get(card.sessionId) : undefined;
  // Outlines fade out past an open end, like the play they mark.
  const outlineMask = (part: StripPart) => {
    const from = part.openStart ? `transparent, black ${BLEED_PX}px` : "black";
    const to = part.openEnd ? `black calc(100% - ${BLEED_PX}px), transparent` : "black";
    return `linear-gradient(to right, ${from}, ${to})`;
  };

  return (
    <section className="flex flex-col gap-5 border-b border-rule py-7 xl:flex-row xl:gap-10">
      <div className="flex shrink-0 items-baseline gap-4 xl:block xl:w-[190px]">
        <h2 className="font-display text-4xl leading-none">{formatDate(day.start, { weekday: "long" })}</h2>
        <div className="text-xs tracking-[0.12em] text-faint uppercase xl:mt-1.5">
          {formatDate(day.start, { day: "numeric", month: "long" })}
        </div>
        <div className="ml-auto font-mono text-[22px] tabular-nums xl:mt-3.5 xl:ml-0">{formatHoursMinutes(day.playedMs)}</div>
      </div>

      <div className="flex min-w-0 flex-1 flex-col gap-3.5">
        <div>
          <div ref={frame} className="relative">
            <div aria-hidden="true" className="relative h-2.5 rounded-full bg-raised">
              {day.runs.map((run) => (
                <PlayBlock
                  key={run.start_ms}
                  run={run}
                  day={day.start}
                  fillOf={fillOf}
                  markOf={markOf}
                  openStart={runOpenStart(run)}
                  openEnd={runOpenEnd(run)}
                />
              ))}
              {parts
                .filter((part) => part.session.id === linked)
                .map((part) => (
                  <span
                    key={`outline-${part.place.left}`}
                    className={cn(
                      "pointer-events-none absolute border-[1.5px]",
                      part.openStart ? "border-l-0" : "rounded-l-full",
                      part.openEnd ? "border-r-0" : "rounded-r-full",
                    )}
                    style={{
                      top: -OUTLINE_GAP_PX,
                      bottom: -OUTLINE_GAP_PX,
                      left: part.openStart ? -BLEED_PX : `calc(${part.place.left}% - ${OUTLINE_GAP_PX}px)`,
                      right: part.openEnd
                        ? -BLEED_PX
                        : `calc(${100 - part.place.left - part.place.width}% - ${OUTLINE_GAP_PX}px)`,
                      borderColor: markOf(part.session.game_id),
                      maskImage: outlineMask(part),
                    }}
                  />
                ))}
              {/* Shorter parts on top, so a session beside a long one can still be pointed at. */}
              {[...parts]
                .sort((a, b) => b.place.width - a.place.width)
                .map((part) => (
                  <span
                    key={`hit-${part.session.id}-${part.place.left}`}
                    className="absolute"
                    style={{
                      left: `${part.place.left}%`,
                      width: `${part.place.width}%`,
                      top: -HIT_SLOP_PX,
                      bottom: -HIT_SLOP_PX,
                    }}
                    onMouseEnter={(event) => showCard(part.session.id, event.currentTarget)}
                    onMouseLeave={() => {
                      onLink(null);
                      setCard(null);
                    }}
                  />
                ))}
            </div>
            <div aria-hidden="true" className="mt-1.5 flex justify-between font-mono text-[10px] text-faint">
              {TICKS.map((hour) => (
                <span key={hour}>{String(hour).padStart(2, "0")}</span>
              ))}
            </div>
            {card && cardSession && (
              <SessionCard
                session={cardSession}
                shares={sharesOf(cardSession.id)}
                day={day.start}
                title={titleOf(cardSession.game_id)}
                mark={markOf(cardSession.game_id)}
                style={card.style}
              />
            )}
          </div>
          {day.groups.length > 0 && (
            <div className="mt-2.5 flex flex-col gap-1.5">
              {day.groups.map((group) => (
                <p key={group.game_ids.join()} className="flex items-center gap-2.5 text-[13px] text-faint">
                  <span
                    aria-hidden="true"
                    className="h-2.5 w-6 shrink-0 rounded-full"
                    style={{ backgroundImage: hatch(group.game_ids.map(markOf)) }}
                  />
                  {sideBySideSentence(group, titleOf)}
                </p>
              ))}
            </div>
          )}
        </div>

        {continued.map((session) => {
          const shares = sharesOf(session.id);
          const share = shares.find((each) => each.day === dayKey);
          if (!share) return null;
          const phrase = continuedPhrase(session, titleOf(session.game_id), day.start, { throughDay: share.into_next });
          const mark = markOf(session.game_id);
          const started = parseVaultimeDate(session.started_at_wall);
          const ended = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : null;
          return (
            <article
              key={`continued-${session.id}`}
              id={entryId(session.id, dayKey)}
              tabIndex={-1}
              {...linkProps(session.id)}
              className="flex items-baseline gap-5 outline-none"
            >
              <span
                className="w-[20ch] shrink-0 font-mono text-[13px] text-faint transition-colors"
                style={linked === session.id ? { color: mark } : undefined}
              >
                {formatSpanOnDay(started, ended, day.start)}
              </span>
              <div className="min-w-0 flex-1">
                <p className="font-prose text-[19px] leading-snug text-pretty text-soft">
                  <PhraseText phrase={{ before: phrase.before, em: phrase.em }} emColor={mark} emFill={fillFor(session.game_id)} />
                  {phrase.middle}
                  {/* A link is a box of its own, so the stop after it is kept on its line. */}
                  <span className="whitespace-nowrap">
                    <JumpLink
                      label={`${phrase.link}, where this session began`}
                      onClick={() => onJump(session.id, dayOf(shares[0]))}
                    >
                      {phrase.link}
                    </JumpLink>
                    {!phrase.after && "."}
                  </span>
                  {phrase.after && `${phrase.after}.`}
                </p>
                <div className="mt-1 text-[13px] text-faint">
                  {continuedAmounts(share.runtime_ms, session.runtime_ms, !session.ended_at_wall)}
                </div>
              </div>
            </article>
          );
        })}
        {[
          ...day.sessionIds.flatMap((id) => {
            const session = sessionsById.get(id);
            return session ? [{ at: session.started_at_wall, session, change: null }] : [];
          }),
          ...day.changes.map((change) => ({ at: change.changed_at, session: null, change })),
        ]
          .sort((a, b) => a.at.localeCompare(b.at))
          .map(({ session, change }) => {
            if (change) {
              if (change.status === "none") return null;
              const title = titleOf(change.game_id);
              return (
                <article key={change.id} className="flex items-baseline gap-5">
                  <span className="w-[20ch] shrink-0 font-mono text-[13px] text-faint">
                    {formatClockTime(parseVaultimeDate(change.changed_at))}
                  </span>
                  <p className="font-prose flex min-w-0 flex-1 items-baseline gap-2.5 text-[20px] leading-snug">
                    <GameStatusIcon status={change.status} className="size-4 shrink-0 translate-y-0.5 text-violet" />
                    <span>
                      <PhraseText
                        phrase={statusSentence(change.status, title, playedBefore(change.game_id, change.changed_at))}
                      />
                    </span>
                  </p>
                </article>
              );
            }
            if (!session) return null;
            const later = sharesOf(session.id).slice(1);
            const laterMs = later.reduce((sum, share) => sum + share.runtime_ms, 0);

            const ended = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : null;
            return (
              <div
                key={session.id}
                id={entryId(session.id, dayKey)}
                tabIndex={-1}
                {...linkProps(session.id)}
                className="outline-none"
              >
                <SessionLine
                  session={session}
                  events={events}
                  gameTitle={titleOf(session.game_id)}
                  titleColor={markOf(session.game_id)}
                  titleFill={fillFor(session.game_id)}
                  gameSessions={sessionsByGame.get(session.game_id)}
                  earlierMs={byGame.get(session.game_id)?.earlier?.earlier_ms}
                  when={formatSpanOnDay(parseVaultimeDate(session.started_at_wall), ended, day.start)}
                  highlighted={linked === session.id}
                  amountsNote={
                    later.length > 0 && (
                      <>
                        {afterMidnightLead(laterMs)}
                        <span className="whitespace-nowrap">
                          <DayLinks shares={later} onJump={(target) => onJump(session.id, target)} />.
                        </span>
                      </>
                    )
                  }
                  bordered={false}
                  note={notes[session.id]}
                  onSaveNote={(note) => onSaveNote(session.id, note)}
                  onCorrected={onCorrected}
                />
              </div>
            );
          })}
      </div>
    </section>
  );
});
