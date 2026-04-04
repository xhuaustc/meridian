import { useEffect, useState, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useSearchParams } from 'react-router-dom';
import { RefreshCw, ClipboardList } from 'lucide-react';
import { Button } from '../components/ui/Button';
import { Select } from '../components/ui/Select';
import { ConfirmDialog } from '../components/ui/Dialog';
import { useProxyStore } from '../stores/proxy-store';
import { useToastStore } from '../stores/toast-store';
import * as api from '../lib/api';
import { cn } from '../lib/utils';

type LogTab = 'access' | 'error';

function colorizeStatusCode(code: string): string {
  const num = parseInt(code);
  if (num >= 200 && num < 300) return 'text-success';
  if (num >= 300 && num < 400) return 'text-warning';
  if (num >= 400) return 'text-error';
  return '';
}

function renderLogLine(line: string) {
  // Try to find and color HTTP status codes (3-digit numbers)
  const parts = line.split(/(\b[1-5]\d{2}\b)/);
  return parts.map((part, i) => {
    if (/^[1-5]\d{2}$/.test(part)) {
      return (
        <span key={i} className={colorizeStatusCode(part)}>
          {part}
        </span>
      );
    }
    return <span key={i}>{part}</span>;
  });
}

export function LogsPage() {
  const { t } = useTranslation('common');
  const addToast = useToastStore((s) => s.addToast);
  const { proxies, fetchProxies } = useProxyStore();
  const [searchParams] = useSearchParams();
  const logEndRef = useRef<HTMLDivElement>(null);

  const [tab, setTab] = useState<LogTab>('access');
  const [ruleId, setRuleId] = useState<string>(() => searchParams.get('proxyId') || '');
  const [lines, setLines] = useState<string[]>([]);
  const [totalLines, setTotalLines] = useState(0);
  const [loading, setLoading] = useState(false);
  const [showClear, setShowClear] = useState(false);

  useEffect(() => {
    fetchProxies();
  }, [fetchProxies]);

  const fetchLogs = useCallback(async () => {
    setLoading(true);
    try {
      const result =
        tab === 'access'
          ? await api.readAccessLog(500, ruleId || undefined)
          : await api.readErrorLog(500);
      setLines(result.lines);
      setTotalLines(result.total_lines);
    } catch {
      setLines([]);
      setTotalLines(0);
    }
    setLoading(false);
  }, [tab, ruleId]);

  useEffect(() => {
    fetchLogs();
  }, [fetchLogs]);

  // Auto-refresh every 2 seconds
  useEffect(() => {
    const interval = setInterval(() => {
      fetchLogs();
    }, 2000);
    return () => clearInterval(interval);
  }, [fetchLogs]);

  // Auto-scroll only if user is near the bottom
  const logContainerRef = useRef<HTMLDivElement>(null);
  const isNearBottom = useRef(true);

  const handleScroll = useCallback(() => {
    const el = logContainerRef.current;
    if (!el) return;
    isNearBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 60;
  }, []);

  useEffect(() => {
    if (isNearBottom.current) {
      logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [lines]);

  const handleClear = async () => {
    try {
      await api.clearLogs();
      setLines([]);
      setTotalLines(0);
      addToast('success', t('logs.clearSuccess'));
    } catch (e) {
      addToast('error', String(e));
    }
  };

  return (
    <div>
      <div className="flex items-center justify-between mb-5">
        <h1 className="text-[18px] font-semibold tracking-[-0.02em]">
          {t('logs.title')}
        </h1>
        <div className="flex items-center gap-2">
          <button
            onClick={() => setTab('access')}
            className={cn(
              'px-2.5 py-[5px] border rounded-[20px] text-[11.5px] cursor-pointer',
              tab === 'access'
                ? 'bg-accent-light text-accent border-[#bfdbfe] dark:border-accent/40'
                : 'bg-bg-secondary text-text-secondary border-border hover:bg-bg-hover',
            )}
          >
            {t('logs.accessLog')}
          </button>
          <button
            onClick={() => setTab('error')}
            className={cn(
              'px-2.5 py-[5px] border rounded-[20px] text-[11.5px] cursor-pointer',
              tab === 'error'
                ? 'bg-accent-light text-accent border-[#bfdbfe] dark:border-accent/40'
                : 'bg-bg-secondary text-text-secondary border-border hover:bg-bg-hover',
            )}
          >
            {t('logs.errorLog')}
          </button>
          {tab === 'access' && (
            <Select
              className="w-40"
              value={ruleId}
              onChange={(e) => setRuleId(e.target.value)}
            >
              <option value="">{t('monitor.allRules')}</option>
              {proxies.map((r) => (
                <option key={r.id} value={r.id}>
                  {r.name}
                </option>
              ))}
            </Select>
          )}
          <Button size="sm" onClick={fetchLogs} disabled={loading}>
            <RefreshCw className={cn('w-3 h-3', loading && 'animate-spin')} />
            {t('logs.refresh')}
          </Button>
          <Button size="sm" onClick={() => setShowClear(true)}>
            {t('logs.clear')}
          </Button>
        </div>
      </div>

      {totalLines > 0 && (
        <div className="text-[11px] text-text-tertiary mb-2">
          {t('logs.totalLines', { count: totalLines })}
        </div>
      )}

      <div
        ref={logContainerRef}
        onScroll={handleScroll}
        className="bg-[#1c1917] rounded-[var(--radius-md)] p-4 font-mono text-[11.5px] leading-[1.8] text-[#a8a29e] max-h-[calc(100vh-200px)] overflow-y-auto"
      >
        {lines.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center">
            <ClipboardList className="w-8 h-8 text-[#57534e] mb-2" />
            <p className="text-[12px] text-[#78716c]">{t('logs.emptyTitle')}</p>
            <p className="text-[11px] text-[#57534e] mt-1">{t('logs.emptyDesc')}</p>
          </div>
        ) : (
          <>
            {lines.map((line, i) => (
              <div key={i}>{renderLogLine(line)}</div>
            ))}
            <div className="text-[#78716c]">&#9612;</div>
            <div ref={logEndRef} />
          </>
        )}
      </div>

      <ConfirmDialog
        open={showClear}
        onClose={() => setShowClear(false)}
        onConfirm={handleClear}
        title={t('logs.clear')}
        message={t('logs.clearConfirm')}
        danger
      />
    </div>
  );
}
