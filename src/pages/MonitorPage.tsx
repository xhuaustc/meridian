// SPEC: FEAT-002-proxy-monitoring/spec.md | TASK-006
import { useEffect, useState, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useSearchParams } from 'react-router-dom';
import { RefreshCw, BarChart3 } from 'lucide-react';
import { ContentToolbar } from '../components/layout/ContentToolbar';
import { Select } from '../components/ui/Select';
import {
  AreaChart,
  Area,
  LineChart,
  Line,
  PieChart,
  Pie,
  Cell,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import { Button } from '../components/ui/Button';
import { useProxyStore } from '../stores/proxy-store';
import { getProxyMetrics } from '../lib/api';
import { cn } from '../lib/utils';
import type { ProxyMetrics } from '../types';

type TimeRange = '1h' | '6h' | '24h';

const PIE_COLORS: Record<string, string> = {
  '2xx': '#16a34a',
  '3xx': '#2563eb',
  '4xx': '#d97706',
  '5xx': '#dc2626',
};

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const val = bytes / Math.pow(1024, i);
  return `${val < 10 ? val.toFixed(1) : Math.round(val)} ${units[i]}`;
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function formatTime(iso: string, range: TimeRange): string {
  const d = new Date(iso);
  const hh = String(d.getHours()).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  if (range === '24h') {
    return `${hh}:${mm}`;
  }
  return `${hh}:${mm}`;
}

export function MonitorPage() {
  const { t } = useTranslation('common');
  const { proxies, fetchProxies } = useProxyStore();
  const [searchParams] = useSearchParams();

  const [ruleId, setRuleId] = useState<string | undefined>(() => searchParams.get('proxyId') || undefined);
  const [timeRange, setTimeRange] = useState<TimeRange>('1h');
  const [metrics, setMetrics] = useState<ProxyMetrics | null>(null);
  const [loading, setLoading] = useState(false);
  const timerRef = useRef<ReturnType<typeof setInterval> | undefined>(undefined);

  const loadMetrics = useCallback(async () => {
    try {
      const data = await getProxyMetrics(ruleId, timeRange);
      setMetrics(data);
    } catch {
      // silently fail — metrics are non-critical
    } finally {
      setLoading(false);
    }
  }, [ruleId, timeRange]);

  // Initial load + on filter change
  useEffect(() => {
    fetchProxies();
  }, [fetchProxies]);

  useEffect(() => {
    setLoading(true);
    loadMetrics();
  }, [loadMetrics]);

  // Auto-refresh every 30s
  useEffect(() => {
    timerRef.current = setInterval(loadMetrics, 30_000);
    return () => clearInterval(timerRef.current);
  }, [loadMetrics]);

  const handleRefresh = () => {
    setLoading(true);
    loadMetrics();
  };

  const summary = metrics?.summary;
  const timeSeries = metrics?.time_series ?? [];
  const statusDist = metrics?.status_distribution ?? [];
  const hasData = summary && summary.total_requests > 0;

  const timeRanges: { key: TimeRange; label: string }[] = [
    { key: '1h', label: t('monitor.timeRange1h') },
    { key: '6h', label: t('monitor.timeRange6h') },
    { key: '24h', label: t('monitor.timeRange24h') },
  ];

  // Format time series for charts
  const chartData = timeSeries.map((b) => ({
    ...b,
    label: formatTime(b.timestamp, timeRange),
  }));

  return (
    <>
      <ContentToolbar title={t('monitor.title')}>
        <Select
          className="w-44"
          value={ruleId ?? ''}
          onChange={(e) => setRuleId(e.target.value || undefined)}
        >
          <option value="">{t('monitor.allRules')}</option>
          {proxies.map((r) => (
            <option key={r.id} value={r.id}>
              {r.name}
            </option>
          ))}
        </Select>

        <div className="flex bg-bg-secondary border border-border rounded-[var(--radius-sm)] overflow-hidden">
          {timeRanges.map((tr) => (
            <button
              key={tr.key}
              className={cn(
                'px-3 py-[6px] text-[12px] font-medium cursor-pointer transition-colors',
                timeRange === tr.key
                  ? 'bg-accent text-white'
                  : 'text-text-secondary hover:text-text-primary hover:bg-bg-hover',
              )}
              onClick={() => setTimeRange(tr.key)}
            >
              {tr.label}
            </button>
          ))}
        </div>

        <Button variant="ghost" size="icon" onClick={handleRefresh}>
          <RefreshCw className={cn('w-3.5 h-3.5', loading && 'animate-spin')} />
        </Button>
      </ContentToolbar>
      <div className="p-6 overflow-y-auto flex-1">
        {/* Empty state */}
        {!hasData && !loading && (
          <div className="flex flex-col items-center justify-center py-20 text-center">
            <div className="w-12 h-12 rounded-full bg-bg-hover flex items-center justify-center mb-4">
              <BarChart3 className="w-6 h-6 text-text-tertiary" />
            </div>
            <div className="text-[13px] font-medium text-text-secondary mb-1">
              {t('monitor.emptyTitle')}
            </div>
            <div className="text-[12px] text-text-tertiary">
              {t('monitor.emptyDesc')}
            </div>
          </div>
        )}

        {/* Stats cards */}
        {(hasData || loading) && (
          <>
            <div className="grid grid-cols-4 gap-3 mb-5">
              <StatCard
                label={t('monitor.totalRequests')}
                value={summary ? formatNumber(summary.total_requests) : '-'}
                color="text-accent"
              />
              <StatCard
                label={t('monitor.errorRate')}
                value={summary ? `${(summary.error_rate * 100).toFixed(1)}%` : '-'}
                color={summary && summary.error_rate > 0.05 ? 'text-error' : 'text-success'}
              />
              <StatCard
                label={t('monitor.avgLatency')}
                value={summary ? `${summary.avg_latency_ms.toFixed(0)} ms` : '-'}
                color="text-text-primary"
              />
              <StatCard
                label={t('monitor.bandwidth')}
                value={summary ? formatBytes(summary.total_bytes) : '-'}
                color="text-text-primary"
              />
            </div>

            {/* Request Volume — Area Chart */}
            <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] p-4 mb-4">
              <div className="text-[12px] font-medium text-text-secondary mb-3">
                {t('monitor.requestVolume')}
              </div>
              <ResponsiveContainer width="100%" height={200}>
                <AreaChart data={chartData}>
                  <defs>
                    <linearGradient id="fillReqs" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="var(--color-accent)" stopOpacity={0.2} />
                      <stop offset="100%" stopColor="var(--color-accent)" stopOpacity={0} />
                    </linearGradient>
                  </defs>
                  <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
                  <XAxis
                    dataKey="label"
                    tick={{ fontSize: 10, fill: 'var(--color-text-tertiary)' }}
                    interval="preserveStartEnd"
                    tickLine={false}
                    axisLine={{ stroke: 'var(--color-border)' }}
                  />
                  <YAxis
                    tick={{ fontSize: 10, fill: 'var(--color-text-tertiary)' }}
                    tickLine={false}
                    axisLine={false}
                    width={40}
                  />
                  <Tooltip
                    contentStyle={{
                      fontSize: 11,
                      background: 'var(--color-bg-secondary)',
                      border: '1px solid var(--color-border)',
                      borderRadius: 6,
                    }}
                  />
                  <Area
                    type="monotone"
                    dataKey="requests"
                    name={t('monitor.requests')}
                    stroke="var(--color-accent)"
                    fill="url(#fillReqs)"
                    strokeWidth={1.5}
                  />
                </AreaChart>
              </ResponsiveContainer>
            </div>

            {/* Bottom row: Latency + Status Distribution */}
            <div className="grid grid-cols-2 gap-4">
              {/* Response Time — Line Chart */}
              <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] p-4">
                <div className="text-[12px] font-medium text-text-secondary mb-3">
                  {t('monitor.responseTime')}
                </div>
                <ResponsiveContainer width="100%" height={180}>
                  <LineChart data={chartData}>
                    <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
                    <XAxis
                      dataKey="label"
                      tick={{ fontSize: 10, fill: 'var(--color-text-tertiary)' }}
                      interval="preserveStartEnd"
                      tickLine={false}
                      axisLine={{ stroke: 'var(--color-border)' }}
                    />
                    <YAxis
                      tick={{ fontSize: 10, fill: 'var(--color-text-tertiary)' }}
                      tickLine={false}
                      axisLine={false}
                      width={40}
                    />
                    <Tooltip
                      contentStyle={{
                        fontSize: 11,
                        background: 'var(--color-bg-secondary)',
                        border: '1px solid var(--color-border)',
                        borderRadius: 6,
                      }}
                    />
                    <Line
                      type="monotone"
                      dataKey="avg_latency_ms"
                      name={t('monitor.latency')}
                      stroke="var(--color-warning)"
                      strokeWidth={1.5}
                      dot={false}
                    />
                  </LineChart>
                </ResponsiveContainer>
              </div>

              {/* Status Distribution — Pie Chart */}
              <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] p-4">
                <div className="text-[12px] font-medium text-text-secondary mb-3">
                  {t('monitor.statusDistribution')}
                </div>
                {statusDist.length > 0 ? (
                  <div className="flex items-center gap-4">
                    <ResponsiveContainer width="50%" height={180}>
                      <PieChart>
                        <Pie
                          data={statusDist}
                          dataKey="count"
                          nameKey="group"
                          cx="50%"
                          cy="50%"
                          innerRadius={40}
                          outerRadius={70}
                          strokeWidth={1}
                          stroke="var(--color-bg-secondary)"
                        >
                          {statusDist.map((entry) => (
                            <Cell key={entry.group} fill={PIE_COLORS[entry.group] ?? '#888'} />
                          ))}
                        </Pie>
                        <Tooltip
                          contentStyle={{
                            fontSize: 11,
                            background: 'var(--color-bg-secondary)',
                            border: '1px solid var(--color-border)',
                            borderRadius: 6,
                          }}
                        />
                      </PieChart>
                    </ResponsiveContainer>
                    <div className="flex flex-col gap-2">
                      {statusDist.map((s) => {
                        const total = statusDist.reduce((a, b) => a + b.count, 0);
                        const pct = total > 0 ? ((s.count / total) * 100).toFixed(1) : '0';
                        return (
                          <div key={s.group} className="flex items-center gap-2 text-[12px]">
                            <span
                              className="w-2.5 h-2.5 rounded-full"
                              style={{ background: PIE_COLORS[s.group] ?? '#888' }}
                            />
                            <span className="text-text-secondary">{s.group}</span>
                            <span className="text-text-primary font-medium">{pct}%</span>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                ) : (
                  <div className="flex items-center justify-center h-[180px] text-[12px] text-text-tertiary">
                    -
                  </div>
                )}
              </div>
            </div>
          </>
        )}
      </div>
    </>
  );
}

function StatCard({
  label,
  value,
  color,
}: {
  label: string;
  value: string;
  color: string;
}) {
  return (
    <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3.5">
      <div className="text-[11px] text-text-tertiary uppercase tracking-[0.03em]">
        {label}
      </div>
      <div className={cn('text-[22px] font-semibold tracking-[-0.02em] mt-0.5', color)}>
        {value}
      </div>
    </div>
  );
}
