// SPDX-FileCopyrightText: 2023 shadcn
// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: MIT

import { Button as ButtonPrimitive } from "@base-ui/react/button"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const buttonVariants = cva(
  "group/button inline-flex shrink-0 items-center justify-center rounded-lg border border-transparent bg-clip-padding text-[13px] font-medium whitespace-nowrap transition-colors outline-none select-none focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:ring-offset-2 focus-visible:ring-offset-ink active:not-aria-[haspopup]:translate-y-px disabled:pointer-events-none disabled:opacity-45 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default: "bg-violet font-semibold text-violet-ink hover:bg-violet-hover",
        outline:
          "bg-glint/7 text-text ring-1 ring-glint/8 ring-inset hover:bg-glint/11 aria-expanded:bg-glint/11",
        ghost:
          "text-soft hover:bg-glint/6 hover:text-text aria-expanded:bg-glint/6 aria-expanded:text-text",
        destructive:
          "border-destructive/40 bg-transparent text-destructive hover:bg-destructive/10 focus-visible:ring-destructive/40",
      },
      size: {
        default:
          "h-[34px] gap-2 px-3.5 has-data-[icon=inline-end]:pr-3 has-data-[icon=inline-start]:pl-3",
        sm: "h-[30px] gap-1.5 px-3 text-[12.5px] has-data-[icon=inline-end]:pr-2.5 has-data-[icon=inline-start]:pl-2.5 [&_svg:not([class*='size-'])]:size-3.5",
        icon: "size-[34px]",
        "icon-sm": "size-[30px]",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant = "default",
  size = "default",
  ...props
}: ButtonPrimitive.Props & VariantProps<typeof buttonVariants>) {
  return (
    <ButtonPrimitive
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button }
