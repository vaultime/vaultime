// SPDX-FileCopyrightText: 2023 shadcn
// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: MIT

import * as React from "react"
import { Input as InputPrimitive } from "@base-ui/react/input"

import { useFieldControl } from "@/components/ui/field-context"
import { cn } from "@/lib/utils"

function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  const field = useFieldControl()
  return (
    <InputPrimitive
      type={type}
      data-slot="input"
      aria-describedby={field?.messageId}
      aria-invalid={field?.invalid || undefined}
      className={cn(
        "h-10 w-full min-w-0 rounded-[10px] border border-hairline bg-transparent px-3 py-1 text-sm text-text transition-colors outline-none file:inline-flex file:h-6 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-text placeholder:text-faint focus-visible:border-violet focus-visible:ring-2 focus-visible:ring-violet/30 disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-amber aria-invalid:focus-visible:ring-amber/30",
        className
      )}
      {...props}
    />
  )
}

export { Input }
