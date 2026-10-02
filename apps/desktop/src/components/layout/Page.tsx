// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Building blocks for pages that read like a document: a header, sections
// with their title on the left, and rows separated by hairlines.

import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

export function PageHeader({
  overline,
  title,
  aside,
  children,
}: {
  overline: ReactNode;
  title: string;
  /** Controls on the right, like week arrows. */
  aside?: ReactNode;
  /** The sentence under the title. */
  children?: ReactNode;
}) {
  return (
    <header className="flex items-end justify-between gap-10 border-b border-rule px-8 pt-12 pb-8 xl:px-14">
      <div className="min-w-0">
        <div className="label-caps">{overline}</div>
        <h1 className="font-display mt-3 text-[clamp(56px,6vw,84px)] leading-[0.95]">
          {title}
        </h1>
        {children && (
          <p className="font-prose mt-4 max-w-[640px] text-[23px] leading-[1.3] text-pretty text-soft">
            {children}
          </p>
        )}
      </div>
      {aside && <div className="flex shrink-0 gap-2.5">{aside}</div>}
    </header>
  );
}

export function PageSection({
  title,
  description,
  children,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="grid gap-x-12 gap-y-4 border-b border-rule py-9 lg:grid-cols-[260px_minmax(0,1fr)]">
      <div>
        <h2 className="font-display text-[28px] leading-tight">{title}</h2>
        {description && <p className="mt-2 text-sm leading-relaxed text-pretty text-faint">{description}</p>}
      </div>
      <div className="min-w-0">{children}</div>
    </section>
  );
}

/** A label with an optional hint on the left and a value or control on the right. */
export function PageRow({
  label,
  hint,
  htmlFor,
  children,
}: {
  label: ReactNode;
  hint?: ReactNode;
  /** Id of the control, so the label focuses it. */
  htmlFor?: string;
  children?: ReactNode;
}) {
  const Label = htmlFor ? "label" : "div";
  return (
    <div className="flex items-center justify-between gap-8 border-b border-rule py-4 first:pt-0 last:border-b-0">
      <div className="min-w-0">
        <Label htmlFor={htmlFor} className="text-[15px] text-text">
          {label}
        </Label>
        {hint && <p className="mt-1 max-w-[520px] text-[13px] leading-relaxed text-faint">{hint}</p>}
      </div>
      {children !== undefined && <div className="shrink-0 text-right">{children}</div>}
    </div>
  );
}

/** A round arrow button that steps through weeks or years in a page header. */
export function StepButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      className="flex size-11 items-center justify-center rounded-full border border-hairline text-soft transition-colors hover:bg-raised hover:text-text focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none disabled:border-rule disabled:text-faint/50 disabled:hover:bg-transparent"
    >
      {children}
    </button>
  );
}

/** A one-line message under a header or inside a section. */
export function Notice({
  tone = "info",
  children,
  className,
}: {
  tone?: "info" | "warning";
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      role={tone === "warning" ? "alert" : "status"}
      className={cn(
        "flex flex-wrap items-center justify-between gap-3 border-l-2 py-1.5 pl-4 text-sm",
        tone === "warning" ? "border-amber text-amber" : "border-violet text-soft",
        className,
      )}
    >
      {children}
    </div>
  );
}
