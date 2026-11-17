import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, Upload } from 'lucide-react';
import { getVersion } from '@tauri-apps/api/app';
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
import { save, open } from '@tauri-apps/plugin-dialog';
import { ContentToolbar } from '../components/layout/ContentToolbar';
import { Button } from '../components/ui/Button';
import { Badge } from '../components/ui/Badge';
import { Toggle } from '../components/ui/Toggle';
import { ConfirmDialog } from '../components/ui/Dialog';
import { useSettingsStore } from '../stores/settings-store';
import { useToastStore } from '../stores/toast-store';
import { useApiError } from '../hooks/useApiError';
import { useProxyStore } from '../stores/proxy-store';
import { useCertStore } from '../stores/cert-store';
import { useAccessStore } from '../stores/access-store';
import { useHostsStore } from '../stores/hosts-store';
import * as api from '../lib/api';
import { cn } from '../lib/utils';
import { REQUEST_PARAM_HINT_KEYS, REQUEST_PARAM_LABEL_KEYS, isValidRequestParamValue } from '../lib/requestParams';
import type { RecoveryPreview, EngineCapabilities, HostnameSettingsEntry, HostnameSettingsOverview, RequestParamCatalog, RequestParamKey, RequestParamValues } from '../types';

export function SettingsPage() {
  const { t, i18n } = useTranslation('common');
  const { theme, setTheme, language, setLanguage } = useSettingsStore();
  const addToast = useToastStore((s) => s.addToast);
  const formatError = useApiError();
  const fetchProxies = useProxyStore((s) => s.fetchProxies);
  const proxies = useProxyStore((s) => s.proxies);
  const fetchCertificates = useCertStore((s) => s.fetchCertificates);
  const fetchLists = useAccessStore((s) => s.fetchLists);
  const fetchEntries = useHostsStore((s) => s.fetchEntries);
  const [showImportConfirm, setShowImportConfirm] = useState(false);
  const [recoveryPath, setRecoveryPath] = useState('');
  const [recoveryPreview, setRecoveryPreview] = useState<RecoveryPreview | null>(null);
  const [passphrase, setPassphrase] = useState('');
  const [recoveryBusy, setRecoveryBusy] = useState(false);
  const [autoStartEngine, setAutoStartEngine] = useState(false);
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [logRetentionDays, setLogRetentionDays] = useState('7');
  const [workerProcesses, setWorkerProcesses] = useState('2');
  const [appVersion, setAppVersion] = useState('');
  const [capabilities, setCapabilities] = useState<EngineCapabilities | null>(null);
  const [capabilitiesLoading, setCapabilitiesLoading] = useState(true);
  const [requestParamCatalog, setRequestParamCatalog] = useState<RequestParamCatalog | null>(null);
  const [requestDefaultDrafts, setRequestDefaultDrafts] = useState<RequestParamValues | null>(null);
  const [requestDefaultErrors, setRequestDefaultErrors] = useState<Partial<Record<RequestParamKey, boolean>>>({});
  const [hostnameOverview, setHostnameOverview] = useState<HostnameSettingsOverview | null>(null);
  const [hostnameOverviewError, setHostnameOverviewError] = useState(false);
  const [hstsBusyHostname, setHstsBusyHostname] = useState<string | null>(null);
  const [cleanupBusy, setCleanupBusy] = useState(false);
  const [showOrphanCleanup, setShowOrphanCleanup] = useState(false);

  useEffect(() => {
    // Load app version
    getVersion().then(setAppVersion);
    // Load auto-start engine setting
    api.getSetting('auto_start_engine').then((v) => setAutoStartEngine(v === 'true'));
    // Load launch-at-login state
    isEnabled().then(setLaunchAtLogin).catch(() => {});
    // Load log retention days
    api.getSetting('log_retention_days').then((v) => {
      if (v) setLogRetentionDays(v);
    });
    api.getSetting('worker_processes').then((v) => {
      if (v) setWorkerProcesses(v);
    });
    // The global request-parameter defaults. The backend falls back to its
    // built-in values for missing rows, so the inputs never start blank.
    api
      .getRequestParamDefaults()
      .then((catalog) => {
        setRequestParamCatalog(catalog);
        setRequestDefaultDrafts(catalog.defaults);
      })
      .catch(() => {});
    api
      .getEngineCapabilities()
      .then((caps) => setCapabilities(caps))
      .catch(() => setCapabilities(null))
      .finally(() => setCapabilitiesLoading(false));
  }, []);

  // Whether a stored hostname setting is an orphan depends on the rule list.
  // Every rule create/update/delete goes through the proxy store, so refetch
  // when that list changes; the initial (possibly empty) value loads the
  // section on mount, including when the database has no rules at all.
  useEffect(() => {
    let cancelled = false;
    api
      .listHostnameSettings()
      .then((overview) => {
        if (cancelled) return;
        setHostnameOverview(overview);
        setHostnameOverviewError(false);
      })
      .catch(() => {
        if (!cancelled) setHostnameOverviewError(true);
      });
    return () => {
      cancelled = true;
    };
  }, [proxies]);

  const refreshHostnameSettings = async () => {
    try {
      const overview = await api.listHostnameSettings();
      setHostnameOverview(overview);
      setHostnameOverviewError(false);
    } catch (e) {
      setHostnameOverviewError(true);
      addToast('error', formatError(e));
    }
  };

  // The settings page is the only way to turn HSTS off for a hostname whose
  // rules no longer expose the toggle (the last TLS rule was deleted or changed
  // to non-TLS). The rule form owns the switch; this page owns the way back.
  const handleDisableHsts = async (entry: HostnameSettingsEntry) => {
    setHstsBusyHostname(entry.hostname);
    try {
      await api.setHostnameHsts(
        entry.hostname,
        false,
        entry.hsts_max_age,
        entry.hsts_include_subdomains,
      );
      addToast('success', t('settings.domainDisableHstsSuccess', { hostname: entry.hostname }));
      await refreshHostnameSettings();
    } catch (e) {
      addToast('error', formatError(e));
    } finally {
      setHstsBusyHostname(null);
    }
  };

  const handleCleanupOrphans = async () => {
    setCleanupBusy(true);
    try {
      const removed = await api.cleanupOrphanHostnameSettings();
      addToast('success', t('settings.domainCleanupSuccess', { count: removed }));
      await refreshHostnameSettings();
    } catch (e) {
      addToast('error', formatError(e));
    } finally {
      setCleanupBusy(false);
    }
  };

  const handleExport = async () => {
    if (Array.from(passphrase).length < 12) {
      addToast('error', t('settings.passphraseHint'));
      return;
    }
    try {
      const filePath = await save({
        defaultPath: `meridian-recovery-${new Date().toISOString().split('T')[0]}.meridian`,
        filters: [{ name: 'Meridian recovery', extensions: ['meridian'] }],
      });
      if (!filePath) return;
      setRecoveryBusy(true);
      await api.createRecoveryBundle(filePath, passphrase);
      setPassphrase('');
      addToast('success', t('settings.exportSuccess'));
    } catch (e) {
      addToast('error', formatError(e));
    } finally {
      setRecoveryBusy(false);
    }
  };

  const handleImportFile = async () => {
    try {
      if (Array.from(passphrase).length < 12) {
        addToast('error', t('settings.passphraseHint'));
        return;
      }
      const filePath = await open({
        filters: [{ name: 'Meridian recovery', extensions: ['meridian'] }],
        multiple: false,
      });
      if (!filePath || typeof filePath !== 'string') return;
      setRecoveryBusy(true);
      const preview = await api.previewRecoveryBundle(filePath, passphrase);
      setRecoveryPath(filePath);
      setRecoveryPreview(preview);
      setShowImportConfirm(true);
    } catch (e) {
      addToast('error', formatError(e));
    } finally {
      setRecoveryBusy(false);
    }
  };

  const handleImport = async () => {
    if (!recoveryPath || !recoveryPreview) return;
    try {
      setRecoveryBusy(true);
      const backupPath = await api.restoreRecoveryBundle(recoveryPath, passphrase);
      await Promise.all([
        fetchProxies(),
        fetchCertificates(),
        fetchLists(),
        fetchEntries(),
      ]);
      addToast('success', t('settings.importSuccess', { path: backupPath }));
      setRecoveryPath('');
      setRecoveryPreview(null);
      setPassphrase('');
    } catch (e) {
      addToast('error', formatError(e));
    } finally {
      setRecoveryBusy(false);
    }
  };

  const handleLanguageChange = (lang: string) => {
    i18n.changeLanguage(lang);
    setLanguage(lang);
  };

  const clearRequestDefaultError = (directive: RequestParamKey) => {
    setRequestDefaultErrors((current) => {
      const next = { ...current };
      delete next[directive];
      return next;
    });
  };

  // Each row saves on blur, like the other settings here. A value the backend
  // parser would reject stays unsaved and is reported against its own field.
  const saveRequestDefault = async (directive: RequestParamKey) => {
    if (!requestParamCatalog || !requestDefaultDrafts) return;
    const value = requestDefaultDrafts[directive].trim();
    if (value === requestParamCatalog.defaults[directive]) {
      clearRequestDefaultError(directive);
      return;
    }
    if (!isValidRequestParamValue(directive, value)) {
      setRequestDefaultErrors((current) => ({ ...current, [directive]: true }));
      return;
    }
    try {
      const catalog = await api.setRequestParamDefault(directive, value);
      setRequestParamCatalog(catalog);
      setRequestDefaultDrafts((current) =>
        current ? { ...current, [directive]: catalog.defaults[directive] } : catalog.defaults,
      );
      clearRequestDefaultError(directive);
    } catch (e) {
      addToast('error', formatError(e));
    }
  };

  const requestDefaultErrorMessage = (directive: RequestParamKey) => {
    const field = t(REQUEST_PARAM_LABEL_KEYS[directive]);
    return directive === 'proxy_buffering'
      ? t('proxyForm.requestParamInvalidToggle', { field })
      : t('proxyForm.requestParamInvalidUnit', {
          field,
          example: t(REQUEST_PARAM_HINT_KEYS[directive]),
        });
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

      {/* Domain Settings */}
      <section className="mb-8">
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('settings.domainSettings')}
        </h2>
        <p className="text-[11px] text-text-tertiary mb-3">
          {t('settings.domainSettingsDesc')}
        </p>
        {hostnameOverview === null ? (
          <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3 text-[12px] text-text-tertiary">
            {hostnameOverviewError ? t('settings.domainSettingsError') : t('common.loading')}
          </div>
        ) : hostnameOverview.entries.length === 0 ? (
          <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-6 text-center text-[12px] text-text-tertiary">
            {t('settings.domainSettingsEmpty')}
          </div>
        ) : (
          <>
            <div className="flex flex-col gap-3">
              {hostnameOverview.entries.map((entry) => (
                <div
                  key={entry.hostname}
                  className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-[13px] font-medium truncate">{entry.hostname}</span>
                        {entry.is_orphan && (
                          <Badge variant="tcp" className="shrink-0">
                            {t('settings.domainOrphan')}
                          </Badge>
                        )}
                      </div>
                      <div className="text-[11px] text-text-tertiary mt-0.5">
                        {entry.is_orphan
                          ? t('settings.domainOrphanDesc')
                          : t('settings.domainRuleCount', { count: entry.rule_count })}
                      </div>
                    </div>
                    {entry.hsts_enabled && (
                      <Button
                        size="sm"
                        className="shrink-0 whitespace-nowrap"
                        disabled={hstsBusyHostname === entry.hostname}
                        onClick={() => handleDisableHsts(entry)}
                      >
                        {t('settings.domainDisableHsts')}
                      </Button>
                    )}
                  </div>
                  <div className="flex flex-wrap items-center gap-1.5 mt-2">
                    <CoverageChip
                      on={entry.hsts_enabled}
                      label={
                        entry.hsts_enabled
                          ? entry.hsts_include_subdomains
                            ? t('settings.domainHstsOnWithSubdomains', { seconds: entry.hsts_max_age })
                            : t('settings.domainHstsOn', { seconds: entry.hsts_max_age })
                          : t('settings.domainHstsOff')
                      }
                    />
                    <CoverageChip
                      on={entry.sec_nosniff}
                      label={t('settings.domainHeaderNosniff')}
                      stateLabel={entry.sec_nosniff
                        ? t('settings.domainHeaderStateOn')
                        : t('settings.domainHeaderStateOff')}
                    />
                    <CoverageChip
                      on={entry.sec_referrer_policy}
                      label={t('settings.domainHeaderReferrer')}
                      stateLabel={entry.sec_referrer_policy
                        ? t('settings.domainHeaderStateOn')
                        : t('settings.domainHeaderStateOff')}
                    />
                    <CoverageChip
                      on={entry.sec_frame_options}
                      label={t('settings.domainHeaderFrame')}
                      stateLabel={entry.sec_frame_options
                        ? t('settings.domainHeaderStateOn')
                        : t('settings.domainHeaderStateOff')}
                    />
                  </div>
                </div>
              ))}
            </div>
            <p className="text-[11px] text-text-tertiary mt-2">
              {t('settings.domainHstsCacheHint')}
            </p>
            <div className="mt-3 flex items-center justify-between gap-3 bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
              <div className="min-w-0">
                <div className="text-[12px] font-medium">
                  {t('settings.domainOrphanSummary', { count: hostnameOverview.orphan_count })}
                </div>
                <div className="text-[11px] text-text-tertiary mt-0.5">
                  {t('settings.domainCleanupDesc')}
                </div>
              </div>
              <Button
                size="sm"
                className="shrink-0 whitespace-nowrap"
                disabled={hostnameOverview.orphan_count === 0 || cleanupBusy}
                onClick={() => setShowOrphanCleanup(true)}
              >
                {t('settings.domainCleanup')}
              </Button>
            </div>
          </>
        )}
        {hostnameOverviewError && hostnameOverview !== null && (
          <p className="text-[11px] text-error mt-2">{t('settings.domainSettingsError')}</p>
        )}
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
                } catch (e) { addToast('error', formatError(e)); }
              }}
            />
          </div>
        </div>
      </section>

      {/* Performance */}
      <section className="mb-8">
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('settings.performance')}
        </h2>
        <div className="flex items-center justify-between bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
          <div>
            <div className="text-[13px] font-medium">{t('settings.workerProcesses')}</div>
            <div className="text-[11px] text-text-tertiary mt-0.5">{t('settings.workerProcessesDesc')}</div>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={async () => {
                const next = workerProcesses === 'auto' ? '2' : 'auto';
                try {
                  await api.setSetting('worker_processes', next);
                  setWorkerProcesses(next);
                } catch (e) { addToast('error', formatError(e)); }
              }}
              className={cn(
                'px-3 py-[5px] text-[12px] border rounded-[var(--radius-sm)] cursor-pointer',
                workerProcesses === 'auto'
                  ? 'bg-accent-light text-accent border-accent font-medium'
                  : 'bg-bg-primary text-text-secondary border-border hover:bg-bg-hover',
              )}
            >
              {t('settings.workerProcessesAuto')}
            </button>
            {workerProcesses !== 'auto' && (
              <input
                type="number"
                min="1"
                max="64"
                className="w-16 px-2 py-[5px] border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-primary text-text-primary text-center outline-none focus:border-accent"
                value={workerProcesses}
                onChange={(e) => setWorkerProcesses(e.target.value)}
                onBlur={async () => {
                  const val = Math.max(1, Math.min(64, parseInt(workerProcesses) || 2));
                  try {
                    await api.setSetting('worker_processes', String(val));
                    setWorkerProcesses(String(val));
                  } catch (e) {
                    addToast('error', formatError(e));
                    api.getSetting('worker_processes')
                      .then((current) => setWorkerProcesses(current || '2'))
                      .catch(() => {});
                  }
                }}
              />
            )}
          </div>
        </div>
        <div className="mt-3 flex items-center justify-between bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
          <div>
            <div className="text-[13px] font-medium">{t('settings.http2')}</div>
            <div className="text-[11px] text-text-tertiary mt-0.5">{t('settings.http2Desc')}</div>
          </div>
          <span
            className={cn(
              'text-[12px] font-medium',
              capabilities?.http2 ? 'text-success' : 'text-text-tertiary',
            )}
          >
            {capabilitiesLoading
              ? '—'
              : !capabilities?.probed
                ? t('settings.http2Unknown')
                : capabilities.http2
                  ? t('settings.http2Enabled')
                  : t('settings.http2Unsupported')}
          </span>
        </div>
      </section>

      {/* Request Parameter Defaults */}
      <section className="mb-8">
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('settings.requestDefaults')}
        </h2>
        <p className="text-[11px] text-text-tertiary mb-3">
          {t('settings.requestDefaultsDesc')}
        </p>
        <div className="border border-border rounded-[var(--radius-sm)] divide-y divide-border">
          {requestParamCatalog && requestDefaultDrafts ? (
            requestParamCatalog.params.map((info) => {
              const directive = info.directive;
              const invalid = !!requestDefaultErrors[directive];
              return (
                <div key={directive} className="px-4 py-3">
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <div className="text-[13px] font-medium">
                        {t(REQUEST_PARAM_LABEL_KEYS[directive])}
                      </div>
                      <div className="text-[11px] text-text-tertiary mt-0.5">
                        {t(REQUEST_PARAM_HINT_KEYS[directive])}
                      </div>
                    </div>
                    <input
                      value={requestDefaultDrafts[directive]}
                      spellCheck={false}
                      onChange={(e) => {
                        const value = e.target.value;
                        setRequestDefaultDrafts((current) =>
                          current ? { ...current, [directive]: value } : current,
                        );
                        clearRequestDefaultError(directive);
                      }}
                      onBlur={() => saveRequestDefault(directive)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') e.currentTarget.blur();
                      }}
                      className={cn(
                        'w-[132px] px-3 py-[5px] border rounded-[var(--radius-sm)] text-[12px] bg-bg-primary text-text-primary text-right outline-none focus:border-accent',
                        invalid ? 'border-error' : 'border-border',
                      )}
                    />
                  </div>
                  {invalid && (
                    <p className="text-[11px] text-error mt-1 text-right">
                      {requestDefaultErrorMessage(directive)}
                    </p>
                  )}
                </div>
              );
            })
          ) : (
            <div className="px-4 py-3 text-[12px] text-text-tertiary">{t('common.loading')}</div>
          )}
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
              className="w-16 px-2 py-[5px] border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-primary text-text-primary text-center outline-none focus:border-accent"
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
          <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
            <label htmlFor="recovery-passphrase" className="block text-[13px] font-medium mb-1">{t('settings.passphrase')}</label>
            <input
              id="recovery-passphrase"
              type="password"
              autoComplete="new-password"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              className="w-full px-3 py-2 border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-primary text-text-primary outline-none focus:border-accent"
            />
            <div className="text-[11px] text-text-tertiary mt-1">{t('settings.passphraseHint')}</div>
          </div>
          <div className="flex items-center justify-between bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
            <div>
              <div className="text-[13px] font-medium">{t('settings.export')}</div>
              <div className="text-[11px] text-text-tertiary mt-0.5">
                {t('settings.recoveryDesc')}
              </div>
            </div>
            <Button size="sm" className="shrink-0 whitespace-nowrap" disabled={recoveryBusy} onClick={handleExport}>
              <Download className="w-3.5 h-3.5" />
              {t('settings.exportAction')}
            </Button>
          </div>
          <div className="flex items-center justify-between bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3">
            <div>
              <div className="text-[13px] font-medium">{t('settings.import')}</div>
              <div className="text-[11px] text-text-tertiary mt-0.5">
                {t('settings.restoreDesc')}
              </div>
            </div>
            <Button size="sm" className="shrink-0 whitespace-nowrap" disabled={recoveryBusy} onClick={handleImportFile}>
              <Upload className="w-3.5 h-3.5" />
              {t('settings.importAction')}
            </Button>
          </div>
        </div>
      </section>

      {/* About */}
      <section>
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('settings.about')}
        </h2>
        <div className="space-y-2 text-[12px] text-text-secondary">
          <div className="flex justify-between">
            <span>{t('settings.version')}</span>
            <span>{appVersion}</span>
          </div>
          <div className="flex justify-between">
            <span>{t('settings.author')}</span>
            <span>Xiaohua Pan</span>
          </div>
        </div>
      </section>

      <ConfirmDialog
        open={showImportConfirm}
        onClose={() => {
          setShowImportConfirm(false);
          setRecoveryPath('');
          setRecoveryPreview(null);
        }}
        onConfirm={handleImport}
        title={t('settings.import')}
        message={t('settings.importConfirm', {
          proxies: recoveryPreview?.proxy_count ?? 0,
          certificates: recoveryPreview?.certificate_count ?? 0,
          hosts: recoveryPreview?.host_count ?? 0,
          credentials: recoveryPreview?.dns_credential_count ?? 0,
        })}
        danger
      />
      <ConfirmDialog
        open={showOrphanCleanup}
        onClose={() => setShowOrphanCleanup(false)}
        onConfirm={handleCleanupOrphans}
        title={t('settings.domainCleanupTitle')}
        message={t('settings.domainCleanupConfirm', {
          count: hostnameOverview?.orphan_count ?? 0,
        })}
        detail={
          (hostnameOverview?.orphan_hsts_count ?? 0) > 0
            ? t('settings.domainCleanupHstsWarning', {
                count: hostnameOverview?.orphan_hsts_count ?? 0,
              })
            : undefined
        }
        confirmText={t('settings.domainCleanup')}
        danger
      />
      </div>
      </div>
    </>
  );
}

function CoverageChip({
  label,
  on,
  stateLabel,
}: {
  label: string;
  on: boolean;
  stateLabel?: string;
}) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px]',
        on ? 'bg-success-bg text-success' : 'bg-bg-hover text-text-tertiary',
      )}
    >
      <span>{label}</span>
      {stateLabel && <span className="font-medium">{stateLabel}</span>}
    </span>
  );
}
