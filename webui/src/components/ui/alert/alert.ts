import { cva, type VariantProps } from 'class-variance-authority'

export const alertVariants = cva(
  'flex gap-3 rounded-3xl border px-4 py-4 text-sm shadow-sm',
  {
    variants: {
      variant: {
        default: 'border-border/70 bg-background/80 text-foreground',
        warning: 'border-primary/20 bg-primary/10 text-foreground',
        destructive: 'border-destructive/20 bg-destructive/10 text-foreground',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  },
)

export type AlertVariants = VariantProps<typeof alertVariants>
