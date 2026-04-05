import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useLocation, useNavigate } from 'react-router-dom';
import {
  BarChart3,
  Activity,
  Lock,
  Shield,
  Globe,
  ClipboardList,
  Settings,
  Sun,
  Moon,
  Monitor,
  Languages,
  Play,
  Square,
  RotateCw,
} from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { cn } from '../../lib/utils';
import { useSettingsStore } from '../../stores/settings-store';
import { useEngineStore } from '../../stores/engine-store';

interface NavItem {
  icon: React.ElementType;
  labelKey: string;
  path: string;
}

type Theme = 'light' | 'dark' | 'system';

const themeIcons: Record<Theme, React.ElementType> = {
  light: Sun,
  dark: Moon,
  system: Monitor,
};

const themeOrder: Theme[] = ['light', 'dark', 'system'];

export function Sidebar() {
  const { t } = useTranslation('common');
  const location = useLocation();
  const navigate = useNavigate();
  const theme = useSettingsStore((s) => s.theme);
  const setTheme = useSettingsStore((s) => s.setTheme);
  const language = useSettingsStore((s) => s.language);
  const setLanguage = useSettingsStore((s) => s.setLanguage);
  const { status, loading, fetchStatus, start, stop, reload } = useEngineStore();

  // Poll engine status
  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 5000);
    return () => clearInterval(interval);
  }, [fetchStatus]);

  // Listen for system color scheme changes
  useEffect(() => {
    if (theme === 'system') {
      const mq = window.matchMedia('(prefers-color-scheme: dark)');
      const handler = () => useSettingsStore.getState().applyTheme('system');
      mq.addEventListener('change', handler);
      return () => mq.removeEventListener('change', handler);
    }
  }, [theme]);

  const isRunning = status?.status === 'running';

  const sections: { titleKey: string; items: NavItem[] }[] = [
    {
      titleKey: 'nav.proxyManagement',
      items: [
        { icon: BarChart3, labelKey: 'nav.dashboard', path: '/' },
        { icon: Activity, labelKey: 'nav.monitor', path: '/monitor' },
      ],
    },
    {
      titleKey: 'nav.security',
      items: [
        { icon: Lock, labelKey: 'nav.certs', path: '/certs' },
        { icon: Shield, labelKey: 'nav.access', path: '/access' },
        { icon: Globe, labelKey: 'nav.hosts', path: '/hosts' },
      ],
    },
    {
      titleKey: 'nav.system',
      items: [
        { icon: ClipboardList, labelKey: 'nav.logs', path: '/logs' },
        { icon: Settings, labelKey: 'nav.settings', path: '/settings' },
      ],
    },
  ];

  const isActive = (path: string) => {
    if (path === '/') return location.pathname === '/';
    return location.pathname.startsWith(path);
  };

  return (
    <div className="sidebar-nav bg-bg-sidebar border-r border-border py-3 px-2 flex flex-col overflow-y-auto relative">
      {/* macOS drag region — covers the 52px top padding area for traffic lights */}
      <div
        className="hidden [html[data-platform=macos]_&]:block absolute top-0 left-0 right-0 h-[52px]"
        onMouseDown={(e) => {
          e.preventDefault();
          getCurrentWindow().startDragging();
        }}
        onDoubleClick={() => getCurrentWindow().toggleMaximize()}
      />
      <div className="flex flex-col gap-0.5 flex-1">
        {sections.map((section, si) => (
          <div key={si}>
            <div
              className={cn(
                'px-3 text-[10px] font-semibold uppercase tracking-[0.05em] text-text-tertiary mb-1',
                si === 0 ? 'mt-1' : 'mt-4',
              )}
            >
              {t(section.titleKey)}
            </div>
            {section.items.map((item) => {
              const active = isActive(item.path);
              return (
                <button
                  key={item.path}
                  onClick={() => navigate(item.path)}
                  className={cn(
                    'w-full flex items-center gap-2.5 px-3 py-2 rounded-[var(--radius-sm)] cursor-pointer text-[13px] transition-all duration-150 text-left',
                    active
                      ? 'bg-bg-secondary text-text-primary font-medium shadow-[0_1px_2px_rgba(0,0,0,0.04)]'
                      : 'text-text-secondary hover:bg-bg-hover hover:text-text-primary',
                  )}
                >
                  <item.icon
                    className={cn('w-[18px] h-[18px] shrink-0', active ? 'opacity-100' : 'opacity-65')}
                  />
                  <span className="flex-1">{t(item.labelKey)}</span>
                </button>
              );
            })}
          </div>
        ))}
      </div>

      {/* Bottom area */}
      <div className="mt-auto mx-1 flex flex-col pb-1">
        {/* Engine status + controls — above the divider */}
        <div className="flex items-center justify-between px-1 pb-3">
          <div
            className={cn(
              'flex items-center gap-1.5 px-2 py-1 rounded-[20px] text-[11px] font-medium',
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
          <div className="flex items-center gap-0.5">
            {!isRunning ? (
              <button
                onClick={start}
                disabled={loading}
                className="p-1.5 rounded hover:bg-bg-hover text-text-secondary hover:text-success disabled:opacity-50 cursor-pointer"
                title={t('engine.start')}
              >
                <Play className="w-3.5 h-3.5" />
              </button>
            ) : (
              <>
                <button
                  onClick={reload}
                  disabled={loading}
                  className="p-1.5 rounded hover:bg-bg-hover text-text-secondary hover:text-accent disabled:opacity-50 cursor-pointer"
                  title={t('engine.reload')}
                >
                  <RotateCw className="w-3.5 h-3.5" />
                </button>
                <button
                  onClick={stop}
                  disabled={loading}
                  className="p-1.5 rounded hover:bg-bg-hover text-text-secondary hover:text-error disabled:opacity-50 cursor-pointer"
                  title={t('engine.stop')}
                >
                  <Square className="w-3.5 h-3.5" />
                </button>
              </>
            )}
          </div>
        </div>

        {/* Divider */}
        <div className="border-t border-border pt-2 flex flex-col gap-2">
        {/* Theme switcher */}
        <div className="flex items-center gap-1 bg-bg-primary rounded-[var(--radius-sm)] p-0.5">
          {themeOrder.map((t) => {
            const Icon = themeIcons[t];
            return (
              <button
                key={t}
                onClick={() => setTheme(t)}
                className={cn(
                  'flex-1 flex items-center justify-center py-1.5 rounded-[4px] cursor-pointer transition-all duration-150',
                  theme === t
                    ? 'bg-bg-secondary text-text-primary shadow-[0_1px_2px_rgba(0,0,0,0.06)]'
                    : 'text-text-tertiary hover:text-text-secondary',
                )}
                title={t.charAt(0).toUpperCase() + t.slice(1)}
              >
                <Icon className="w-3.5 h-3.5" />
              </button>
            );
          })}
        </div>
        {/* Language switcher */}
        <button
          onClick={() => setLanguage(language === 'zh' ? 'en' : 'zh')}
          className="flex items-center gap-2 px-2.5 py-1.5 rounded-[var(--radius-sm)] text-[12px] text-text-tertiary hover:text-text-secondary hover:bg-bg-hover cursor-pointer transition-colors"
        >
          <Languages className="w-3.5 h-3.5" />
          <span>{language === 'zh' ? 'English' : '中文'}</span>
        </button>
        </div>
      </div>
    </div>
  );
}
