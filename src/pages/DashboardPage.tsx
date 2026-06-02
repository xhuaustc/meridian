import { useEffect, useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { Plus, Search, Pencil, Copy, Trash2, BarChart3, ClipboardList, Activity, Power, PowerOff, Globe, Shield, ArrowRight, Lock } from 'lucide-react';
import { ContentToolbar } from '../components/layout/ContentToolbar';
import { Button } from '../components/ui/Button';
import { Badge } from '../components/ui/Badge';
import { SkeletonStats, SkeletonTable } from '../components/ui/Skeleton';
import { Toggle } from '../components/ui/Toggle';
import { ConfirmDialog } from '../components/ui/Dialog';
import { useProxyStore } from '../stores/proxy-store';
import { useCertStore } from '../stores/cert-store';
import { useAccessStore } from '../stores/access-store';
import { useHostsStore } from '../stores/hosts-store';
import { useToastStore } from '../stores/toast-store';
import { useApiError } from '../hooks/useApiError';
import { checkExpiringCerts, createProxy, listProxies, checkHostnameExists, deleteHost, batchToggleProxies, batchDeleteProxies } from '../lib/api';
import { openUrl } from '@tauri-apps/plugin-opener';
import { cn } from '../lib/utils';
import type { ProxyRule } from '../types';

type FilterType = 'all' | 'http' | 'https' | 'tcp' | 'udp';

function getDisplayType(rule: ProxyRule): string {
  if (rule.proxy_type === 'stream_tcp') return 'TCP';
  if (rule.proxy_type === 'stream_udp') return 'UDP';
  if (rule.tls_mode === 'terminate' || rule.tls_mode === 'passthrough') return 'HTTPS';
  return 'HTTP';
}

function getBadgeVariant(type: string): 'http' | 'https' | 'tcp' | 'udp' {
  return type.toLowerCase() as 'http' | 'https' | 'tcp' | 'udp';
}

function getRoute(rule: ProxyRule): { from: string; to: string; href: string | null } {
  const isStream = rule.proxy_type === 'stream_tcp' || rule.proxy_type === 'stream_udp';
  const scheme = rule.tls_mode === 'terminate' || rule.tls_mode === 'passthrough' ? 'https' : 'http';
  const from = isStream
    ? `:${rule.listen_port}`
    : `${scheme}://${rule.domain || ''}:${rule.listen_port}${rule.path_prefix || '/'}`;
  const to = `${rule.upstream_scheme}://${rule.upstream_host}:${rule.upstream_port}`;
  let href: string | null = null;
  if (!isStream && rule.domain) {
    const defaultPort = scheme === 'https' ? 443 : 80;
    const portPart = rule.listen_port === defaultPort ? '' : `:${rule.listen_port}`;
    href = `${scheme}://${rule.domain}${portPart}${rule.path_prefix || '/'}`;
  }
  return { from, to, href };
}

function isLocalDevDomain(domain: string): boolean {
  return (
    domain.endsWith('.local') ||
    domain.endsWith('.test') ||
    domain.endsWith('.localhost') ||
    !domain.includes('.')
  );
}

function getRuleHealth(
  rule: ProxyRule,
  certIds: Set<string>,
  hostnames: Set<string>,
): { labelKey: string; tone: 'success' | 'warning' | 'muted' } {
  if (!rule.enabled) {
    return { labelKey: 'dashboard.statusOff', tone: 'muted' };
  }
  if (rule.tls_mode === 'terminate' && rule.certificate_id && !certIds.has(rule.certificate_id)) {
    return { labelKey: 'dashboard.statusCertMissing', tone: 'warning' };
  }
  if (
    rule.proxy_type === 'http' &&
    rule.domain &&
    isLocalDevDomain(rule.domain) &&
    !hostnames.has(rule.domain)
  ) {
    return { labelKey: 'dashboard.statusHostsMissing', tone: 'warning' };
  }
  return { labelKey: 'dashboard.statusActive', tone: 'success' };
}

export function DashboardPage() {
  const { t } = useTranslation('common');
  const navigate = useNavigate();
  const { proxies, loading, fetchProxies, toggleProxy, deleteProxy } = useProxyStore();
  const { certificates, fetchCertificates } = useCertStore();
  const { lists: accessLists, fetchLists: fetchAccessLists } = useAccessStore();
  const { entries: hostEntries, fetchEntries: fetchHostEntries } = useHostsStore();
  const addToast = useToastStore((s) => s.addToast);
  const formatError = useApiError();

  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState<FilterType>('all');
  const [deleteTarget, setDeleteTarget] = useState<ProxyRule | null>(null);
  const [expiringCount, setExpiringCount] = useState(0);
  const [hostsCleanupTarget, setHostsCleanupTarget] = useState<{ hostname: string; hostEntryId: string } | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [batchDeleteOpen, setBatchDeleteOpen] = useState(false);

  useEffect(() => {
    fetchProxies();
    fetchCertificates();
    fetchAccessLists();
    fetchHostEntries();
    checkExpiringCerts(30)
      .then((certs) => setExpiringCount(certs.length))
      .catch(() => {});
  }, [fetchProxies, fetchCertificates, fetchAccessLists, fetchHostEntries]);

  const filtered = useMemo(() => {
    let result = proxies;
    if (search) {
      const q = search.toLowerCase();
      result = result.filter(
        (r) =>
          r.name.toLowerCase().includes(q) ||
          (r.domain && r.domain.toLowerCase().includes(q)) ||
          r.listen_port.toString().includes(q),
      );
    }
    if (filter !== 'all') {
      result = result.filter((r) => getDisplayType(r).toLowerCase() === filter);
    }
    return result;
  }, [proxies, search, filter]);

  const stats = useMemo(() => {
    const active = proxies.filter((p) => p.enabled).length;
    return { active, total: proxies.length };
  }, [proxies]);

  const certIds = useMemo(() => new Set(certificates.map((cert) => cert.id)), [certificates]);
  const hostnames = useMemo(() => new Set(hostEntries.map((entry) => entry.hostname)), [hostEntries]);

  const handleToggle = async (rule: ProxyRule) => {
    try {
      await toggleProxy(rule.id, !rule.enabled);
      addToast('success', t('dashboard.toggleSuccess'));
    } catch (e) {
      addToast('error', formatError(e));
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    const deletedDomain = deleteTarget.domain;
    try {
      await deleteProxy(deleteTarget.id);
      addToast('success', t('dashboard.deleteSuccess'));
    } catch (e) {
      addToast('error', formatError(e));
      setDeleteTarget(null);
      return;
    }
    setDeleteTarget(null);

    // Check if domain's hosts entry should be cleaned up
    if (deletedDomain) {
      try {
        const allProxies = await listProxies();
        const domainStillUsed = allProxies.rules.some(
          (r) => r.domain === deletedDomain,
        );
        if (!domainStillUsed) {
          const hostEntry = await checkHostnameExists(deletedDomain);
          if (hostEntry) {
            setHostsCleanupTarget({ hostname: deletedDomain, hostEntryId: hostEntry.id });
            return;
          }
        }
      } catch { /* ignore */ }
    }
  };

  const handleCopy = async (rule: ProxyRule) => {
    try {
      await createProxy({
        name: `${rule.name} (copy)`,
        proxy_type: rule.proxy_type,
        listen_port: rule.listen_port + 1,
        listen_host: rule.listen_host,
        domain: rule.domain,
        path_prefix: rule.path_prefix,
        upstream_host: rule.upstream_host,
        upstream_port: rule.upstream_port,
        upstream_scheme: rule.upstream_scheme,
        tls_mode: rule.tls_mode,
        certificate_id: rule.certificate_id,
        access_list_id: rule.access_list_id,
        websocket: rule.websocket,
        keep_alive: rule.keep_alive,
        custom_headers: rule.custom_headers,
        upstream_targets: rule.upstream_targets,
      });
      await fetchProxies();
      addToast('success', t('dashboard.copySuccess'));
    } catch (e) {
      addToast('error', formatError(e));
    }
  };

  const toggleSelect = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleSelectAll = () => {
    if (selected.size === filtered.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(filtered.map((r) => r.id)));
    }
  };

  const handleBatchToggle = async (enabled: boolean) => {
    const ids = Array.from(selected);
    try {
      const count = await batchToggleProxies(ids, enabled);
      addToast('success', t('dashboard.batchToggleSuccess', { count }));
      setSelected(new Set());
      await fetchProxies();
    } catch (e) {
      addToast('error', formatError(e));
    }
  };

  const handleBatchDelete = async () => {
    const ids = Array.from(selected);
    try {
      const count = await batchDeleteProxies(ids);
      addToast('success', t('dashboard.batchDeleteSuccess', { count }));
      setSelected(new Set());
      setBatchDeleteOpen(false);
      await fetchProxies();
    } catch (e) {
      addToast('error', formatError(e));
    }
  };

  const filters: { value: FilterType; label: string }[] = [
    { value: 'all', label: t('dashboard.filterAll') },
    { value: 'http', label: t('dashboard.filterHttp') },
    { value: 'https', label: t('dashboard.filterHttps') },
    { value: 'tcp', label: t('dashboard.filterTcp') },
    { value: 'udp', label: t('dashboard.filterUdp') },
  ];

  return (
    <>
      <ContentToolbar title={t('dashboard.title')}>
        <Button variant="primary" onClick={() => navigate('/proxy/new')}>
          <Plus className="w-3.5 h-3.5" />
          {t('dashboard.addProxy')}
        </Button>
      </ContentToolbar>
      <div className="p-6 overflow-y-auto flex-1">
        {loading && proxies.length === 0 ? (
          <>
            <SkeletonStats />
            <SkeletonTable rows={5} />
          </>
        ) : (
        <>
        {/* Stats — cross-resource overview */}
        <div className="grid grid-cols-4 gap-3 mb-5">
          <button
            onClick={() => {/* already on this page */}}
            className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3.5 text-left cursor-default"
          >
            <div className="flex items-center gap-2 text-[11px] text-text-tertiary uppercase tracking-[0.03em]">
              <BarChart3 className="w-3.5 h-3.5 opacity-50" />
              {t('dashboard.activeProxies')}
            </div>
            <div className="text-[22px] font-semibold tracking-[-0.02em] mt-0.5 text-success">
              {stats.active}
            </div>
            <div className="text-[11px] text-text-tertiary mt-0.5">
              {t('dashboard.totalRules', { count: stats.total })}
            </div>
          </button>
          <button
            onClick={() => navigate('/certs')}
            className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3.5 text-left hover:border-accent/40 transition-colors cursor-pointer"
          >
            <div className="flex items-center gap-2 text-[11px] text-text-tertiary uppercase tracking-[0.03em]">
              <Lock className="w-3.5 h-3.5 opacity-50" />
              {t('dashboard.certificates')}
            </div>
            <div className="text-[22px] font-semibold tracking-[-0.02em] mt-0.5">
              {certificates.length}
            </div>
            <div className={cn('text-[11px] mt-0.5', expiringCount > 0 ? 'text-warning' : 'text-text-tertiary')}>
              {expiringCount > 0
                ? t('dashboard.expiringSoon', { count: expiringCount })
                : '\u00A0'}
            </div>
          </button>
          <button
            onClick={() => navigate('/access')}
            className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3.5 text-left hover:border-accent/40 transition-colors cursor-pointer"
          >
            <div className="flex items-center gap-2 text-[11px] text-text-tertiary uppercase tracking-[0.03em]">
              <Shield className="w-3.5 h-3.5 opacity-50" />
              {t('dashboard.accessLists')}
            </div>
            <div className="text-[22px] font-semibold tracking-[-0.02em] mt-0.5">
              {accessLists.length}
            </div>
            <div className="text-[11px] text-text-tertiary mt-0.5">
              {t('dashboard.accessListsDesc', { count: accessLists.length })}
            </div>
          </button>
          <button
            onClick={() => navigate('/hosts')}
            className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3.5 text-left hover:border-accent/40 transition-colors cursor-pointer"
          >
            <div className="flex items-center gap-2 text-[11px] text-text-tertiary uppercase tracking-[0.03em]">
              <Globe className="w-3.5 h-3.5 opacity-50" />
              {t('dashboard.hostEntries')}
            </div>
            <div className="text-[22px] font-semibold tracking-[-0.02em] mt-0.5">
              {hostEntries.length}
            </div>
            <div className="text-[11px] text-text-tertiary mt-0.5">
              {t('dashboard.hostEntriesDesc', { count: hostEntries.length })}
            </div>
          </button>
        </div>

        {/* Search & Filter */}
        <div className="flex items-center gap-2 mb-4">
          <div className="flex-1 relative">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text-tertiary" />
            <input
              className="w-full pl-8 pr-3 py-[7px] border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-secondary outline-none text-text-primary placeholder:text-text-tertiary focus:border-accent"
              placeholder={t('dashboard.searchPlaceholder')}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
          {filters.map((f) => (
            <button
              key={f.value}
              onClick={() => setFilter(f.value)}
              className={cn(
                'px-2.5 py-[5px] border rounded-[20px] text-[12px] cursor-pointer',
                filter === f.value
                  ? 'bg-accent-light text-accent border-[#bfdbfe] dark:border-accent/40'
                  : 'bg-bg-secondary text-text-secondary border-border hover:bg-bg-hover',
              )}
            >
              {f.label}
            </button>
          ))}
        </div>

        {/* Table */}
        {filtered.length === 0 ? (
          proxies.length === 0 ? (
            /* First-run onboarding empty state */
            <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] py-12 px-8">
              <div className="text-center mb-8">
                <BarChart3 className="w-12 h-12 text-accent mx-auto mb-3 opacity-80" />
                <p className="text-[16px] font-semibold text-text-primary">
                  {t('dashboard.onboardingTitle')}
                </p>
                <p className="text-[12px] text-text-tertiary mt-1.5 max-w-md mx-auto">
                  {t('dashboard.onboardingDesc')}
                </p>
              </div>
              <div className="grid grid-cols-3 gap-4 max-w-2xl mx-auto mb-8">
                <button
                  onClick={() => navigate('/proxy/new')}
                  className="flex flex-col items-center gap-2.5 p-5 bg-bg-primary border border-border rounded-[var(--radius-md)] hover:border-accent hover:bg-accent/5 transition-colors cursor-pointer group"
                >
                  <div className="w-10 h-10 rounded-full bg-accent/10 flex items-center justify-center">
                    <Globe className="w-5 h-5 text-accent" />
                  </div>
                  <span className="text-[12px] font-medium text-text-primary">{t('dashboard.onboardingStep1')}</span>
                  <span className="text-[11px] text-text-tertiary text-center">{t('dashboard.onboardingStep1Desc')}</span>
                  <ArrowRight className="w-3.5 h-3.5 text-text-tertiary group-hover:text-accent transition-colors" />
                </button>
                <button
                  onClick={() => navigate('/certs')}
                  className="flex flex-col items-center gap-2.5 p-5 bg-bg-primary border border-border rounded-[var(--radius-md)] hover:border-accent hover:bg-accent/5 transition-colors cursor-pointer group"
                >
                  <div className="w-10 h-10 rounded-full bg-success/10 flex items-center justify-center">
                    <Shield className="w-5 h-5 text-success" />
                  </div>
                  <span className="text-[12px] font-medium text-text-primary">{t('dashboard.onboardingStep2')}</span>
                  <span className="text-[11px] text-text-tertiary text-center">{t('dashboard.onboardingStep2Desc')}</span>
                  <ArrowRight className="w-3.5 h-3.5 text-text-tertiary group-hover:text-accent transition-colors" />
                </button>
                <button
                  onClick={() => navigate('/hosts')}
                  className="flex flex-col items-center gap-2.5 p-5 bg-bg-primary border border-border rounded-[var(--radius-md)] hover:border-accent hover:bg-accent/5 transition-colors cursor-pointer group"
                >
                  <div className="w-10 h-10 rounded-full bg-warning/10 flex items-center justify-center">
                    <ClipboardList className="w-5 h-5 text-warning" />
                  </div>
                  <span className="text-[12px] font-medium text-text-primary">{t('dashboard.onboardingStep3')}</span>
                  <span className="text-[11px] text-text-tertiary text-center">{t('dashboard.onboardingStep3Desc')}</span>
                  <ArrowRight className="w-3.5 h-3.5 text-text-tertiary group-hover:text-accent transition-colors" />
                </button>
              </div>
              <div className="text-center">
                <Button variant="primary" onClick={() => navigate('/proxy/new')}>
                  <Plus className="w-3.5 h-3.5" />
                  {t('dashboard.addProxy')}
                </Button>
              </div>
            </div>
          ) : (
            /* Filtered empty state */
            <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] py-16 flex flex-col items-center justify-center">
              <Search className="w-10 h-10 text-text-tertiary mb-3" />
              <p className="text-[13px] font-medium text-text-secondary">
                {t('dashboard.noResults')}
              </p>
            </div>
          )
        ) : (
          <div className="card-elevated bg-bg-secondary border border-border rounded-[var(--radius-md)] overflow-hidden">
            <table className="w-full border-collapse">
              <thead>
                <tr>
                  <th className="px-3 py-2.5 bg-bg-sidebar border-b border-border w-10">
                    <input
                      type="checkbox"
                      checked={filtered.length > 0 && selected.size === filtered.length}
                      onChange={toggleSelectAll}
                      className="accent-accent cursor-pointer"
                      aria-label={t('dashboard.selectAll')}
                    />
                  </th>
                  <th className="text-left px-4 py-2.5 text-[11px] font-semibold text-text-tertiary uppercase tracking-[0.03em] bg-bg-sidebar border-b border-border w-[28%]">
                    {t('dashboard.colName')}
                  </th>
                  <th className="text-left px-4 py-2.5 text-[11px] font-semibold text-text-tertiary uppercase tracking-[0.03em] bg-bg-sidebar border-b border-border w-[8%]">
                    {t('dashboard.colType')}
                  </th>
                  <th className="text-left px-4 py-2.5 text-[11px] font-semibold text-text-tertiary uppercase tracking-[0.03em] bg-bg-sidebar border-b border-border w-[32%]">
                    {t('dashboard.colRoute')}
                  </th>
                  <th className="text-left px-4 py-2.5 text-[11px] font-semibold text-text-tertiary uppercase tracking-[0.03em] bg-bg-sidebar border-b border-border w-[10%]">
                    {t('dashboard.colStatus')}
                  </th>
                  <th className="text-left px-4 py-2.5 text-[11px] font-semibold text-text-tertiary uppercase tracking-[0.03em] bg-bg-sidebar border-b border-border w-[8%]">
                    {t('dashboard.colToggle')}
                  </th>
                  <th className="text-left px-4 py-2.5 text-[11px] font-semibold text-text-tertiary uppercase tracking-[0.03em] bg-bg-sidebar border-b border-border w-[12%]" />
                </tr>
              </thead>
              <tbody>
                {filtered.map((rule) => {
                  const displayType = getDisplayType(rule);
                  const route = getRoute(rule);
                  const health = getRuleHealth(rule, certIds, hostnames);
                  return (
                    <tr
                      key={rule.id}
                      className="group hover:bg-bg-primary border-b border-border last:border-b-0"
                    >
                      <td className="px-3 py-3">
                        <input
                          type="checkbox"
                          checked={selected.has(rule.id)}
                          onChange={() => toggleSelect(rule.id)}
                          className="accent-accent cursor-pointer"
                        />
                      </td>
                      <td className="px-4 py-3 text-[13px]">
                        <div className="font-medium text-text-primary">{rule.name}</div>
                      </td>
                      <td className="px-4 py-3">
                        <Badge variant={getBadgeVariant(displayType)}>
                          {displayType}
                        </Badge>
                      </td>
                      <td className="px-4 py-3 font-mono text-[12px] text-text-secondary">
                        {route.href ? (
                          <button
                            className={cn(
                              'hover:underline cursor-pointer bg-transparent border-none p-0 font-mono text-[12px]',
                              rule.enabled ? 'text-accent' : 'text-text-tertiary cursor-not-allowed',
                            )}
                            disabled={!rule.enabled}
                            onClick={(e) => {
                              e.stopPropagation();
                              openUrl(route.href!);
                            }}
                          >
                            {route.from}
                          </button>
                        ) : (
                          route.from
                        )}
                        <span className="text-text-tertiary mx-1.5">&rarr;</span>
                        <span className="text-text-primary">{route.to}</span>
                      </td>
                      <td className="px-4 py-3">
                        <span
                          className={cn(
                            'inline-flex items-center gap-1.5 text-[12px]',
                            health.tone === 'success' && 'text-success',
                            health.tone === 'warning' && 'text-warning',
                            health.tone === 'muted' && 'text-text-tertiary',
                          )}
                        >
                          <span
                            className={cn(
                              'w-[7px] h-[7px] rounded-full',
                              health.tone === 'success' && 'bg-success',
                              health.tone === 'warning' && 'bg-warning',
                              health.tone === 'muted' && 'bg-text-tertiary',
                            )}
                          />
                          {t(health.labelKey)}
                        </span>
                      </td>
                      <td className="px-4 py-3">
                        <Toggle
                          checked={rule.enabled}
                          onChange={() => handleToggle(rule)}
                        />
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex gap-1 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity duration-150 touch:opacity-100">
                          <button
                            onClick={() => navigate(`/logs?proxyId=${rule.id}`)}
                            className="w-7 h-7 flex items-center justify-center rounded text-text-secondary hover:bg-bg-sidebar hover:text-text-primary"
                            title={t('dashboard.logs')}
                            aria-label={t('dashboard.logs')}
                          >
                            <ClipboardList className="w-[15px] h-[15px]" />
                          </button>
                          <button
                            onClick={() => navigate(`/monitor?proxyId=${rule.id}`)}
                            className="w-7 h-7 flex items-center justify-center rounded text-text-secondary hover:bg-bg-sidebar hover:text-text-primary"
                            title={t('dashboard.monitor')}
                            aria-label={t('dashboard.monitor')}
                          >
                            <Activity className="w-[15px] h-[15px]" />
                          </button>
                          <button
                            onClick={() => navigate(`/proxy/${rule.id}`)}
                            className="w-7 h-7 flex items-center justify-center rounded text-text-secondary hover:bg-bg-sidebar hover:text-text-primary"
                            title={t('dashboard.edit')}
                            aria-label={t('dashboard.edit')}
                          >
                            <Pencil className="w-[15px] h-[15px]" />
                          </button>
                          <button
                            onClick={() => handleCopy(rule)}
                            className="w-7 h-7 flex items-center justify-center rounded text-text-secondary hover:bg-bg-sidebar hover:text-text-primary"
                            title={t('dashboard.copy')}
                            aria-label={t('dashboard.copy')}
                          >
                            <Copy className="w-[15px] h-[15px]" />
                          </button>
                          <button
                            onClick={() => setDeleteTarget(rule)}
                            className="w-7 h-7 flex items-center justify-center rounded text-text-secondary hover:bg-bg-sidebar hover:text-error"
                            title={t('dashboard.delete')}
                            aria-label={t('dashboard.delete')}
                          >
                            <Trash2 className="w-[15px] h-[15px]" />
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}

        </>
        )}

        <ConfirmDialog
          open={!!deleteTarget}
          onClose={() => setDeleteTarget(null)}
          onConfirm={handleDelete}
          title={t('common.delete')}
          message={t('dashboard.confirmDelete', { name: deleteTarget?.name })}
          confirmText={t('common.delete')}
          danger
        />

        {/* Floating batch action bar */}
        {selected.size > 0 && (
          <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 flex items-center gap-3 bg-bg-secondary border border-border rounded-[var(--radius-md)] shadow-lg px-5 py-3">
            <span className="text-[13px] font-medium text-text-primary">
              {t('dashboard.selected', { count: selected.size })}
            </span>
            <div className="w-px h-5 bg-border" />
            <button
              onClick={() => handleBatchToggle(true)}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-sm)] text-[12px] bg-success/10 text-success hover:bg-success/20 cursor-pointer"
            >
              <Power className="w-3.5 h-3.5" />
              {t('dashboard.batchEnable')}
            </button>
            <button
              onClick={() => handleBatchToggle(false)}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-sm)] text-[12px] bg-bg-hover text-text-secondary hover:bg-bg-sidebar cursor-pointer"
            >
              <PowerOff className="w-3.5 h-3.5" />
              {t('dashboard.batchDisable')}
            </button>
            <button
              onClick={() => setBatchDeleteOpen(true)}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-sm)] text-[12px] bg-error/10 text-error hover:bg-error/20 cursor-pointer"
            >
              <Trash2 className="w-3.5 h-3.5" />
              {t('dashboard.batchDelete')}
            </button>
          </div>
        )}

        <ConfirmDialog
          open={batchDeleteOpen}
          onClose={() => setBatchDeleteOpen(false)}
          onConfirm={handleBatchDelete}
          title={t('common.delete')}
          message={t('dashboard.batchDeleteConfirm', { count: selected.size })}
          confirmText={t('common.delete')}
          danger
        />

        <ConfirmDialog
          open={!!hostsCleanupTarget}
          onClose={() => setHostsCleanupTarget(null)}
          onConfirm={async () => {
            if (hostsCleanupTarget) {
              try {
                await deleteHost(hostsCleanupTarget.hostEntryId);
                addToast('success', t('hosts.deleteSuccess'));
              } catch (e) {
                addToast('error', formatError(e));
              }
            }
          }}
          title={t('hosts.deletePromptTitle')}
          message={t('hosts.deletePromptMessage', { domain: hostsCleanupTarget?.hostname })}
          confirmText={t('hosts.deletePromptConfirm')}
          danger
        />
      </div>
    </>
  );
}
