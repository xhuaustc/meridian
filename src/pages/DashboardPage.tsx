import { useEffect, useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { Plus, Search, Pencil, Copy, Trash2, BarChart3 } from 'lucide-react';
import { Button } from '../components/ui/Button';
import { Badge } from '../components/ui/Badge';
import { Toggle } from '../components/ui/Toggle';
import { ConfirmDialog } from '../components/ui/Dialog';
import { useProxyStore } from '../stores/proxy-store';
import { useCertStore } from '../stores/cert-store';
import { useToastStore } from '../stores/toast-store';
import { checkExpiringCerts, createProxy } from '../lib/api';
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

function getRoute(rule: ProxyRule): { from: string; to: string } {
  const isStream = rule.proxy_type === 'stream_tcp' || rule.proxy_type === 'stream_udp';
  const from = isStream
    ? `:${rule.listen_port}`
    : `${rule.domain || ''}:${rule.listen_port}${rule.path_prefix || '/'}`;
  const to = `${rule.upstream_host}:${rule.upstream_port}`;
  return { from, to };
}

export function DashboardPage() {
  const { t } = useTranslation('common');
  const navigate = useNavigate();
  const { proxies, fetchProxies, toggleProxy, deleteProxy } = useProxyStore();
  const { certificates, fetchCertificates } = useCertStore();
  const addToast = useToastStore((s) => s.addToast);

  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState<FilterType>('all');
  const [deleteTarget, setDeleteTarget] = useState<ProxyRule | null>(null);
  const [expiringCount, setExpiringCount] = useState(0);

  useEffect(() => {
    fetchProxies();
    fetchCertificates();
    checkExpiringCerts(30)
      .then((certs) => setExpiringCount(certs.length))
      .catch(() => {});
  }, [fetchProxies, fetchCertificates]);

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
    const httpCount = proxies.filter(
      (p) => p.proxy_type === 'http',
    ).length;
    const streamCount = proxies.filter(
      (p) => p.proxy_type === 'stream_tcp' || p.proxy_type === 'stream_udp',
    ).length;
    return { active, total: proxies.length, httpCount, streamCount };
  }, [proxies]);

  const handleToggle = async (rule: ProxyRule) => {
    try {
      await toggleProxy(rule.id, !rule.enabled);
      addToast('success', t('dashboard.toggleSuccess'));
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await deleteProxy(deleteTarget.id);
      addToast('success', t('dashboard.deleteSuccess'));
    } catch (e) {
      addToast('error', String(e));
    }
    setDeleteTarget(null);
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
        tls_mode: rule.tls_mode,
        certificate_id: rule.certificate_id,
        access_list_id: rule.access_list_id,
        websocket: rule.websocket,
      });
      await fetchProxies();
      addToast('success', t('dashboard.copySuccess'));
    } catch (e) {
      addToast('error', String(e));
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
    <div>
      {/* Header */}
      <div className="flex items-center justify-between mb-5">
        <h1 className="text-[18px] font-semibold tracking-[-0.02em]">
          {t('dashboard.title')}
        </h1>
        <div className="flex gap-2">
          <Button variant="primary" onClick={() => navigate('/proxy/new')}>
            <Plus className="w-3.5 h-3.5" />
            {t('dashboard.addProxy')}
          </Button>
        </div>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-4 gap-3 mb-5">
        <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3.5">
          <div className="text-[11px] text-text-tertiary uppercase tracking-[0.03em]">
            {t('dashboard.activeProxies')}
          </div>
          <div className="text-[22px] font-semibold tracking-[-0.02em] mt-0.5 text-success">
            {stats.active}
          </div>
          <div className="text-[11px] text-text-tertiary mt-0.5">
            {t('dashboard.totalRules', { count: stats.total })}
          </div>
        </div>
        <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3.5">
          <div className="text-[11px] text-text-tertiary uppercase tracking-[0.03em]">
            {t('dashboard.httpHttps')}
          </div>
          <div className="text-[22px] font-semibold tracking-[-0.02em] mt-0.5">
            {stats.httpCount}
          </div>
          <div className="text-[11px] text-text-tertiary mt-0.5">
            {t('dashboard.layer7')}
          </div>
        </div>
        <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3.5">
          <div className="text-[11px] text-text-tertiary uppercase tracking-[0.03em]">
            {t('dashboard.tcpUdp')}
          </div>
          <div className="text-[22px] font-semibold tracking-[-0.02em] mt-0.5">
            {stats.streamCount}
          </div>
          <div className="text-[11px] text-text-tertiary mt-0.5">
            {t('dashboard.layer4')}
          </div>
        </div>
        <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3.5">
          <div className="text-[11px] text-text-tertiary uppercase tracking-[0.03em]">
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
        </div>
      </div>

      {/* Search & Filter */}
      <div className="flex items-center gap-2 mb-4">
        <div className="flex-1 relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text-tertiary" />
          <input
            className="w-full pl-8 pr-3 py-[7px] border border-border rounded-[var(--radius-sm)] text-[12.5px] bg-bg-secondary outline-none text-text-primary placeholder:text-text-tertiary focus:border-accent"
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
              'px-2.5 py-[5px] border rounded-[20px] text-[11.5px] cursor-pointer',
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
        <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] py-16 flex flex-col items-center justify-center">
          <BarChart3 className="w-10 h-10 text-text-tertiary mb-3" />
          <p className="text-[13px] font-medium text-text-secondary">
            {t('dashboard.emptyTitle')}
          </p>
          <p className="text-[12px] text-text-tertiary mt-1">
            {t('dashboard.emptyDesc')}
          </p>
        </div>
      ) : (
        <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] overflow-hidden">
          <table className="w-full border-collapse">
            <thead>
              <tr>
                <th className="text-left px-4 py-2.5 text-[11px] font-semibold text-text-tertiary uppercase tracking-[0.03em] bg-bg-sidebar border-b border-border w-[30%]">
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
                return (
                  <tr
                    key={rule.id}
                    className="group hover:bg-bg-primary border-b border-border last:border-b-0"
                  >
                    <td className="px-4 py-3 text-[13px]">
                      <div className="font-medium text-text-primary">{rule.name}</div>
                    </td>
                    <td className="px-4 py-3">
                      <Badge variant={getBadgeVariant(displayType)}>
                        {displayType}
                      </Badge>
                    </td>
                    <td className="px-4 py-3 font-mono text-[12px] text-text-secondary">
                      {route.from}
                      <span className="text-text-tertiary mx-1.5">&rarr;</span>
                      <span className="text-text-primary">{route.to}</span>
                    </td>
                    <td className="px-4 py-3">
                      <span
                        className={cn(
                          'inline-flex items-center gap-1.5 text-[12px]',
                          rule.enabled ? 'text-success' : 'text-text-tertiary',
                        )}
                      >
                        <span
                          className={cn(
                            'w-[7px] h-[7px] rounded-full',
                            rule.enabled ? 'bg-success' : 'bg-text-tertiary',
                          )}
                        />
                        {rule.enabled
                          ? t('dashboard.statusActive')
                          : t('dashboard.statusOff')}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <Toggle
                        checked={rule.enabled}
                        onChange={() => handleToggle(rule)}
                      />
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
                        <button
                          onClick={() => navigate(`/proxy/${rule.id}`)}
                          className="w-7 h-7 flex items-center justify-center rounded text-text-secondary hover:bg-bg-sidebar hover:text-text-primary"
                          title={t('dashboard.edit')}
                        >
                          <Pencil className="w-[15px] h-[15px]" />
                        </button>
                        <button
                          onClick={() => handleCopy(rule)}
                          className="w-7 h-7 flex items-center justify-center rounded text-text-secondary hover:bg-bg-sidebar hover:text-text-primary"
                          title={t('dashboard.copy')}
                        >
                          <Copy className="w-[15px] h-[15px]" />
                        </button>
                        <button
                          onClick={() => setDeleteTarget(rule)}
                          className="w-7 h-7 flex items-center justify-center rounded text-text-secondary hover:bg-bg-sidebar hover:text-error"
                          title={t('dashboard.delete')}
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

      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleDelete}
        title={t('common.delete')}
        message={t('dashboard.confirmDelete', { name: deleteTarget?.name })}
        confirmText={t('common.delete')}
        danger
      />
    </div>
  );
}
