import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, Upload, Database } from 'lucide-react';
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
import { ContentToolbar } from '../components/layout/ContentToolbar';
import { Button } from '../components/ui/Button';
import { Toggle } from '../components/ui/Toggle';
import { ConfirmDialog } from '../components/ui/Dialog';
import { useSettingsStore } from '../stores/settings-store';
import { useToastStore } from '../stores/toast-store';
import * as api from '../lib/api';
import { cn } from '../lib/utils';
import type { ExportData } from '../types';

export function SettingsPage() {
  const { t, i18n } = useTranslation('common');
  const { theme, setTheme, language, setLanguage } = useSettingsStore();
  const addToast = useToastStore((s) => s.addToast);
  const [showImportConfirm, setShowImportConfirm] = useState(false);
  const [importPayload, setImportPayload] = useState<ExportData | null>(null);
  const [autoStartEngine, setAutoStartEngine] = useState(false);
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [logRetentionDays, setLogRetentionDays] = useState('7');

  useEffect(() => {
    // Load auto-start engine setting
    api.getSetting('auto_start_engine').then((v) => setAutoStartEngine(v === 'true'));
    // Load launch-at-login state
    isEnabled().then(setLaunchAtLogin).catch(() => {});
    // Load log retention days
    api.getSetting('log_retention_days').then((v) => {
      if (v) setLogRetentionDays(v);
    });
  }, []);

  const handleExport = async () => {
    try {
      const data = await api.exportData();
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `meridian-export-${new Date().toISOString().split('T')[0]}.json`;
      a.click();
      URL.revokeObjectURL(url);
      addToast('success', t('settings.exportSuccess'));
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleImportFile = () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;
      try {
        const text = await file.text();
        const data = JSON.parse(text) as ExportData;
        setImportPayload(data);
        setShowImportConfirm(true);
      } catch {
        addToast('error', t('common.error'));
      }
    };
    input.click();
  };

  const handleImport = async () => {
    if (!importPayload) return;
    try {
      await api.importData(importPayload);
      addToast('success', t('settings.importSuccess'));
      setImportPayload(null);
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleBackup = async () => {
    try {
      const path = await api.backupDatabase();
      addToast('success', t('settings.backupSuccess', { path }));
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleLanguageChange = (lang: string) => {
    i18n.changeLanguage(lang);
    setLanguage(lang);
  };

  const themes: { value: 'light' | 'dark' | 'system'; labelKey: string }[] = [
    { value: 'light', labelKey: 'settings.themeLight' },
    { value: 'dark', labelKey: 'settings.themeDark' },
    { value: 'system', labelKey: 'settings.themeSystem' },
  ];

  return (
    <>
      <ContentToolbar title={t('settings.title')} />
      <div className="p-6 overflow-y-auto flex-1">
      <div className="max-w-[560px]">

      {/* Language */}
      <section className="mb-8">
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('settings.language')}
        </h2>
        <div className="flex border border-border rounded-[var(--radius-sm)] overflow-hidden w-fit">
          {['zh', 'en'].map((lang) => (
            <button
              key={lang}
              onClick={() => handleLanguageChange(lang)}
              className={cn(
                'px-4 py-[7px] text-[12px] cursor-pointer border-r border-border last:border-r-0',
                language === lang
                  ? 'bg-accent-light text-accent font-medium'
                  : 'bg-bg-secondary text-text-secondary hover:bg-bg-hover',
              )}
            >
              {lang === 'zh' ? '中文' : 'English'}
            </button>
          ))}
        </div>
      </section>

      {/* Theme */}
      <section className="mb-8">
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('settings.theme')}
        </h2>
        <div className="flex border border-border rounded-[var(--radius-sm)] overflow-hidden w-fit">
          {themes.map((th) => (
            <button
              key={th.value}
              onClick={() => setTheme(th.value)}
              className={cn(
                'px-4 py-[7px] text-[12px] cursor-pointer border-r border-border last:border-r-0',
                theme === th.value
                  ? 'bg-accent-light text-accent font-medium'
                  : 'bg-bg-secondary text-text-secondary hover:bg-bg-hover',
              )}
            >
              {t(th.labelKey)}
            </button>
          ))}
        </div>
      </section>

      {/* Startup */}
      <section className="mb-8">
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('settings.startup')}
        </h2>
        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
            <div>
              <div className="text-[13px] font-medium">{t('settings.autoStartEngine')}</div>
              <div className="text-[11px] text-text-tertiary mt-0.5">{t('settings.autoStartEngineDesc')}</div>
            </div>
            <Toggle
              checked={autoStartEngine}
              onChange={async (v) => {
                setAutoStartEngine(v);
                await api.setSetting('auto_start_engine', v ? 'true' : 'false');
              }}
            />
          </div>
          <div className="flex items-center justify-between bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
            <div>
              <div className="text-[13px] font-medium">{t('settings.launchAtLogin')}</div>
              <div className="text-[11px] text-text-tertiary mt-0.5">{t('settings.launchAtLoginDesc')}</div>
            </div>
            <Toggle
              checked={launchAtLogin}
              onChange={async (v) => {
                try {
                  if (v) { await enable(); } else { await disable(); }
                  setLaunchAtLogin(v);
                } catch (e) { addToast('error', String(e)); }
              }}
            />
          </div>
        </div>
      </section>

      {/* Log Retention */}
      <section className="mb-8">
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('settings.logRetention')}
        </h2>
        <div className="flex items-center justify-between bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
          <div>
            <div className="text-[13px] font-medium">{t('settings.logRetentionDays')}</div>
            <div className="text-[11px] text-text-tertiary mt-0.5">{t('settings.logRetentionDaysDesc')}</div>
          </div>
          <div className="flex items-center gap-2">
            <input
              type="number"
              min="1"
              max="365"
              className="w-16 px-2 py-[5px] border border-border rounded-[var(--radius-sm)] text-[12.5px] bg-bg-primary text-text-primary text-center outline-none focus:border-accent"
              value={logRetentionDays}
              onChange={(e) => setLogRetentionDays(e.target.value)}
              onBlur={async () => {
                const days = Math.max(1, Math.min(365, parseInt(logRetentionDays) || 7));
                setLogRetentionDays(String(days));
                await api.setSetting('log_retention_days', String(days));
              }}
            />
            <span className="text-[12px] text-text-secondary">{t('settings.logRetentionUnit')}</span>
          </div>
        </div>
      </section>

      {/* Data Management */}
      <section className="mb-8">
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('settings.dataManagement')}
        </h2>
        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
            <div>
              <div className="text-[13px] font-medium">{t('settings.export')}</div>
              <div className="text-[11px] text-text-tertiary mt-0.5">
                JSON
              </div>
            </div>
            <Button size="sm" onClick={handleExport}>
              <Download className="w-3.5 h-3.5" />
              {t('settings.export')}
            </Button>
          </div>
          <div className="flex items-center justify-between bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
            <div>
              <div className="text-[13px] font-medium">{t('settings.import')}</div>
              <div className="text-[11px] text-text-tertiary mt-0.5">
                JSON
              </div>
            </div>
            <Button size="sm" onClick={handleImportFile}>
              <Upload className="w-3.5 h-3.5" />
              {t('settings.import')}
            </Button>
          </div>
          <div className="flex items-center justify-between bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
            <div>
              <div className="text-[13px] font-medium">{t('settings.backup')}</div>
              <div className="text-[11px] text-text-tertiary mt-0.5">
                SQLite
              </div>
            </div>
            <Button size="sm" onClick={handleBackup}>
              <Database className="w-3.5 h-3.5" />
              {t('settings.backup')}
            </Button>
          </div>
        </div>
      </section>

      {/* About */}
      <section>
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('settings.about')}
        </h2>
        <div className="text-[12px] text-text-secondary">
          {t('settings.version')}: 0.1.0
        </div>
      </section>

      <ConfirmDialog
        open={showImportConfirm}
        onClose={() => {
          setShowImportConfirm(false);
          setImportPayload(null);
        }}
        onConfirm={handleImport}
        title={t('settings.import')}
        message={t('settings.importConfirm')}
        danger
      />
      </div>
      </div>
    </>
  );
}
