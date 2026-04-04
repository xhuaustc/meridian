import { forwardRef, type SelectHTMLAttributes } from 'react';
import { cn } from '../../lib/utils';

export interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {}

const Select = forwardRef<HTMLSelectElement, SelectProps>(
  ({ className, children, ...props }, ref) => {
    return (
      <select
        className={cn(
          'w-full px-2.5 py-[7px] border border-border rounded-[var(--radius-sm)] text-[13px] bg-bg-secondary text-text-primary outline-none focus:border-accent',
          className,
        )}
        ref={ref}
        {...props}
      >
        {children}
      </select>
    );
  },
);

Select.displayName = 'Select';

export { Select };
