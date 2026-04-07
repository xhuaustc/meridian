import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft, Plus, X } from 'lucide-react';
import { Button } from '../ui/Button';
import { Input } from '../ui/Input';
import { Select } from '../ui/Select';
import { Toggle } from '../ui/Toggle';
import { useProxyStore } from '../../stores/proxy-store';
import { useCertStore } from '../../stores/cert-store';
import { useAccessStore } from '../../stores/access-store';
import { useToastStore } from '../../stores/toast-store';
import { useApiError } from '../../hooks/useApiError';
import { checkPortConflict, checkHostnameExists, createHost } from '../../lib/api';
import { Dialog } from '../ui/Dialog';
import { cn } from '../../lib/utils';
import type { ProxyRule, ProxyType, TlsMode, CreateProxyRule, UpdateProxyRule, UpstreamTarget, UpstreamScheme } from '../../types';

type FormProxyType = 'http' | 'https' | 'tcp' | 'udp';

function toFormType(pt: ProxyType, tls: TlsMode): FormProxyType {
  if (pt === 'stream_tcp') return 'tcp';
  if (pt === 'stream_udp') return 'udp';
  if (tls === 'terminate' || tls === 'passthrough') return 'https';
  return 'http';
}

function fromFormType(ft: FormProxyType): { proxy_type: ProxyType; defaultTls: TlsMode } {
  switch (ft) {
    case 'http':
      return { proxy_type: 'http', defaultTls: 'none' };
    case 'https':
      return { proxy_type: 'http', defaultTls: 'terminate' };
    case 'tcp':
      return { proxy_type: 'stream_tcp', defaultTls: 'none' };
    case 'udp':
      return { proxy_type: 'stream_udp', defaultTls: 'none' };
  }
}

interface ProxyFormProps {
  rule?: ProxyRule;
}

export function ProxyForm({ rule }: ProxyFormProps) {
  const { t } = useTranslation('common');
  const navigate = useNavigate();
  const { createProxy, updateProxy } = useProxyStore();
  const { certificates, fetchCertificates } = useCertStore();
  const { lists: accessLists, fetchLists } = useAccessStore();
  const addToast = useToastStore((s) => s.addToast);
  const formatError = useApiError();

  const isEdit = !!rule;

  const [formType, setFormType] = useState<FormProxyType>(
    rule ? toFormType(rule.proxy_type, rule.tls_mode) : 'http',
  );
  const [name, setName] = useState(rule?.name ?? '');
  const [domain, setDomain] = useState(rule?.domain ?? '');
  const [listenPort, setListenPort] = useState(rule?.listen_port?.toString() ?? '');
  const [pathPrefix, setPathPrefix] = useState(rule?.path_prefix ?? '');
  const [upstreamHost, setUpstreamHost] = useState(rule?.upstream_host ?? '');
  const [upstreamPort, setUpstreamPort] = useState(rule?.upstream_port?.toString() ?? '');
  const [upstreamScheme, setUpstreamScheme] = useState<UpstreamScheme>(rule?.upstream_scheme ?? 'http');
  const [tlsMode, setTlsMode] = useState<TlsMode>(rule?.tls_mode ?? 'none');
  const [certificateId, setCertificateId] = useState(rule?.certificate_id ?? '');
  const [accessListId, setAccessListId] = useState(rule?.access_list_id ?? '');
  const [websocket, setWebsocket] = useState(rule?.websocket ?? false);
  const [portWarning, setPortWarning] = useState('');
  const [saving, setSaving] = useState(false);
  const [errors, setErrors] = useState<Record<string, boolean>>({});
  const [customHeaders, setCustomHeaders] = useState<Array<{key: string, value: string}>>([]);
  const [upstreamTargets, setUpstreamTargets] = useState<UpstreamTarget[]>([]);
  const [multiUpstream, setMultiUpstream] = useState(false);
  const [showHostsPrompt, setShowHostsPrompt] = useState(false);
  const [hostsPromptDomain, setHostsPromptDomain] = useState('');
  const [hostsPromptIp, setHostsPromptIp] = useState('127.0.0.1');

  useEffect(() => {
    fetchCertificates();
    fetchLists();
  }, [fetchCertificates, fetchLists]);

  useEffect(() => {
    if (rule?.custom_headers) {
      try {
        const parsed = JSON.parse(rule.custom_headers);
        setCustomHeaders(
          Object.entries(parsed).map(([key, value]) => ({ key, value: value as string }))
        );
      } catch { /* ignore */ }
    }
    if (rule?.upstream_targets) {
      try {
        const targets: UpstreamTarget[] = JSON.parse(rule.upstream_targets);
        if (targets.length > 0) {
          setUpstreamTargets(targets);
          setMultiUpstream(true);
        }
      } catch { /* ignore */ }
    }
  }, [rule]);

  useEffect(() => {
    const { defaultTls } = fromFormType(formType);
    if (formType === 'https') {
      if (tlsMode === 'none') setTlsMode('terminate');
    } else if (formType === 'http') {
      setTlsMode('none');
    } else {
      setTlsMode(defaultTls);
    }
  }, [formType]);

  const isStream = formType === 'tcp' || formType === 'udp';
  const showTls = formType === 'http' || formType === 'https';
  const showWebsocket = formType === 'http' || formType === 'https';

  const checkPortConflicts = useCallback(async () => {
    if (!listenPort) return;
    try {
      const { proxy_type } = fromFormType(formType);
      const conflicts = await checkPortConflict(
        parseInt(listenPort),
        proxy_type,
        isStream ? undefined : domain || undefined,
        isStream ? undefined : pathPrefix || undefined,
        isEdit ? rule?.id : undefined,
      );
      if (conflicts.length > 0) {
        setPortWarning(conflicts.map((c) => c.message).join('; '));
      } else {
        setPortWarning('');
      }
    } catch {
      setPortWarning('');
    }
  }, [listenPort, formType, domain, pathPrefix, isEdit, isStream, rule?.id]);

  const clearError = (field: string) => {
    setErrors((prev) => {
      if (!prev[field]) return prev;
      const next = { ...prev };
      delete next[field];
      return next;
    });
  };

  const handleSave = async () => {
    const errs: Record<string, boolean> = {};
    if (!name.trim()) errs.name = true;
    if (!listenPort) errs.listenPort = true;
    if (!multiUpstream) {
      if (!upstreamHost.trim()) errs.upstreamHost = true;
      if (!upstreamPort) errs.upstreamPort = true;
    }
    if (!isStream && !domain.trim()) errs.domain = true;

    // Port conflict check
    if (listenPort) {
      try {
        const { proxy_type } = fromFormType(formType);
        const conflicts = await checkPortConflict(
          parseInt(listenPort),
          proxy_type,
          isStream ? undefined : domain || undefined,
          isStream ? undefined : pathPrefix || undefined,
          isEdit ? rule?.id : undefined,
        );
        if (conflicts.length > 0) {
          errs.listenPort = true;
          setPortWarning(conflicts.map((c) => c.message).join('; '));
        }
      } catch { /* ignore */ }
    }

    if (Object.keys(errs).length > 0) {
      setErrors(errs);
      // Show first error as toast
      if (errs.name) addToast('error', t('proxyForm.nameRequired'));
      else if (errs.listenPort && !portWarning) addToast('error', t('proxyForm.portRequired'));
      else if (errs.listenPort && portWarning) addToast('error', t('proxyForm.portConflictWarning', { message: portWarning }));
      else if (errs.domain) addToast('error', t('proxyForm.domainHint'));
      else if (errs.upstreamHost || errs.upstreamPort) addToast('error', t('proxyForm.upstreamRequired'));
      return;
    }

    setSaving(true);
    try {
      const { proxy_type } = fromFormType(formType);

      const headersObj: Record<string, string> = {};
      for (const h of customHeaders) {
        if (h.key.trim()) {
          headersObj[h.key.trim()] = h.value;
        }
      }
      const custom_headers = Object.keys(headersObj).length > 0 ? JSON.stringify(headersObj) : null;

      // Serialize upstream targets
      const validTargets = multiUpstream
        ? upstreamTargets.filter((t) => t.host.trim() && t.port > 0)
        : [];
      const upstream_targets = validTargets.length > 0 ? JSON.stringify(validTargets) : null;

      if (isEdit && rule) {
        const input: UpdateProxyRule = {
          name,
          proxy_type,
          listen_port: parseInt(listenPort),
          listen_host: '0.0.0.0',
          domain: isStream ? null : domain || null,
          path_prefix: isStream ? null : pathPrefix || null,
          upstream_host: upstreamHost,
          upstream_port: parseInt(upstreamPort),
          upstream_scheme: upstreamScheme,
          tls_mode: tlsMode,
          certificate_id: tlsMode === 'terminate' ? certificateId || null : null,
          access_list_id: accessListId || null,
          websocket: showWebsocket ? websocket : false,
          custom_headers,
          upstream_targets,
        };
        await updateProxy(rule.id, input);
        addToast('success', t('proxyForm.updateSuccess'));
      } else {
        const input: CreateProxyRule = {
          name,
          proxy_type,
          listen_port: parseInt(listenPort),
          listen_host: '0.0.0.0',
          domain: isStream ? null : domain || null,
          path_prefix: isStream ? null : pathPrefix || null,
          upstream_host: upstreamHost,
          upstream_port: parseInt(upstreamPort),
          upstream_scheme: upstreamScheme,
          tls_mode: tlsMode,
          certificate_id: tlsMode === 'terminate' ? certificateId || null : null,
          access_list_id: accessListId || null,
          websocket: showWebsocket ? websocket : false,
          custom_headers,
          upstream_targets,
        };
        await createProxy(input);
        addToast('success', t('proxyForm.createSuccess'));

        // Check if domain needs a hosts entry
        if (domain.trim() && !isStream) {
          try {
            const existing = await checkHostnameExists(domain.trim());
            if (!existing) {
              setHostsPromptDomain(domain.trim());
              setHostsPromptIp('127.0.0.1');
              setShowHostsPrompt(true);
              return; // Don't navigate yet — show dialog first
            }
          } catch { /* ignore check failure */ }
        }
      }

      navigate('/');
    } catch (e) {
      addToast('error', formatError(e));
    } finally {
      setSaving(false);
    }
  };

  const typeOptions: { value: FormProxyType; label: string }[] = [
    { value: 'http', label: t('proxyForm.typeHttp') },
    { value: 'https', label: t('proxyForm.typeHttps') },
    { value: 'tcp', label: t('proxyForm.typeTcp') },
    { value: 'udp', label: t('proxyForm.typeUdp') },
  ];

  const tlsOptions: { value: TlsMode; label: string }[] = [
    { value: 'none', label: t('proxyForm.tlsNone') },
    { value: 'terminate', label: t('proxyForm.tlsTerminate') },
    { value: 'passthrough', label: t('proxyForm.tlsPassthrough') },
  ];

  return (
    <div className="max-w-[640px]">
      <button
        onClick={() => navigate('/')}
        className="inline-flex items-center gap-1 text-[12px] text-text-secondary cursor-pointer mb-4 hover:text-text-primary"
      >
        <ArrowLeft className="w-3.5 h-3.5" />
        {t('proxyForm.backToList')}
      </button>

      {/* Basic Info */}
      <section className="mt-6">
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('proxyForm.basicInfo')}
        </h2>
        <div className="mb-4">
          <label className="block text-[12px] font-medium text-text-secondary mb-1">
            {t('proxyForm.ruleName')}
          </label>
          <Input
            value={name}
            onChange={(e) => { setName(e.target.value); clearError('name'); }}
            placeholder={t('proxyForm.ruleNamePlaceholder')}
            className={errors.name ? 'border-error' : ''}
          />
        </div>
        <div className="mb-4">
          <label className="block text-[12px] font-medium text-text-secondary mb-1">
            {t('proxyForm.proxyType')}
          </label>
          <div className="flex border border-border rounded-[var(--radius-sm)] overflow-hidden">
            {typeOptions.map((opt) => (
              <button
                key={opt.value}
                type="button"
                onClick={() => {
                  setFormType(opt.value);
                  if (!isEdit) {
                    const defaults: Record<FormProxyType, string> = { http: '80', https: '443', tcp: '', udp: '' };
                    setListenPort(defaults[opt.value]);
                  }
                }}
                className={cn(
                  'flex-1 py-[7px] px-3 text-center text-[12px] cursor-pointer border-r border-border last:border-r-0',
                  formType === opt.value
                    ? 'bg-accent-light text-accent font-medium'
                    : 'bg-bg-secondary text-text-secondary hover:bg-bg-hover',
                )}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>
      </section>

      {/* Listen Config */}
      <section className="mt-6">
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('proxyForm.listenConfig')}
        </h2>
        <div className="grid grid-cols-2 gap-3">
          {!isStream && (
            <div className="mb-4">
              <label className="block text-[12px] font-medium text-text-secondary mb-1">
                {t('proxyForm.domain')}
              </label>
              <Input
                value={domain}
                onChange={(e) => { setDomain(e.target.value); clearError('domain'); }}
                placeholder={t('proxyForm.domainPlaceholder')}
                className={errors.domain ? 'border-error' : ''}
              />
              <p className="text-[11px] text-text-tertiary mt-1">
                {t('proxyForm.domainHint')}
              </p>
            </div>
          )}
          <div className="mb-4">
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('proxyForm.listenPort')}
            </label>
            <Input
              type="number"
              value={listenPort}
              onChange={(e) => { setListenPort(e.target.value); clearError('listenPort'); setPortWarning(''); }}
              onBlur={checkPortConflicts}
              placeholder={t('proxyForm.listenPortPlaceholder')}
              className={errors.listenPort ? 'border-error' : ''}
            />
            {portWarning && (
              <p className="text-[11px] text-warning mt-1">
                {t('proxyForm.portConflictWarning', { message: portWarning })}
              </p>
            )}
          </div>
        </div>
        {!isStream && (
          <div className="mb-4">
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('proxyForm.pathPrefix')}
            </label>
            <Input
              value={pathPrefix}
              onChange={(e) => setPathPrefix(e.target.value)}
              placeholder={t('proxyForm.pathPrefixPlaceholder')}
            />
            <p className="text-[11px] text-text-tertiary mt-1">
              {t('proxyForm.pathPrefixHint')}
            </p>
          </div>
        )}
      </section>

      {/* Forward Target */}
      <section className="mt-6">
        <div className="flex items-center justify-between mb-3 pb-2 border-b border-border">
          <h2 className="text-[13px] font-semibold">
            {t('proxyForm.forwardTarget')}
          </h2>
          <div className="flex items-center gap-2">
            <label className="text-[11px] text-text-tertiary">
              {t('proxyForm.multiUpstream')}
            </label>
            <Toggle checked={multiUpstream} onChange={(v) => {
              setMultiUpstream(v);
              if (v) {
                // Migrate current single target into the list
                const host = upstreamHost.trim() || '127.0.0.1';
                const port = parseInt(upstreamPort) || 80;
                if (upstreamTargets.length === 0) {
                  setUpstreamTargets([{ host, port, weight: 1 }]);
                }
              } else if (upstreamTargets.length > 0) {
                // Write first target back to single fields
                setUpstreamHost(upstreamTargets[0].host);
                setUpstreamPort(upstreamTargets[0].port.toString());
              }
            }} />
          </div>
        </div>

        {!isStream && (
          <div className="mb-4">
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('proxyForm.upstreamScheme')}
            </label>
            <div className="flex border border-border rounded-[var(--radius-sm)] overflow-hidden w-fit">
              {(['http', 'https'] as const).map((s) => (
                <button
                  key={s}
                  type="button"
                  onClick={() => setUpstreamScheme(s)}
                  className={cn(
                    'py-[5px] px-4 text-center text-[12px] cursor-pointer border-r border-border last:border-r-0',
                    upstreamScheme === s
                      ? 'bg-accent-light text-accent font-medium'
                      : 'bg-bg-secondary text-text-secondary hover:bg-bg-hover',
                  )}
                >
                  {s.toUpperCase()}
                </button>
              ))}
            </div>
            <p className="text-[11px] text-text-tertiary mt-1">
              {t('proxyForm.upstreamSchemeHint')}
            </p>
          </div>
        )}

        {!multiUpstream ? (
          /* Single target mode */
          <div className="grid grid-cols-[1fr_120px] gap-3">
            <div className="mb-4">
              <label className="block text-[12px] font-medium text-text-secondary mb-1">
                {t('proxyForm.upstreamHost')}
              </label>
              <Input
                value={upstreamHost}
                onChange={(e) => { setUpstreamHost(e.target.value); clearError('upstreamHost'); }}
                placeholder={t('proxyForm.upstreamHostPlaceholder')}
                className={errors.upstreamHost ? 'border-error' : ''}
              />
            </div>
            <div className="mb-4">
              <label className="block text-[12px] font-medium text-text-secondary mb-1">
                {t('proxyForm.upstreamPort')}
              </label>
              <Input
                type="number"
                value={upstreamPort}
                onChange={(e) => { setUpstreamPort(e.target.value); clearError('upstreamPort'); }}
                placeholder={t('proxyForm.upstreamPortPlaceholder')}
                className={errors.upstreamPort ? 'border-error' : ''}
              />
            </div>
          </div>
        ) : (
          /* Multi-target load balancing mode */
          <div>
            <p className="text-[11px] text-text-tertiary mb-3">
              {t('proxyForm.multiUpstreamDesc')}
            </p>
            {/* Column headers */}
            <div className="flex gap-2 mb-1.5 px-0.5">
              <span className="flex-1 text-[11px] font-medium text-text-tertiary">{t('proxyForm.upstreamHost')}</span>
              <span className="w-[100px] text-[11px] font-medium text-text-tertiary">{t('proxyForm.upstreamPort')}</span>
              <span className="w-[80px] text-[11px] font-medium text-text-tertiary">{t('proxyForm.weight')}</span>
              <span className="w-7 shrink-0" />
            </div>
            {upstreamTargets.map((target, idx) => (
              <div key={idx} className="flex gap-2 mb-2 items-center">
                <Input
                  value={target.host}
                  onChange={(e) => {
                    const next = [...upstreamTargets];
                    next[idx] = { ...next[idx], host: e.target.value };
                    setUpstreamTargets(next);
                  }}
                  placeholder={t('proxyForm.upstreamHostPlaceholder')}
                  className="flex-1"
                />
                <Input
                  type="number"
                  value={target.port.toString()}
                  onChange={(e) => {
                    const next = [...upstreamTargets];
                    next[idx] = { ...next[idx], port: parseInt(e.target.value) || 0 };
                    setUpstreamTargets(next);
                  }}
                  placeholder={t('proxyForm.upstreamPortPlaceholder')}
                  className="w-[100px]"
                />
                <Input
                  type="number"
                  value={(target.weight ?? 1).toString()}
                  onChange={(e) => {
                    const next = [...upstreamTargets];
                    next[idx] = { ...next[idx], weight: parseInt(e.target.value) || 1 };
                    setUpstreamTargets(next);
                  }}
                  placeholder="1"
                  className="w-[80px]"
                />
                <button
                  type="button"
                  onClick={() => {
                    if (upstreamTargets.length > 1) {
                      setUpstreamTargets(upstreamTargets.filter((_, i) => i !== idx));
                    }
                  }}
                  disabled={upstreamTargets.length <= 1}
                  className={cn(
                    'w-7 h-7 flex items-center justify-center rounded shrink-0',
                    upstreamTargets.length > 1
                      ? 'text-text-secondary hover:bg-bg-sidebar hover:text-error cursor-pointer'
                      : 'text-text-tertiary/30 cursor-not-allowed',
                  )}
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
            ))}
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setUpstreamTargets([...upstreamTargets, { host: '127.0.0.1', port: 80, weight: 1 }])}
            >
              <Plus className="w-3.5 h-3.5" />
              {t('proxyForm.addTarget')}
            </Button>
          </div>
        )}
      </section>

      {/* TLS Config */}
      {showTls && (
        <section className="mt-6">
          <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
            {t('proxyForm.tlsConfig')}
          </h2>
          <div className="mb-4">
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('proxyForm.tlsMode')}
            </label>
            <div className="flex border border-border rounded-[var(--radius-sm)] overflow-hidden">
              {tlsOptions.map((opt) => (
                <button
                  key={opt.value}
                  type="button"
                  onClick={() => setTlsMode(opt.value)}
                  className={cn(
                    'flex-1 py-[7px] px-3 text-center text-[12px] cursor-pointer border-r border-border last:border-r-0',
                    tlsMode === opt.value
                      ? 'bg-accent-light text-accent font-medium'
                      : 'bg-bg-secondary text-text-secondary hover:bg-bg-hover',
                  )}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
          {tlsMode === 'terminate' && (
            <div className="mb-4">
              <label className="block text-[12px] font-medium text-text-secondary mb-1">
                {t('proxyForm.certificate')}
              </label>
              <Select
                value={certificateId}
                onChange={(e) => setCertificateId(e.target.value)}
              >
                <option value="">{t('proxyForm.certPlaceholder')}</option>
                {certificates.map((cert) => (
                  <option key={cert.id} value={cert.id}>
                    {cert.domain} ({cert.name})
                  </option>
                ))}
              </Select>
            </div>
          )}
        </section>
      )}

      {/* Advanced */}
      <section className="mt-6">
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('proxyForm.advancedOptions')}
        </h2>
        <div className="grid grid-cols-2 gap-3">
          <div className="mb-4">
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('proxyForm.accessList')}
            </label>
            <Select
              value={accessListId}
              onChange={(e) => setAccessListId(e.target.value)}
            >
              <option value="">{t('proxyForm.accessListNone')}</option>
              {accessLists.map((al) => (
                <option key={al.list.id} value={al.list.id}>
                  {al.list.name}
                </option>
              ))}
            </Select>
          </div>
          {showWebsocket && (
            <div className="mb-4 flex items-center gap-2 mt-6">
              <Toggle checked={websocket} onChange={setWebsocket} />
              <label className="text-[12px] font-medium text-text-secondary">
                {t('proxyForm.websocket')}
              </label>
            </div>
          )}
        </div>
      </section>

      {/* Custom Headers */}
      {!isStream && (
        <section className="mt-6">
          <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
            {t('proxyForm.customHeaders')}
          </h2>
          <p className="text-[11px] text-text-tertiary mb-3">
            {t('proxyForm.customHeadersDesc')}
          </p>
          {customHeaders.map((header, idx) => (
            <div key={idx} className="flex gap-2 mb-2 items-center">
              <Input
                value={header.key}
                onChange={(e) => {
                  const next = [...customHeaders];
                  next[idx] = { ...next[idx], key: e.target.value };
                  setCustomHeaders(next);
                }}
                placeholder="Header-Name"
                className="flex-1"
              />
              <Input
                value={header.value}
                onChange={(e) => {
                  const next = [...customHeaders];
                  next[idx] = { ...next[idx], value: e.target.value };
                  setCustomHeaders(next);
                }}
                placeholder="Value"
                className="flex-1"
              />
              <button
                type="button"
                onClick={() => setCustomHeaders(customHeaders.filter((_, i) => i !== idx))}
                className="w-7 h-7 flex items-center justify-center rounded text-text-secondary hover:bg-bg-sidebar hover:text-error shrink-0"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          ))}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setCustomHeaders([...customHeaders, { key: '', value: '' }])}
          >
            <Plus className="w-3.5 h-3.5" />
            {t('proxyForm.addHeader')}
          </Button>
        </section>
      )}

      {/* Footer */}
      <div className="flex gap-2 justify-end mt-8 pt-4 border-t border-border">
        <Button variant="default" onClick={() => navigate('/')}>
          {t('proxyForm.cancel')}
        </Button>
        <Button variant="primary" onClick={handleSave} disabled={saving}>
          {t('proxyForm.save')}
        </Button>
      </div>
      {/* Hosts entry prompt after proxy creation */}
      <Dialog
        open={showHostsPrompt}
        onClose={() => { setShowHostsPrompt(false); navigate('/'); }}
        title={t('hosts.promptTitle')}
        footer={
          <>
            <Button onClick={() => { setShowHostsPrompt(false); navigate('/'); }}>
              {t('hosts.promptSkip')}
            </Button>
            <Button
              variant="primary"
              onClick={async () => {
                try {
                  await createHost({ ip: hostsPromptIp, hostname: hostsPromptDomain });
                  addToast('success', t('hosts.createSuccess'));
                } catch (e) {
                  addToast('error', formatError(e));
                }
                setShowHostsPrompt(false);
                navigate('/');
              }}
            >
              {t('hosts.promptAdd')}
            </Button>
          </>
        }
      >
        <p className="text-[13px] text-text-secondary mb-3">
          {t('hosts.promptMessage', { domain: hostsPromptDomain })}
        </p>
        <div>
          <label className="block text-[12px] font-medium text-text-secondary mb-1">
            {t('hosts.ip')}
          </label>
          <Input
            value={hostsPromptIp}
            onChange={(e) => setHostsPromptIp(e.target.value)}
            placeholder="127.0.0.1"
          />
        </div>
      </Dialog>
    </div>
  );
}
