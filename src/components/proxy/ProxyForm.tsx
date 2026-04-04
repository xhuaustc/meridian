import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { Button } from '../ui/Button';
import { Input } from '../ui/Input';
import { Select } from '../ui/Select';
import { Toggle } from '../ui/Toggle';
import { useProxyStore } from '../../stores/proxy-store';
import { useCertStore } from '../../stores/cert-store';
import { useAccessStore } from '../../stores/access-store';
import { useToastStore } from '../../stores/toast-store';
import { checkPortConflict } from '../../lib/api';
import { cn } from '../../lib/utils';
import type { ProxyRule, ProxyType, TlsMode, CreateProxyRule, UpdateProxyRule } from '../../types';

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
  const [tlsMode, setTlsMode] = useState<TlsMode>(rule?.tls_mode ?? 'none');
  const [certificateId, setCertificateId] = useState(rule?.certificate_id ?? '');
  const [accessListId, setAccessListId] = useState(rule?.access_list_id ?? '');
  const [websocket, setWebsocket] = useState(rule?.websocket ?? false);
  const [portWarning, setPortWarning] = useState('');
  const [saving, setSaving] = useState(false);
  const [errors, setErrors] = useState<Record<string, boolean>>({});

  useEffect(() => {
    fetchCertificates();
    fetchLists();
  }, [fetchCertificates, fetchLists]);

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
    if (!upstreamHost.trim()) errs.upstreamHost = true;
    if (!upstreamPort) errs.upstreamPort = true;
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
          tls_mode: tlsMode,
          certificate_id: tlsMode === 'terminate' ? certificateId || null : null,
          access_list_id: accessListId || null,
          websocket: showWebsocket ? websocket : false,
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
          tls_mode: tlsMode,
          certificate_id: tlsMode === 'terminate' ? certificateId || null : null,
          access_list_id: accessListId || null,
          websocket: showWebsocket ? websocket : false,
        };
        await createProxy(input);
        addToast('success', t('proxyForm.createSuccess'));
      }

      navigate('/');
    } catch (e) {
      addToast('error', String(e));
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

      <div className="flex items-center justify-between mb-5">
        <h1 className="text-[18px] font-semibold tracking-[-0.02em]">
          {isEdit ? t('proxyForm.editTitle') : t('proxyForm.createTitle')}
        </h1>
      </div>

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
        <h2 className="text-[13px] font-semibold mb-3 pb-2 border-b border-border">
          {t('proxyForm.forwardTarget')}
        </h2>
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

      {/* Footer */}
      <div className="flex gap-2 justify-end mt-8 pt-4 border-t border-border">
        <Button variant="default" onClick={() => navigate('/')}>
          {t('proxyForm.cancel')}
        </Button>
        <Button variant="primary" onClick={handleSave} disabled={saving}>
          {t('proxyForm.save')}
        </Button>
      </div>
    </div>
  );
}
