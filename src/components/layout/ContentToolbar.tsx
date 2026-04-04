import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Play, Square, RotateCw } from 'lucide-react';
import { useEngineStore } from '../../stores/engine-store';
import { useSettingsStore } from '../../stores/settings-store';
import { cn } from '../../lib/utils';

interface ContentToolbarProps {
  title: string;
  children?: React.ReactNode;
}

export function ContentToolbar({ title, children }: ContentToolbarProps) {
  const { t } = useTranslation('common');
  const { status, loading, fetchStatus, start, stop, reload } = useEngineStore();
  const theme = useSettingsStore((s) => s.theme);

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 5000);
    return () => clearInterval(interval);
  }, [fetchStatus]);

  // Listen for system color scheme changes (migrated from old Titlebar)
  useEffect(() => {
    if (theme === 'system') {
      const mq = window.matchMedia('(prefers-color-scheme: dark)');
      const handler = () => useSettingsStore.getState().applyTheme('system');
      mq.addEventListener('change', handler);
      return () => mq.removeEventListener('change', handler);
    }
  }, [theme]);

  const isRunning = status?.status === 'running';

  return (
    <div
      className="h-12 flex items-center justify-between px-5 border-b border-border shrink-0"
      data-tauri-drag-region
    >
      <h1
        className="text-[15px] font-semibold tracking-[-0.01em] text-text-primary"
        data-tauri-drag-region
      >
        {title}
      </h1>
      <div className="flex items-center gap-3">
        {/* Engine status pill */}
        <div
          className={cn(
            'flex items-center gap-1.5 px-2.5 py-1 rounded-[20px] text-[11px] font-medium',
            isRunning
              ? 'bg-success-bg text-success'
              : 'bg-error-bg text-error',
          )}
        >
          <span
            className={cn(
              'w-1.5 h-1.5 rounded-full bg-current',
              isRunning && 'animate-pulse',
            )}
          />
          {isRunning ? t('engine.running') : t('engine.stopped')}
        </div>

        {/* Engine controls */}
        <div className="flex items-center gap-1">
          {!isRunning ? (
            <button
              onClick={start}
              disabled={loading}
              className="p-1.5 rounded hover:bg-bg-hover text-text-secondary hover:text-success disabled:opacity-50"
              title={t('engine.start')}
            >
              <Play className="w-3.5 h-3.5" />
            </button>
          ) : (
            <>
              <button
                onClick={reload}
                disabled={loading}
                className="p-1.5 rounded hover:bg-bg-hover text-text-secondary hover:text-accent disabled:opacity-50"
                title={t('engine.reload')}
              >
                <RotateCw className="w-3.5 h-3.5" />
              </button>
              <button
                onClick={stop}
                disabled={loading}
                className="p-1.5 rounded hover:bg-bg-hover text-text-secondary hover:text-error disabled:opacity-50"
                title={t('engine.stop')}
              >
                <Square className="w-3.5 h-3.5" />
              </button>
            </>
          )}
        </div>

        {/* Page-specific actions */}
        {children}
      </div>
    </div>
  );
}
