// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Draws the year card on a canvas at the exact size of its format. Colors
// come from the player's look through lib/theme.ts, sizes are the card's own
// type scale, the way index.css holds the app's.

import { oklchToHex, type Oklch } from "@/lib/color";
import { COVER_ASPECT, HOUR_MS, MINUTE_MS, SHARE_BACKDROP_SHRINK, SHARE_FORMATS, TINT_LEVELS } from "@/lib/constants";
import type { Phrase } from "@/lib/sentences";
import {
  calendarGrid,
  gamesFor,
  stackBlocks,
  type ShareCard,
  type ShareGame,
  type ShareOptions,
  type StackBlock,
} from "@/lib/share-card";
import { accentColor, groundColor, type GroundId, type ThemeMode } from "@/lib/theme";
import { formatDate, formatHoursMinutes, formatHoursShort } from "@/lib/time";

/** The player's look the card takes its colors from. */
export interface CardLook {
  ground: GroundId;
  accent: string;
}

const MONA = "'Mona Sans Variable', system-ui, sans-serif";
const GEIST = "'Geist Variable', system-ui, sans-serif";
const MONO = "'Geist Mono Variable', ui-monospace, monospace";

/** Accent shares of the calendar's heat levels, as the YearHeatmap classes have them. */
const HEAT_ALPHAS = [0, 0.2, 0.35, 0.55, 0.75, 1];
/** Days still to come show the empty color this faintly, as bg-raised/40 does. */
const FUTURE_ALPHA = 0.4;
/** A row of covers is laid out for at least this many, so one game does not get a giant cover. */
const FEWEST_COVER_SLOTS = 3;
/** Share of the ground over the blurred cover behind the card, at its top and where it ends, per format and mode. */
const BACKDROP_DIM = {
  wide: { dark: [0.8, 0.9], light: [0.84, 0.92] },
  square: { dark: [0.78, 0.92], light: [0.82, 0.94] },
  story: { dark: [0.6, 1], light: [0.7, 1] },
} as const;

/** The card's type scale and spacing per format. */
const SCALES = {
  wide: { pad: 60, header: 13, year: 132, prose: 24, statValue: 30, statLabel: 11, small: 13, caption: 15, gap: 18 },
  square: { pad: 72, header: 15, year: 196, prose: 31, statValue: 36, statLabel: 12, small: 15, caption: 17, gap: 26 },
  story: { pad: 84, header: 19, year: 300, prose: 46, statValue: 44, statLabel: 15, small: 21, caption: 34, gap: 36 },
} as const;

type Scale = (typeof SCALES)[keyof typeof SCALES];

interface Colors {
  ground: string;
  raised: string;
  hairline: string;
  idle: string;
  faint: string;
  soft: string;
  text: string;
  accent: string;
  mode: ThemeMode;
  groundOklch: Oklch;
}

function cardColors(mode: ThemeMode, look: CardLook): Colors {
  const tone = (role: Parameters<typeof groundColor>[2]) => oklchToHex(groundColor(mode, look.ground, role));
  return {
    ground: tone("ink"),
    raised: tone("raised"),
    hairline: tone("hairline-strong"),
    idle: tone("idle"),
    faint: tone("faint"),
    soft: tone("soft"),
    text: tone("text"),
    accent: oklchToHex(accentColor(look.accent, mode)),
    mode,
    groundOklch: groundColor(mode, look.ground, "ink"),
  };
}

const images = new Map<string, Promise<HTMLImageElement | null>>();

/** A decoded image, null when it cannot be read. Covers are data URLs or same origin files, so the canvas stays clean. */
function loadImage(src: string): Promise<HTMLImageElement | null> {
  let image = images.get(src);
  if (!image) {
    image = (async () => {
      try {
        const element = new Image();
        element.decoding = "async";
        element.src = src;
        await element.decode();
        return element;
      } catch {
        return null;
      }
    })();
    images.set(src, image);
  }
  return image;
}

/** Waits for the card's fonts, so the first drawing does not use a fallback. */
async function cardFontsReady(): Promise<void> {
  if (typeof document === "undefined" || !document.fonts) return;
  await Promise.all(
    [`500 64px ${MONA}`, `italic 450 64px ${MONA}`, `400 64px ${GEIST}`, `400 64px ${MONO}`].map((font) =>
      document.fonts.load(font).catch(() => []),
    ),
  );
  await document.fonts.ready;
}

/** Draws `card` on `canvas`, which takes the size of the format. */
export async function drawShareCard(
  canvas: HTMLCanvasElement,
  card: ShareCard,
  options: ShareOptions,
  look: CardLook,
): Promise<void> {
  await cardFontsReady();
  const games = gamesFor(card, options.format);
  const covers = new Map<string, HTMLImageElement | null>();
  await Promise.all(
    games.map(async (game) => covers.set(game.gameId, game.cover ? await loadImage(game.cover) : null)),
  );

  const { width, height } = SHARE_FORMATS[options.format];
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const painter = new Painter(ctx, cardColors(options.mode, look), SCALES[options.format]);
  painter.background(width, height, games[0] ? (covers.get(games[0].gameId) ?? null) : null, options.format);
  const layout = { wide: drawWide, square: drawSquare, story: drawStory }[options.format];
  layout(painter, card, options, games, covers, width, height);
}

type Layout = (
  painter: Painter,
  card: ShareCard,
  options: ShareOptions,
  games: ShareGame[],
  covers: Map<string, HTMLImageElement | null>,
  width: number,
  height: number,
) => void;

/** One block of a column: how tall it wants to be, and how to draw it into the space it gets. */
interface Block extends StackBlock {
  draw: (top: number, height: number) => void;
}

function stack(blocks: Block[], top: number, bottom: number, gap: number, align?: "spread" | "start" | "center") {
  stackBlocks(blocks, top, bottom, gap, align).forEach((place, index) => blocks[index].draw(place.top, place.height));
}

/** Whether the card names its games, or only counts them. */
function showsCovers(games: ShareGame[]): boolean {
  return games.some((game) => game.title !== null);
}

/** The game the sentence names, so its title takes its mark. */
function emGame(card: ShareCard, games: ShareGame[]): ShareGame | undefined {
  return card.sentence.em && games[0]?.title === card.sentence.em ? games[0] : undefined;
}

const drawWide: Layout = (p, card, options, games, covers, width, height) => {
  const s = p.scale;
  const inner = width - 2 * s.pad;
  p.header(card, s.pad, s.pad, inner);
  const rightWidth = Math.round(inner * 0.44);
  const leftWidth = inner - rightWidth - s.gap * 3;
  const right = s.pad + inner - rightWidth;
  const left: Block[] = [
    p.yearBlock(card.year, s.pad),
    p.sentenceBlock(card.sentence, s.pad, leftWidth, 2, emGame(card, games)),
    p.statsBlock([activeStat(card), daysStat(card), streakStat(card), monthStat(card, true)], s.pad, leftWidth),
    p.smallPrintBlock(smallPrint(card, false), s.pad, leftWidth),
  ];
  const gamesBlock = p.gamesBlock(card, games, covers, right, rightWidth, "row");
  const leftHeight = left.reduce((sum, block) => sum + block.height, 0) + (left.length - 1) * s.gap;
  // Both columns hang from the same top line, the calendar sits below them.
  const columns: Block = {
    height: Math.max(leftHeight, gamesBlock.height),
    maxHeight: Math.max(leftHeight, gamesBlock.maxHeight ?? gamesBlock.height),
    draw: (top, columnsHeight) => {
      stack(left, top, top + columnsHeight, s.gap, "start");
      stack([gamesBlock], top, top + columnsHeight, s.gap, showsCovers(games) ? "start" : "center");
    },
  };
  const blocks = [columns];
  if (options.showCalendar) blocks.push(p.calendarBlock(card, s.pad, inner));
  stack(blocks, s.pad + s.header * 2 + s.gap * 1.5, height - s.pad, s.gap * 2, "center");
};

const drawSquare: Layout = (p, card, options, games, covers, width, height) => {
  const s = p.scale;
  const inner = width - 2 * s.pad;
  p.header(card, s.pad, s.pad, inner);
  const blocks: Block[] = [
    p.yearBlock(card.year, s.pad),
    p.sentenceBlock(card.sentence, s.pad, inner, 2, emGame(card, games)),
    p.statsBlock([activeStat(card), daysStat(card), streakStat(card), monthStat(card, false)], s.pad, inner),
    p.gamesBlock(card, games, covers, s.pad, inner, "row"),
  ];
  if (options.showCalendar) blocks.push(p.calendarBlock(card, s.pad, inner));
  blocks.push(p.smallPrintBlock(smallPrint(card, true), s.pad, inner));
  stack(blocks, s.pad + s.header * 2 + s.gap, height - s.pad, s.gap);
};

const drawStory: Layout = (p, card, options, games, covers, width, height) => {
  const s = p.scale;
  const inner = width - 2 * s.pad;
  p.header(card, s.pad, s.pad, inner);
  const blocks: Block[] = [
    p.yearBlock(card.year, s.pad),
    p.sentenceBlock(card.sentence, s.pad, inner, 3, emGame(card, games)),
    p.heroBlock(card, s.pad, inner),
    p.statsBlock([daysStat(card), streakStat(card), monthStat(card, false)], s.pad, inner),
    p.gamesBlock(card, games, covers, s.pad, inner, "list"),
  ];
  if (options.showCalendar) blocks.push(p.calendarBlock(card, s.pad, inner));
  stack(blocks, s.pad + s.header * 2 + s.gap, height - s.pad, s.gap);
};

interface Stat {
  label: string;
  value: string;
  /** Faint words after the value. */
  unit?: string;
  accent?: boolean;
}

/** Active play in whole hours, or in minutes for less than an hour. */
function activeAmount(card: ShareCard): { value: string; unit: string; words: string } {
  if (card.activeMs < HOUR_MS) {
    const minutes = Math.floor(card.activeMs / MINUTE_MS);
    return { value: String(minutes), unit: "min", words: minutes === 1 ? "minute" : "minutes" };
  }
  const hours = Math.floor(card.activeMs / HOUR_MS);
  return { value: String(hours), unit: "h", words: hours === 1 ? "hour" : "hours" };
}

function activeStat(card: ShareCard): Stat {
  const { value, unit } = activeAmount(card);
  return { label: "Active play", value, unit, accent: true };
}

function daysStat(card: ShareCard): Stat {
  return { label: "Days played", value: String(card.daysPlayed), unit: `of ${card.daysOfYear}` };
}

function streakStat(card: ShareCard): Stat {
  const days = card.longestStreakDays;
  return { label: "Longest streak", value: String(days), unit: days === 1 ? "day" : "days" };
}

function monthStat(card: ShareCard, short: boolean): Stat {
  const month = card.busiestMonth;
  const name = month
    ? formatDate(new Date(card.year, month.month, 1), { month: short ? "short" : "long" })
    : "None";
  return { label: "Busiest month", value: name };
}

/** "212 h with a game running, 30 h idle. Longest session 6 h 12 in Elden Ring." */
function smallPrint(card: ShareCard, withSession: boolean): string {
  const running = `${formatHoursShort(card.playedMs)} with a game running, ${formatHoursShort(card.idleMs)} idle.`;
  const longest = card.longestSession;
  if (!withSession || !longest) return running;
  const where = longest.title ? ` in ${longest.title}` : "";
  return `${running} Longest session ${formatHoursMinutes(longest.runtimeMs)}${where}.`;
}

/** Drawing primitives for one card. */
class Painter {
  readonly ctx: CanvasRenderingContext2D;
  readonly colors: Colors;
  readonly scale: Scale;

  constructor(ctx: CanvasRenderingContext2D, colors: Colors, scale: Scale) {
    this.ctx = ctx;
    this.colors = colors;
    this.scale = scale;
  }

  font(family: string, size: number, weight: number, { stretch = false, italic = false, tracking = 0 } = {}) {
    const ctx = this.ctx;
    ctx.font = `${italic ? "italic " : ""}${weight} ${stretch ? "expanded " : ""}${size}px ${family}`;
    // The width axis of Mona Sans, where the canvas has a separate setting for it.
    if (hasFontStretch(ctx)) ctx.fontStretch = stretch ? "expanded" : "normal";
    if (hasLetterSpacing(ctx)) ctx.letterSpacing = `${tracking * size}px`;
  }

  background(width: number, height: number, cover: HTMLImageElement | null, format: ShareOptions["format"]) {
    const { ctx, colors } = this;
    ctx.fillStyle = colors.ground;
    ctx.fillRect(0, 0, width, height);
    // The top game's cover, blurred, behind the top of the card.
    const band = format === "story" ? height * 0.55 : height;
    if (cover) {
      ctx.save();
      ctx.beginPath();
      ctx.rect(0, 0, width, band);
      ctx.clip();
      drawBlurred(ctx, cover, width, band);
      ctx.restore();
      const fade = ctx.createLinearGradient(0, 0, 0, band);
      const [dimTop, dimEnd] = BACKDROP_DIM[format][colors.mode];
      fade.addColorStop(0, withAlpha(colors.groundOklch, dimTop));
      fade.addColorStop(1, withAlpha(colors.groundOklch, dimEnd));
      ctx.fillStyle = fade;
      ctx.fillRect(0, 0, width, band);
    }
    // A glow of the accent from the top corner.
    const glow = ctx.createRadialGradient(width, 0, 0, width, 0, Math.max(width, height) * 0.8);
    glow.addColorStop(0, hexAlpha(colors.accent, cover ? 0.14 : 0.2));
    glow.addColorStop(1, hexAlpha(colors.accent, 0));
    ctx.fillStyle = glow;
    ctx.fillRect(0, 0, width, height);
  }

  header(card: ShareCard, x: number, y: number, width: number) {
    const { ctx, colors, scale } = this;
    const label = [card.current ? "Year in play so far" : "Year in play", card.pcName].filter(Boolean).join("  ·  ");
    this.font(GEIST, scale.header, 500);
    ctx.fillStyle = colors.faint;
    ctx.textBaseline = "middle";
    trackedText(ctx, label.toUpperCase(), x, y + scale.header, scale.header * 0.14, width * 0.6);
    this.logo(x + width, y + scale.header, scale.header * 2);
    ctx.textBaseline = "alphabetic";
  }

  /** The logo mark and the wordmark, ending at `right`, centered on `middle`. */
  logo(right: number, middle: number, size: number) {
    const { ctx, colors } = this;
    this.font(MONA, size * 0.86, 700, { stretch: true, tracking: -0.03 });
    ctx.textBaseline = "middle";
    ctx.textAlign = "right";
    ctx.fillStyle = colors.text;
    ctx.fillText("Vaultime", right, middle + size * 0.04);
    const textWidth = ctx.measureText("Vaultime").width;
    ctx.textAlign = "left";
    const markLeft = right - textWidth - size * 0.36 - size;
    drawLogoMark(ctx, markLeft, middle - size / 2, size, colors.text, colors.accent);
  }

  yearBlock(year: number, x: number): Block {
    const { ctx, colors, scale } = this;
    this.font(MONA, scale.year, 500, { stretch: true, tracking: -0.015 });
    const metrics = ctx.measureText(String(year));
    const height = metrics.actualBoundingBoxAscent || scale.year * 0.72;
    return {
      height,
      draw: (top) => {
        this.font(MONA, scale.year, 500, { stretch: true, tracking: -0.015 });
        ctx.fillStyle = colors.text;
        ctx.fillText(String(year), x, top + height);
      },
    };
  }

  sentenceBlock(phrase: Phrase, x: number, width: number, maxLines: number, game?: ShareGame): Block {
    const { scale } = this;
    const lineHeight = scale.prose * 1.3;
    const lines = this.wrapPhrase(phrase, width, maxLines);
    return {
      height: lines.length * lineHeight - (lineHeight - scale.prose),
      draw: (top) => {
        lines.forEach((line, index) => this.drawRuns(line, x, top + scale.prose * 0.8 + index * lineHeight, game));
      },
    };
  }

  /** The phrase cut into lines of word runs, the italic part kept italic. */
  wrapPhrase(phrase: Phrase, width: number, maxLines: number): Run[][] {
    const words: Run[] = [];
    const add = (text: string | undefined, italic: boolean) => {
      for (const word of (text ?? "").split(/(\s+)/)) if (word) words.push({ text: word, italic });
    };
    add(phrase.before, false);
    add(phrase.em, true);
    add(phrase.after, false);
    const lines: Run[][] = [[]];
    let used = 0;
    for (const word of words) {
      const line = lines[lines.length - 1];
      const space = /^\s+$/.test(word.text);
      if (space && line.length === 0) continue;
      const wordWidth = this.runWidth(word);
      if (!space && used + wordWidth > width && line.length > 0) {
        while (line.length > 0 && /^\s+$/.test(line[line.length - 1].text)) line.pop();
        lines.push([word]);
        used = wordWidth;
      } else {
        line.push(word);
        used += wordWidth;
      }
    }
    if (lines.length > maxLines) {
      const kept = lines.slice(0, maxLines);
      const last = kept[maxLines - 1];
      last.push({ text: "…", italic: last[last.length - 1]?.italic ?? false });
      while (last.length > 1 && this.lineWidth(last) > width) last.splice(last.length - 2, 1);
      return kept;
    }
    return lines;
  }

  proseFont(italic: boolean) {
    this.font(MONA, this.scale.prose, 450, { italic, tracking: -0.01 });
  }

  runWidth(run: Run): number {
    this.proseFont(run.italic);
    return this.ctx.measureText(run.text).width;
  }

  lineWidth(line: Run[]): number {
    return line.reduce((sum, run) => sum + this.runWidth(run), 0);
  }

  /** A line of the sentence, the italic part in the mark of `game` when it names one. */
  drawRuns(line: Run[], x: number, baseline: number, game?: ShareGame) {
    const { ctx, colors } = this;
    const lefts = line.map((_, index) => x + this.lineWidth(line.slice(0, index)));
    // One blend across the italic words of the line, not one per word.
    const italic = line.flatMap((run, index) => (run.italic ? [index] : []));
    const from = italic.length > 0 ? lefts[italic[0]] : x;
    const to = italic.length > 0 ? lefts[italic[italic.length - 1]] + this.runWidth(line[italic[italic.length - 1]]) : x;
    const emFill = game ? this.markFill(game, from, to) : colors.text;
    line.forEach((run, index) => {
      this.proseFont(run.italic);
      ctx.fillStyle = run.italic ? emFill : colors.soft;
      ctx.fillText(run.text, lefts[index], baseline);
    });
  }

  statsBlock(stats: Stat[], x: number, width: number): Block {
    const { ctx, colors, scale } = this;
    const valueHeight = scale.statValue * 0.74;
    const height = scale.statLabel + scale.statLabel * 1.1 + valueHeight + scale.statLabel * 0.9;
    return {
      height,
      draw: (top) => {
        const column = width / stats.length;
        ctx.fillStyle = colors.hairline;
        ctx.fillRect(x, top, width, 1);
        stats.forEach((stat, index) => {
          const left = x + index * column;
          this.font(GEIST, scale.statLabel, 500);
          ctx.fillStyle = colors.faint;
          const labelBaseline = top + scale.statLabel * 2;
          trackedText(ctx, stat.label.toUpperCase(), left, labelBaseline, scale.statLabel * 0.14, column - scale.gap);
          const baseline = labelBaseline + scale.statLabel * 0.9 + valueHeight;
          this.font(MONO, scale.statValue, 450, { tracking: -0.02 });
          ctx.fillStyle = stat.accent ? colors.accent : colors.text;
          const value = fitText(ctx, stat.value, column - scale.gap);
          ctx.fillText(value, left, baseline);
          if (stat.unit) {
            const valueWidth = ctx.measureText(value).width;
            this.font(MONO, scale.statValue * 0.5, 400);
            ctx.fillStyle = colors.faint;
            const unit = ` ${stat.unit}`;
            if (valueWidth + ctx.measureText(unit).width <= column - scale.gap) {
              ctx.fillText(unit, left + valueWidth, baseline);
            }
          }
        });
      },
    };
  }

  smallPrintBlock(text: string, x: number, width: number): Block {
    const { ctx, colors, scale } = this;
    this.font(GEIST, scale.small, 400);
    const lines = wrapWords(ctx, text, width, 2);
    const lineHeight = scale.small * 1.45;
    return {
      height: lines.length * lineHeight,
      draw: (top) => {
        this.font(GEIST, scale.small, 400);
        ctx.fillStyle = colors.faint;
        lines.forEach((line, index) => ctx.fillText(line, x, top + scale.small + index * lineHeight));
      },
    };
  }

  /** The big number of active play with its small print, for the story. */
  heroBlock(card: ShareCard, x: number, width: number): Block {
    const { ctx, colors, scale } = this;
    const size = scale.year * 0.5;
    const numberHeight = size * 0.74;
    const small = this.smallPrintBlock(smallPrint(card, true), x, width);
    const height = numberHeight + scale.prose * 1.4 + small.height;
    return {
      height,
      draw: (top) => {
        const amount = activeAmount(card);
        this.font(MONO, size, 450, { tracking: -0.04 });
        ctx.fillStyle = colors.accent;
        ctx.fillText(amount.value, x, top + numberHeight);
        const numberWidth = ctx.measureText(amount.value).width;
        this.proseFont(false);
        ctx.fillStyle = colors.soft;
        ctx.fillText(`${amount.words} of active play`, x + numberWidth + scale.prose * 0.5, top + numberHeight);
        small.draw(top + numberHeight + scale.prose * 0.9, small.height);
      },
    };
  }

  calendarHeight(year: number, width: number): number {
    return calendarGrid(year, width).height + this.scale.small * 1.6;
  }

  calendarBlock(card: ShareCard, x: number, width: number): Block {
    return { height: this.calendarHeight(card.year, width), draw: (top) => this.calendar(card, x, top, width) };
  }

  calendar(card: ShareCard, x: number, top: number, width: number) {
    const { ctx, colors, scale } = this;
    const grid = calendarGrid(card.year, width);
    const labelSize = scale.small * 0.85;
    this.font(MONO, labelSize, 400);
    ctx.fillStyle = colors.faint;
    grid.monthColumns.forEach((column, month) => {
      const name = formatDate(new Date(card.year, month, 1), { month: "short" });
      ctx.fillText(name, x + column * grid.pitch, top + labelSize);
    });
    const gridTop = top + scale.small * 1.6;
    const radius = grid.cell * 0.18;
    card.days.forEach((level, index) => {
      const place = grid.places[index];
      const left = x + place.column * grid.pitch;
      const cellTop = gridTop + place.row * grid.pitch;
      ctx.globalAlpha = level < 0 ? FUTURE_ALPHA : 1;
      ctx.fillStyle = colors.raised;
      roundedRect(ctx, left, cellTop, grid.cell, grid.cell, radius);
      ctx.fill();
      if (level > 0) {
        ctx.globalAlpha = HEAT_ALPHAS[level] ?? 1;
        ctx.fillStyle = colors.accent;
        ctx.fill();
      }
    });
    ctx.globalAlpha = 1;
  }

  /** The games, as covers in a row or as a list, or without titles a count and a bar of shares. */
  gamesBlock(
    card: ShareCard,
    games: ShareGame[],
    covers: Map<string, HTMLImageElement | null>,
    x: number,
    width: number,
    shape: "row" | "list",
  ): Block {
    if (!showsCovers(games)) return this.privateGamesBlock(card, games, x, width);
    return shape === "row" ? this.coverRow(games, covers, x, width) : this.gameList(games, covers, x, width);
  }

  coverRow(games: ShareGame[], covers: Map<string, HTMLImageElement | null>, x: number, width: number): Block {
    const { ctx, colors, scale } = this;
    // A few games get larger covers, never more than a row of three.
    const slots = Math.max(games.length, FEWEST_COVER_SLOTS);
    const gap = scale.gap;
    const maxCoverWidth = (width - gap * (slots - 1)) / slots;
    const caption = scale.caption * 3.1;
    const maxHeight = maxCoverWidth / COVER_ASPECT + caption;
    return {
      height: maxHeight * 0.78,
      maxHeight,
      draw: (top, height) => {
        const coverHeight = height - caption;
        const coverWidth = Math.min(maxCoverWidth, coverHeight * COVER_ASPECT);
        games.forEach((game, index) => {
          const left = x + index * (coverWidth + gap);
          this.cover(game, covers.get(game.gameId) ?? null, left, top, coverWidth, coverWidth / COVER_ASPECT);
          const captionTop = top + coverWidth / COVER_ASPECT + scale.caption * 1.4;
          this.font(MONO, scale.caption * 0.8, 400);
          ctx.fillStyle = colors.faint;
          const rank = String(index + 1).padStart(2, "0");
          ctx.fillText(rank, left, captionTop);
          const rankWidth = ctx.measureText(`${rank}  `).width;
          this.font(GEIST, scale.caption, 500);
          this.markedTitle(game, left + rankWidth, captionTop, coverWidth - rankWidth);
          this.font(MONO, scale.caption * 0.85, 400);
          ctx.fillStyle = colors.soft;
          ctx.fillText(formatHoursShort(game.activeMs), left + rankWidth, captionTop + scale.caption * 1.35);
        });
      },
    };
  }

  gameList(games: ShareGame[], covers: Map<string, HTMLImageElement | null>, x: number, width: number): Block {
    const { ctx, colors, scale } = this;
    const minRow = scale.caption * 3.4;
    const maxRow = scale.caption * 5.2;
    const gap = scale.gap * 0.6;
    const total = (row: number) => games.length * row + (games.length - 1) * gap;
    const topShare = games[0]?.share || 1;
    return {
      height: total(minRow),
      maxHeight: total(maxRow),
      draw: (top, height) => {
        const row = (height - (games.length - 1) * gap) / games.length;
        games.forEach((game, index) => {
          const rowTop = top + index * (row + gap);
          const coverWidth = row * COVER_ASPECT;
          this.cover(game, covers.get(game.gameId) ?? null, x, rowTop, coverWidth, row);
          const textLeft = x + coverWidth + scale.gap;
          const textWidth = width - coverWidth - scale.gap;
          const middle = rowTop + row / 2;
          this.font(MONO, scale.caption * 0.6, 400);
          ctx.fillStyle = colors.faint;
          ctx.fillText(String(index + 1).padStart(2, "0"), textLeft, middle - scale.caption * 0.75);
          this.font(MONO, scale.caption * 0.8, 450);
          ctx.fillStyle = colors.soft;
          ctx.textAlign = "right";
          const hours = formatHoursShort(game.activeMs);
          ctx.fillText(hours, x + width, middle + scale.caption * 0.35);
          const hoursWidth = ctx.measureText(hours).width;
          ctx.textAlign = "left";
          this.font(MONA, scale.caption, 500, { tracking: -0.01 });
          this.markedTitle(game, textLeft, middle + scale.caption * 0.35, textWidth - hoursWidth - scale.gap);
          const barWidth = (textWidth - hoursWidth - scale.gap) * (game.share / topShare);
          this.markBar(game, textLeft, middle + scale.caption * 0.85, barWidth, Math.max(3, scale.caption * 0.12));
        });
      },
    };
  }

  /** Without titles: how many games, and how the time split between them. */
  privateGamesBlock(card: ShareCard, games: ShareGame[], x: number, width: number): Block {
    const { ctx, colors, scale } = this;
    const countSize = scale.year * 0.42;
    const barHeight = Math.max(6, scale.caption * 0.5);
    this.font(GEIST, scale.small, 400);
    const note = `The game you played most took ${Math.round(card.topShare * 100)} % of your active time.`;
    const noteLines = wrapWords(ctx, note, width, 2);
    const height = scale.statLabel * 2 + countSize * 0.74 + scale.gap + barHeight + scale.gap * 0.8 + noteLines.length * scale.small * 1.45;
    return {
      height,
      draw: (top) => {
        this.font(GEIST, scale.statLabel, 500);
        ctx.fillStyle = colors.faint;
        trackedText(ctx, "GAMES", x, top + scale.statLabel, scale.statLabel * 0.14, width);
        const baseline = top + scale.statLabel * 2 + countSize * 0.74;
        this.font(MONA, countSize, 500, { stretch: true, tracking: -0.015 });
        ctx.fillStyle = colors.text;
        const count = String(card.gamesCount);
        ctx.fillText(count, x, baseline);
        const countWidth = ctx.measureText(count).width;
        this.proseFont(false);
        ctx.fillStyle = colors.soft;
        ctx.fillText(card.gamesCount === 1 ? "game played" : "games played", x + countWidth + scale.prose * 0.45, baseline);
        const barTop = baseline + scale.gap;
        let left = x;
        ctx.fillStyle = colors.idle;
        roundedRect(ctx, x, barTop, width, barHeight, barHeight / 2);
        ctx.fill();
        for (const game of games) {
          const segment = width * game.share;
          this.markBar(game, left, barTop, Math.max(0, segment - 3), barHeight);
          left += segment;
        }
        this.font(GEIST, scale.small, 400);
        ctx.fillStyle = colors.faint;
        noteLines.forEach((line, index) =>
          ctx.fillText(line, x, barTop + barHeight + scale.gap * 0.8 + scale.small + index * scale.small * 1.45),
        );
      },
    };
  }

  /** A title in its mark, a gradient for art with two colors. */
  markedTitle(game: ShareGame, x: number, baseline: number, width: number) {
    const { ctx } = this;
    const title = fitText(ctx, game.title ?? "", width);
    ctx.fillStyle = this.markFill(game, x, x + ctx.measureText(title).width);
    ctx.fillText(title, x, baseline);
  }

  markBar(game: ShareGame, x: number, top: number, width: number, height: number) {
    if (width <= 0) return;
    const { ctx } = this;
    ctx.fillStyle = this.markFill(game, x, x + width);
    roundedRect(ctx, x, top, width, height, height / 2);
    ctx.fill();
  }

  markFill(game: ShareGame, from: number, to: number): string | CanvasGradient {
    const [first, second] = game.marks.map(oklchToHex);
    if (!first) return this.colors.soft;
    if (!second || to <= from) return first;
    const gradient = this.ctx.createLinearGradient(from, 0, to, 0);
    gradient.addColorStop(0, first);
    gradient.addColorStop(1, second);
    return gradient;
  }

  /** A cover, or a typeset one in the game's hue when it has no art. */
  cover(game: ShareGame, image: HTMLImageElement | null, x: number, y: number, width: number, height: number) {
    const { ctx, colors } = this;
    const radius = Math.min(width, height) * 0.05;
    ctx.save();
    roundedRect(ctx, x, y, width, height, radius);
    ctx.clip();
    if (image) {
      drawCovering(ctx, image, x, y, width, height);
    } else {
      const mode = colors.mode;
      const chroma = game.hue === null ? 0 : 1;
      const tone = (role: "fill" | "ink") =>
        oklchToHex({ lightness: TINT_LEVELS[role][mode], chroma: TINT_LEVELS[role].chroma * chroma, hue: game.hue ?? 0 });
      ctx.fillStyle = tone("fill");
      ctx.fillRect(x, y, width, height);
      ctx.fillStyle = tone("ink");
      const size = Math.max(12, width * 0.17);
      this.font(MONA, size, 500, { stretch: true, tracking: -0.015 });
      const lines = wrapWords(ctx, game.title ?? "", width - size, 3);
      lines.forEach((line, index) =>
        ctx.fillText(line, x + size * 0.5, y + height - size * 0.6 - (lines.length - 1 - index) * size * 1.05),
      );
    }
    ctx.restore();
    ctx.strokeStyle = hexAlpha(colors.text, 0.08);
    ctx.lineWidth = 1;
    roundedRect(ctx, x + 0.5, y + 0.5, width - 1, height - 1, radius);
    ctx.stroke();
  }
}

interface Run {
  text: string;
  italic: boolean;
}

/** Text cut with an ellipsis to fit `width` in the current font. */
function fitText(ctx: CanvasRenderingContext2D, text: string, width: number): string {
  if (ctx.measureText(text).width <= width) return text;
  let cut = text;
  while (cut.length > 0 && ctx.measureText(`${cut.trimEnd()}…`).width > width) cut = cut.slice(0, -1);
  return cut.length > 0 ? `${cut.trimEnd()}…` : "";
}

/** Words in lines of at most `width`, the last line cut with an ellipsis. */
function wrapWords(ctx: CanvasRenderingContext2D, text: string, width: number, maxLines: number): string[] {
  const lines: string[] = [];
  let line = "";
  for (const word of text.split(/\s+/).filter(Boolean)) {
    const next = line ? `${line} ${word}` : word;
    if (ctx.measureText(next).width > width && line) {
      lines.push(line);
      line = word;
    } else {
      line = next;
    }
  }
  if (line) lines.push(line);
  if (lines.length <= maxLines) return lines.map((entry) => fitText(ctx, entry, width));
  const kept = lines.slice(0, maxLines);
  kept[maxLines - 1] = fitText(ctx, `${kept[maxLines - 1]} ${lines[maxLines]}…`, width);
  return kept;
}

/** Spaced capitals, drawn letter by letter where the canvas has no letter spacing. */
function trackedText(ctx: CanvasRenderingContext2D, text: string, x: number, y: number, tracking: number, width: number) {
  if (hasLetterSpacing(ctx)) {
    ctx.letterSpacing = `${tracking}px`;
    ctx.fillText(fitText(ctx, text, width), x, y);
    ctx.letterSpacing = "0px";
    return;
  }
  let left = x;
  for (const char of text) {
    const advance = ctx.measureText(char).width + tracking;
    if (left + advance - x > width) break;
    ctx.fillText(char, left, y);
    left += advance;
  }
}

// Older WebKit canvases have neither setting.
function hasLetterSpacing(ctx: CanvasRenderingContext2D): boolean {
  return typeof (ctx as { letterSpacing?: unknown }).letterSpacing === "string";
}

function hasFontStretch(ctx: CanvasRenderingContext2D): boolean {
  return typeof (ctx as { fontStretch?: unknown }).fontStretch === "string";
}

function roundedRect(ctx: CanvasRenderingContext2D, x: number, y: number, width: number, height: number, radius: number) {
  const r = Math.max(0, Math.min(radius, width / 2, height / 2));
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + width, y, x + width, y + height, r);
  ctx.arcTo(x + width, y + height, x, y + height, r);
  ctx.arcTo(x, y + height, x, y, r);
  ctx.arcTo(x, y, x + width, y, r);
  ctx.closePath();
}

/**
 * An image blurred over the whole box: drawn small and scaled back up in
 * steps, which blurs it the same way in every webview, then softened with
 * the canvas filter where there is one.
 */
function drawBlurred(ctx: CanvasRenderingContext2D, image: HTMLImageElement, width: number, height: number) {
  let source: HTMLCanvasElement | null = null;
  for (const shrink of [SHARE_BACKDROP_SHRINK, SHARE_BACKDROP_SHRINK / 4]) {
    const step = document.createElement("canvas");
    step.width = Math.max(1, Math.round(width / shrink));
    step.height = Math.max(1, Math.round(height / shrink));
    const stepCtx = step.getContext("2d");
    if (!stepCtx) return;
    if (source) {
      stepCtx.imageSmoothingQuality = "high";
      stepCtx.drawImage(source, 0, 0, step.width, step.height);
    } else {
      drawCovering(stepCtx, image, 0, 0, step.width, step.height);
    }
    source = step;
  }
  if (!source) return;
  ctx.save();
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  const radius = Math.round(width / SHARE_BACKDROP_SHRINK);
  if (typeof (ctx as { filter?: unknown }).filter === "string") ctx.filter = `blur(${radius}px)`;
  // Past the edges, so the blur does not fade out at them.
  ctx.drawImage(source, -radius * 2, -radius * 2, width + radius * 4, height + radius * 4);
  ctx.restore();
}

/** An image filling a box and cut to it, like object-fit: cover. */
function drawCovering(
  ctx: CanvasRenderingContext2D,
  image: HTMLImageElement,
  x: number,
  y: number,
  width: number,
  height: number,
) {
  const scale = Math.max(width / image.naturalWidth, height / image.naturalHeight);
  const drawnWidth = image.naturalWidth * scale;
  const drawnHeight = image.naturalHeight * scale;
  ctx.save();
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(image, x + (width - drawnWidth) / 2, y + (height - drawnHeight) / 2, drawnWidth, drawnHeight);
  ctx.restore();
}

// The logo of components/brand/Logo.tsx, in its 54.5 unit view box from 4.75.
const LOGO_VIEW = { origin: 4.75, size: 54.5 };

function drawLogoMark(ctx: CanvasRenderingContext2D, x: number, y: number, size: number, ink: string, pivot: string) {
  const unit = size / LOGO_VIEW.size;
  ctx.save();
  ctx.translate(x, y);
  ctx.scale(unit, unit);
  ctx.translate(-LOGO_VIEW.origin, -LOGO_VIEW.origin);
  ctx.strokeStyle = ink;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  ctx.lineWidth = 4.5;
  roundedRect(ctx, 7, 7, 50, 50, 15);
  ctx.stroke();
  ctx.lineWidth = 6.5;
  ctx.stroke(new Path2D("M25.5 21.3L32 32.5L41 17"));
  ctx.lineWidth = 4.5;
  ctx.stroke(new Path2D("M32 44.5v3.5"));
  ctx.fillStyle = pivot;
  ctx.beginPath();
  ctx.arc(32, 32.5, 3.8, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

function withAlpha(color: Oklch, alpha: number): string {
  return hexAlpha(oklchToHex(color), alpha);
}

/** `#rrggbb` with an alpha from 0 to 1, as `#rrggbbaa`. */
function hexAlpha(hex: string, alpha: number): string {
  const byte = Math.round(Math.min(1, Math.max(0, alpha)) * 255);
  return `${hex}${byte.toString(16).padStart(2, "0")}`;
}
