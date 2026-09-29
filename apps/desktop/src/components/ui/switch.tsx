// SPDX-FileCopyrightText: 2023 shadcn
// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: MIT

import { Switch as SwitchPrimitive } from "@base-ui/react/switch"

import { cn } from "@/lib/utils"

function Switch({ className, ...props }: SwitchPrimitive.Root.Props) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border border-hairline bg-raised p-0.5 transition-colors outline-none focus-visible:ring-2 focus-visible:ring-violet/60 data-checked:border-violet data-checked:bg-violet data-disabled:cursor-not-allowed data-disabled:opacity-50",
        className
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className="size-[18px] rounded-full bg-soft transition-transform data-checked:translate-x-5 data-checked:bg-violet-ink"
      />
    </SwitchPrimitive.Root>
  )
}

export { Switch }
