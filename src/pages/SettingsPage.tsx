import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, Upload, Database } from 'lucide-react';
import { Button } from '../components/ui/Button';
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
    <div className="max-w-[560px]">
      <h1 className="text-[18px] font-semibold tracking-[-0.02em] mb-6">
        {t('settings.title')}
      </h1>

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
  );
}
