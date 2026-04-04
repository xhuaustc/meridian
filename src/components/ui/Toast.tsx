import { CheckCircle, XCircle, AlertTriangle, Info, X } from 'lucide-react';
import { useToastStore } from '../../stores/toast-store';
import { cn } from '../../lib/utils';

const iconMap = {
  success: CheckCircle,
  error: XCircle,
  warning: AlertTriangle,
  info: Info,
};

const colorMap = {
  success: 'border-success bg-success-bg text-success',
  error: 'border-error bg-error-bg text-error',
  warning: 'border-warning bg-warning-bg text-warning',
  info: 'border-accent bg-accent-light text-accent',
};

export function ToastContainer() {
  const { toasts, removeToast } = useToastStore();

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-[9999] flex flex-col gap-2">
      {toasts.map((toast) => {
        const Icon = iconMap[toast.type];
        return (
          <div
            key={toast.id}
            className={cn(
              'flex items-center gap-2 px-3 py-2.5 rounded-[var(--radius-md)] border shadow-[0_2px_8px_rgba(0,0,0,0.06)] text-[12.5px] min-w-[240px] max-w-[360px] animate-[slideIn_0.2s_ease-out]',
              colorMap[toast.type],
            )}
          >
            <Icon className="w-4 h-4 shrink-0" />
            <span className="flex-1">{toast.message}</span>
            <button
              onClick={() => removeToast(toast.id)}
              className="shrink-0 p-0.5 rounded hover:bg-black/5 dark:hover:bg-white/10"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
