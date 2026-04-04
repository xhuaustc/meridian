import { forwardRef, type ButtonHTMLAttributes } from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '../../lib/utils';

const buttonVariants = cva(
  'inline-flex items-center justify-center gap-1.5 rounded-[var(--radius-sm)] text-[12px] font-medium cursor-pointer transition-all duration-150 disabled:opacity-50 disabled:cursor-not-allowed',
  {
    variants: {
      variant: {
        default:
          'border border-border bg-bg-secondary text-text-primary hover:bg-bg-sidebar',
        primary:
          'border border-accent bg-accent text-white hover:bg-[#1d4ed8]',
        ghost:
          'border border-transparent bg-transparent text-text-secondary hover:bg-bg-hover hover:text-text-primary',
        danger:
          'border border-error bg-error text-white hover:bg-[#b91c1c]',
      },
      size: {
        default: 'px-3.5 py-[7px]',
        sm: 'px-2.5 py-1 text-[11px]',
        icon: 'w-7 h-7 p-0',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
);

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => {
    return (
      <button
        className={cn(buttonVariants({ variant, size }), className)}
        ref={ref}
        {...props}
      />
    );
  },
);

Button.displayName = 'Button';

export { Button, buttonVariants };
