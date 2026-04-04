import { cn } from '../../lib/utils';
import { cva, type VariantProps } from 'class-variance-authority';

const badgeVariants = cva(
  'inline-block px-2 py-0.5 rounded text-[11px] font-medium tracking-[0.02em]',
  {
    variants: {
      variant: {
        http: 'bg-accent-light text-accent',
        https: 'bg-success-bg text-success',
        tcp: 'bg-warning-bg text-[#a16207] dark:text-warning',
        udp: 'bg-[#fdf4ff] text-[#9333ea] dark:bg-[#2d1a3e] dark:text-[#c084fc]',
        allow: 'bg-success-bg text-success',
        deny: 'bg-error-bg text-error',
        self_signed: 'bg-[#fef3c7] text-[#92400e] dark:bg-[#2d2305] dark:text-[#fbbf24]',
        upload: 'bg-[#e0e7ff] text-[#3730a3] dark:bg-[#1e1b4b] dark:text-[#a5b4fc]',
        acme: 'bg-[#d1fae5] text-[#065f46] dark:bg-[#14291e] dark:text-[#6ee7b7]',
      },
    },
    defaultVariants: {
      variant: 'http',
    },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return (
    <span className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}
