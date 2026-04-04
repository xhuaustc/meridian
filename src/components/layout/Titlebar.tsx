import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Play, Square, RotateCw } from 'lucide-react';
import { useEngineStore } from '../../stores/engine-store';
import { useSettingsStore } from '../../stores/settings-store';
import { cn } from '../../lib/utils';

export function Titlebar() {
  const { t, i18n } = useTranslation('common');
  const { status, loading, fetchStatus, start, stop, reload } = useEngineStore();
  const { theme, setTheme, language, setLanguage } = useSettingsStore();

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 5000);
    return () => clearInterval(interval);
  }, [fetchStatus]);

  const toggleLanguage = () => {
    const newLang = language === 'zh' ? 'en' : 'zh';
    i18n.changeLanguage(newLang);
    setLanguage(newLang);
  };

  const cycleTheme = () => {
    const order: Array<'light' | 'dark' | 'system'> = ['light', 'dark', 'system'];
    const idx = order.indexOf(theme);
    setTheme(order[(idx + 1) % order.length]);
  };

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
      className="col-span-full bg-bg-secondary border-b border-border flex items-center justify-between px-4 h-12"
      data-tauri-drag-region
    >
      <div className="flex items-center gap-2.5" data-tauri-drag-region>
        <img src="/app-icon.png" alt="Meridian" className="w-6 h-6 rounded-[6px]" />
        <span className="text-[13px] font-semibold tracking-[-0.01em]" data-tauri-drag-region>
          {t('app.name')}
        </span>
      </div>

      <div className="flex items-center gap-3">
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

        <button
          onClick={cycleTheme}
          className="text-[11px] text-text-secondary bg-bg-sidebar border border-border px-2 py-0.5 rounded cursor-pointer hover:bg-bg-hover"
        >
          {theme === 'light'
            ? t('settings.themeLight')
            : theme === 'dark'
              ? t('settings.themeDark')
              : t('settings.themeSystem')}
        </button>

        <button
          onClick={toggleLanguage}
          className="text-[11px] text-text-secondary bg-bg-sidebar border border-border px-2 py-0.5 rounded cursor-pointer hover:bg-bg-hover"
        >
          {language === 'zh' ? '中 / EN' : 'EN / 中'}
        </button>
      </div>
    </div>
  );
}
