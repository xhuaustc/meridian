import { useState, useRef, useEffect, type ReactNode } from 'react';
import { ChevronDown } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectProps {
  value: string;
  onChange: (e: { target: { value: string } }) => void;
  children?: ReactNode;
  className?: string;
  disabled?: boolean;
}

/**
 * Custom styled select dropdown that matches the app design system.
 * Accepts <option> children (same API as native select) and parses them into a custom dropdown.
 */
function Select({ value, onChange, children, className, disabled }: SelectProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  // Parse options from children
  const options: SelectOption[] = [];
  const parseChildren = (nodes: ReactNode) => {
    if (!nodes) return;
    const arr = Array.isArray(nodes) ? nodes : [nodes];
    for (const child of arr) {
      if (Array.isArray(child)) {
        parseChildren(child);
      } else if (child && typeof child === 'object' && 'props' in child) {
        if (child.type === 'option') {
          options.push({
            value: child.props.value ?? '',
            label:
              typeof child.props.children === 'string'
                ? child.props.children
                : String(child.props.children ?? ''),
          });
        }
      }
    }
  };
  parseChildren(children);

  const selected = options.find((o) => o.value === value);
  const displayText = selected?.label ?? '';

  // Close on click outside
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  // Close on Escape
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [open]);

  return (
    <div ref={ref} className={cn('relative', className)}>
      <button
        type="button"
        disabled={disabled}
        onClick={() => !disabled && setOpen(!open)}
        className={cn(
          'w-full flex items-center justify-between gap-2 px-2.5 py-[7px] border border-border rounded-[var(--radius-sm)] text-[13px] bg-bg-secondary text-text-primary outline-none cursor-pointer transition-colors',
          'hover:border-text-tertiary focus:border-accent',
          disabled && 'opacity-50 cursor-not-allowed',
          open && 'border-accent',
        )}
      >
        <span className={cn('truncate text-left', !selected?.value && 'text-text-tertiary')}>
          {displayText || '\u00A0'}
        </span>
        <ChevronDown
          className={cn(
            'w-3.5 h-3.5 shrink-0 text-text-tertiary transition-transform',
            open && 'rotate-180',
          )}
        />
      </button>

      {open && (
        <div className="absolute z-50 mt-1 w-full max-h-[200px] overflow-y-auto border border-border rounded-[var(--radius-md)] bg-bg-secondary shadow-[0_4px_12px_rgba(0,0,0,0.08)] dark:shadow-[0_4px_12px_rgba(0,0,0,0.3)]">
          {options.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => {
                onChange({ target: { value: opt.value } });
                setOpen(false);
              }}
              className={cn(
                'w-full px-2.5 py-[7px] text-left text-[12.5px] cursor-pointer transition-colors',
                opt.value === value
                  ? 'bg-accent-light text-accent font-medium'
                  : 'text-text-primary hover:bg-bg-hover',
              )}
            >
              {opt.label}
            </button>
          ))}
          {options.length === 0 && (
            <div className="px-2.5 py-[7px] text-[12px] text-text-tertiary">-</div>
          )}
        </div>
      )}
    </div>
  );
}

Select.displayName = 'Select';

export { Select };
