// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useLayoutEffect, useRef, useState, type ComponentProps } from "react";
import { Eye, EyeOff } from "lucide-react";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

/** A password field with its own show and hide button, which works on every platform and stays in place. */
export function PasswordInput({ className, ...props }: Omit<ComponentProps<"input">, "type">) {
  const [visible, setVisible] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const caret = useRef<[number, number] | null>(null);

  // Switching the type can move the caret, so it goes back where the player left it.
  useLayoutEffect(() => {
    const input = inputRef.current;
    if (input && caret.current && document.activeElement === input) {
      input.setSelectionRange(...caret.current);
    }
    caret.current = null;
  }, [visible]);

  function toggle() {
    const input = inputRef.current;
    if (input && document.activeElement === input) {
      caret.current = [input.selectionStart ?? input.value.length, input.selectionEnd ?? input.value.length];
    }
    setVisible((current) => !current);
  }

  const label = visible ? "Hide password" : "Show password";

  return (
    <div className="relative">
      <Input {...props} ref={inputRef} type={visible ? "text" : "password"} className={cn("pr-11", className)} />
      <button
        type="button"
        aria-label={label}
        aria-controls={props.id}
        title={label}
        // Keeps the focus in the field while typing.
        onMouseDown={(event) => event.preventDefault()}
        onClick={toggle}
        className="absolute inset-y-0 right-0 flex w-10 items-center justify-center rounded-r-[10px] text-faint transition-colors outline-none hover:text-text focus-visible:text-text focus-visible:ring-2 focus-visible:ring-violet/60"
      >
        {visible ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
      </button>
    </div>
  );
}
