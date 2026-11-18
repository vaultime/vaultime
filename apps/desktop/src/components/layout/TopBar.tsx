// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState, type ReactNode } from "react";
import { useLocation, useNavigate, useNavigationType } from "react-router";
import { ChevronLeft, ChevronRight, Search } from "lucide-react";
import { cn } from "@/lib/utils";

/** The bar over the top of the page: back and forward, and the search that opens the command palette. */
export function TopBar({ scrolled, onSearch }: { scrolled: boolean; onSearch: () => void }) {
  const navigate = useNavigate();
  const { back, forward } = useHistorySteps();
  return (
    <div
      className={cn(
        "sticky top-0 z-20 flex h-(--top-bar) items-center gap-1.5 px-3.5 transition-colors",
        scrolled && "bg-ink/95",
      )}
    >
      <StepButton label="Back" disabled={!back} onClick={() => void navigate(-1)}>
        <ChevronLeft className="size-[18px]" strokeWidth={1.8} />
      </StepButton>
      <StepButton label="Forward" disabled={!forward} onClick={() => void navigate(1)}>
        <ChevronRight className="size-[18px]" strokeWidth={1.8} />
      </StepButton>
      <button
        type="button"
        onClick={onSearch}
        className="mx-auto flex h-[34px] min-w-0 flex-1 basis-0 items-center gap-2 rounded-[9px] bg-glint/6 px-2.5 text-left text-[13px] text-faint ring-1 ring-glint/7 transition-colors ring-inset hover:bg-glint/9 hover:text-soft focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none sm:max-w-[400px]"
      >
        <Search className="size-[15px] shrink-0" strokeWidth={1.8} />
        <span className="min-w-0 flex-1 truncate">Search games, pages and settings</span>
        <kbd className="shrink-0 rounded-[5px] px-1.5 py-0.5 font-mono text-[10.5px] ring-1 ring-glint/12 ring-inset">
          Ctrl K
        </kbd>
      </button>
      {/* As wide as the two buttons, so the search sits in the middle. */}
      <span aria-hidden="true" className="w-[66px] shrink-0" />
    </div>
  );
}

function StepButton({
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
      title={label}
      disabled={disabled}
      onClick={onClick}
      className="flex size-[30px] shrink-0 items-center justify-center rounded-full bg-scrim/40 text-soft transition-colors hover:text-text focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none disabled:text-faint/50"
    >
      {children}
    </button>
  );
}

/** Whether going back or forward leads to a page, from the index the router keeps in the history state. */
function useHistorySteps(): { back: boolean; forward: boolean } {
  const location = useLocation();
  const type = useNavigationType();
  const index = (window.history.state as { idx?: number } | null)?.idx ?? 0;
  const [seen, setSeen] = useState({ key: location.key, furthest: index });
  if (seen.key !== location.key) {
    // Opening a page drops the pages that were ahead.
    setSeen({ key: location.key, furthest: type === "PUSH" ? index : Math.max(seen.furthest, index) });
  }
  return { back: index > 0, forward: index < seen.furthest };
}
