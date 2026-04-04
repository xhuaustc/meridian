import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Upload, Lock, Trash2, Globe, Zap, CheckCircle, Loader2, AlertCircle, Download } from 'lucide-react';
import { ContentToolbar } from '../components/layout/ContentToolbar';
import { Button } from '../components/ui/Button';
import { Badge } from '../components/ui/Badge';
import { Input } from '../components/ui/Input';
import { Select } from '../components/ui/Select';
import { Dialog, ConfirmDialog } from '../components/ui/Dialog';
import { Toggle } from '../components/ui/Toggle';
import { save } from '@tauri-apps/plugin-dialog';
import { useCertStore } from '../stores/cert-store';
import { useDnsCredentialStore } from '../stores/dns-credential-store';
import { useProxyStore } from '../stores/proxy-store';
import { useToastStore } from '../stores/toast-store';
import { exportCertificate } from '../lib/api';
import { cn } from '../lib/utils';
import type { Certificate, DnsCredential } from '../types';

const DNS_PROVIDERS = [
  { value: 'cloudflare', label: 'Cloudflare' },
  { value: 'alidns', label: 'Alibaba Cloud DNS' },
  { value: 'dnspod', label: 'DNSPod (Tencent)' },
  { value: 'route53', label: 'AWS Route 53' },
];

const PROVIDER_FIELDS: Record<string, { key: string; label: string; labelZh: string }[]> = {
  cloudflare: [{ key: 'api_token', label: 'API Token', labelZh: 'API Token' }],
  alidns: [
    { key: 'access_key_id', label: 'AccessKey ID', labelZh: 'AccessKey ID' },
    { key: 'access_key_secret', label: 'AccessKey Secret', labelZh: 'AccessKey Secret' },
  ],
  dnspod: [
    { key: 'secret_id', label: 'SecretId', labelZh: 'SecretId' },
    { key: 'secret_key', label: 'SecretKey', labelZh: 'SecretKey' },
  ],
  route53: [
    { key: 'access_key_id', label: 'Access Key ID', labelZh: 'Access Key ID' },
    { key: 'secret_access_key', label: 'Secret Access Key', labelZh: 'Secret Access Key' },
  ],
};

function daysUntil(dateStr: string): number {
  const now = new Date();
  const target = new Date(dateStr);
  return Math.ceil((target.getTime() - now.getTime()) / (1000 * 60 * 60 * 24));
}

export function CertsPage() {
  const { t } = useTranslation('common');
  const { certificates, fetchCertificates, generateSelfSigned, importCertificate, requestAcmeCert, deleteCertificate } = useCertStore();
  const { credentials, fetchCredentials, createCredential, deleteCredential, testCredential } = useDnsCredentialStore();
  const { proxies } = useProxyStore();
  const addToast = useToastStore((s) => s.addToast);

  const [activeTab, setActiveTab] = useState<'certs' | 'dns'>('certs');

  // Cert dialogs
  const [showGenerate, setShowGenerate] = useState(false);
  const [showUpload, setShowUpload] = useState(false);
  const [showAcme, setShowAcme] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Certificate | null>(null);

  // Generate form
  const [genName, setGenName] = useState('');
  const [genDomain, setGenDomain] = useState('');
  const [genDays, setGenDays] = useState('365');

  // Upload form
  const [upName, setUpName] = useState('');
  const [upDomain, setUpDomain] = useState('');
  const [upCert, setUpCert] = useState('');
  const [upKey, setUpKey] = useState('');
  const [upExpires, setUpExpires] = useState('');

  // ACME form
  const [acmeDomains, setAcmeDomains] = useState('');
  const [acmeCredId, setAcmeCredId] = useState('');
  const [acmeEmail, setAcmeEmail] = useState('');
  const [acmeAutoRenew, setAcmeAutoRenew] = useState(true);

  // DNS credential dialogs
  const [showDnsForm, setShowDnsForm] = useState(false);
  const [dnsDeleteTarget, setDnsDeleteTarget] = useState<DnsCredential | null>(null);
  const [dnsName, setDnsName] = useState('');
  const [dnsProvider, setDnsProvider] = useState('cloudflare');
  const [dnsFields, setDnsFields] = useState<Record<string, string>>({});
  const [testingId, setTestingId] = useState<string | null>(null);

  useEffect(() => {
    fetchCertificates();
    fetchCredentials();
  }, [fetchCertificates, fetchCredentials]);

  // Poll for updates while any cert is still pending
  const hasPendingCerts = certificates.some((c) => c.status === 'pending');
  const pollRef = useRef<ReturnType<typeof setInterval> | undefined>(undefined);
  useEffect(() => {
    if (hasPendingCerts) {
      pollRef.current = setInterval(() => fetchCertificates(), 3000);
    }
    return () => clearInterval(pollRef.current);
  }, [hasPendingCerts, fetchCertificates]);

  const getBoundProxies = (certId: string) =>
    proxies.filter((p) => p.certificate_id === certId).map((p) => p.name);

  // --- Cert Handlers ---
  const handleGenerate = async () => {
    if (!genName || !genDomain) return;
    try {
      await generateSelfSigned(genName, genDomain, parseInt(genDays) || 365);
      addToast('success', t('certs.generateSuccess'));
      setShowGenerate(false);
      setGenName(''); setGenDomain(''); setGenDays('365');
    } catch (e) { addToast('error', String(e)); }
  };

  const handleUpload = async () => {
    if (!upName || !upDomain || !upCert || !upKey || !upExpires) return;
    try {
      await importCertificate(upName, upDomain, upCert, upKey, upExpires);
      addToast('success', t('certs.uploadSuccess'));
      setShowUpload(false);
      setUpName(''); setUpDomain(''); setUpCert(''); setUpKey(''); setUpExpires('');
    } catch (e) { addToast('error', String(e)); }
  };

  const handleAcmeRequest = async () => {
    const domains = acmeDomains.split('\n').map((d) => d.trim()).filter(Boolean);
    if (domains.length === 0 || !acmeCredId || !acmeEmail) return;
    try {
      await requestAcmeCert(domains, acmeCredId, acmeEmail, acmeAutoRenew);
      setShowAcme(false);
      setAcmeDomains(''); setAcmeCredId(''); setAcmeEmail(''); setAcmeAutoRenew(true);
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await deleteCertificate(deleteTarget.id);
      addToast('success', t('certs.deleteSuccess'));
    } catch (e) { addToast('error', String(e)); }
    setDeleteTarget(null);
  };

  const handleExport = async (cert: Certificate) => {
    const safeDomain = cert.domain.replace(/\*/g, '_wildcard');
    const defaultName = `${safeDomain}_${cert.name}.zip`;
    try {
      const path = await save({
        defaultPath: defaultName,
        filters: [{ name: 'ZIP', extensions: ['zip'] }],
      });
      if (!path) return;
      await exportCertificate(cert.id, path);
      addToast('success', t('certs.exportSuccess'));
    } catch (e) {
      addToast('error', String(e));
    }
  };

  // --- DNS Credential Handlers ---
  const handleDnsCreate = async () => {
    if (!dnsName.trim()) return;
    try {
      const credJson = JSON.stringify(dnsFields);
      await createCredential(dnsName, dnsProvider, credJson);
      addToast('success', t('dns.createSuccess'));
      setShowDnsForm(false);
      setDnsName(''); setDnsProvider('cloudflare'); setDnsFields({});
    } catch (e) { addToast('error', String(e)); }
  };

  const handleDnsDelete = async () => {
    if (!dnsDeleteTarget) return;
    try {
      await deleteCredential(dnsDeleteTarget.id);
      addToast('success', t('dns.deleteSuccess'));
    } catch (e) { addToast('error', String(e)); }
    setDnsDeleteTarget(null);
  };

  const handleDnsTest = async (id: string) => {
    setTestingId(id);
    try {
      const result = await testCredential(id);
      if (result.success) {
        addToast('success', result.message);
      } else {
        addToast('error', result.message);
      }
    } catch (e) { addToast('error', String(e)); }
    setTestingId(null);
  };

  const sourceVariant = (s: string) => {
    if (s === 'self_signed') return 'self_signed' as const;
    if (s === 'acme') return 'acme' as const;
    return 'upload' as const;
  };

  const sourceLabel = (s: string) => {
    if (s === 'self_signed') return t('certs.sourceSelfSigned');
    if (s === 'acme') return t('certs.sourceAcme');
    return t('certs.sourceUpload');
  };

  const providerLabel = (p: string) =>
    DNS_PROVIDERS.find((dp) => dp.value === p)?.label || p;

  return (
    <>
      <ContentToolbar title={t('certs.title')}>
        <button
          onClick={() => setActiveTab('certs')}
          className={cn(
            'px-2.5 py-[5px] border rounded-[20px] text-[11.5px] cursor-pointer',
            activeTab === 'certs'
              ? 'bg-accent-light text-accent border-[#bfdbfe] dark:border-accent/40'
              : 'bg-bg-secondary text-text-secondary border-border hover:bg-bg-hover',
          )}
        >
          {t('certs.title')}
        </button>
        <button
          onClick={() => setActiveTab('dns')}
          className={cn(
            'px-2.5 py-[5px] border rounded-[20px] text-[11.5px] cursor-pointer',
            activeTab === 'dns'
              ? 'bg-accent-light text-accent border-[#bfdbfe] dark:border-accent/40'
              : 'bg-bg-secondary text-text-secondary border-border hover:bg-bg-hover',
          )}
        >
          {t('dns.title')}
        </button>
      </ContentToolbar>
      <div className="p-6 overflow-y-auto flex-1">
      {activeTab === 'certs' ? (
        <>
          {/* Cert tab header */}
          <div className="flex items-center justify-between mb-4">
            <div />
            <div className="flex gap-2">
              <Button onClick={() => setShowGenerate(true)}>
                {t('certs.generateSelfSigned')}
              </Button>
              <Button onClick={() => setShowUpload(true)}>
                <Upload className="w-3.5 h-3.5" />
                {t('certs.uploadCert')}
              </Button>
              <Button variant="primary" onClick={() => setShowAcme(true)}>
                <Zap className="w-3.5 h-3.5" />
                {t('certs.requestAcme')}
              </Button>
            </div>
          </div>

          {certificates.length === 0 ? (
            <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] py-16 flex flex-col items-center justify-center">
              <Lock className="w-10 h-10 text-text-tertiary mb-3" />
              <p className="text-[13px] font-medium text-text-secondary">{t('certs.emptyTitle')}</p>
              <p className="text-[12px] text-text-tertiary mt-1">{t('certs.emptyDesc')}</p>
            </div>
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-3">
              {certificates.map((cert) => {
                const isPending = cert.status === 'pending';
                const isFailed = cert.status === 'failed';
                const days = daysUntil(cert.expires_at);
                const isExpiring = !isPending && days <= 30 && days > 0;
                const isExpired = !isPending && days <= 0;
                const bound = getBoundProxies(cert.id);
                const acmeDomainList: string[] = cert.acme_domains
                  ? JSON.parse(cert.acme_domains)
                  : [];
                return (
                  <div key={cert.id} className={cn(
                    'bg-bg-secondary border rounded-[var(--radius-md)] p-4',
                    isPending ? 'border-accent/40' : isFailed ? 'border-error/40' : 'border-border',
                  )}>
                    <div className="flex items-start justify-between mb-3">
                      <div className="min-w-0 flex-1">
                        <div className="font-medium text-[13px] truncate">{cert.domain}</div>
                        <div className="text-[11px] text-text-tertiary mt-0.5 truncate">{cert.name}</div>
                      </div>
                      <div className="flex items-center gap-2 shrink-0 ml-2">
                        {isPending ? (
                          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium bg-accent-light text-accent">
                            <Loader2 className="w-3 h-3 animate-spin" />
                            {t('certs.statusPending')}
                          </span>
                        ) : isFailed ? (
                          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium bg-error-bg text-error">
                            <AlertCircle className="w-3 h-3" />
                            {t('certs.statusFailed')}
                          </span>
                        ) : (
                          <Badge variant={sourceVariant(cert.source)}>{sourceLabel(cert.source)}</Badge>
                        )}
                        <button
                          onClick={() => handleExport(cert)}
                          disabled={cert.status !== 'ready'}
                          title={t('certs.export')}
                          className={cn(
                            'p-1 rounded',
                            cert.status === 'ready'
                              ? 'text-text-tertiary hover:bg-bg-hover hover:text-accent'
                              : 'text-text-tertiary/30 cursor-not-allowed',
                          )}
                        >
                          <Download className="w-3.5 h-3.5" />
                        </button>
                        <button onClick={() => {
                          const b = getBoundProxies(cert.id);
                          if (b.length > 0) {
                            addToast('error', t('certs.deleteBoundError', { name: cert.name, proxies: b.join(', ') }));
                          } else {
                            setDeleteTarget(cert);
                          }
                        }} className="p-1 rounded hover:bg-bg-hover text-text-tertiary hover:text-error">
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>
                    <div className="text-[11.5px] text-text-secondary flex flex-col gap-1">
                      {acmeDomainList.length > 1 && (
                        <span className="text-text-tertiary">
                          {t('certs.domains')}: {acmeDomainList.join(', ')}
                        </span>
                      )}
                      {isPending ? (
                        <span className="text-text-tertiary">{t('certs.acmeRequesting')}</span>
                      ) : isFailed ? (
                        <span className="text-error text-[11px] leading-relaxed break-all">
                          {cert.last_renew_error || t('certs.statusFailed')}
                        </span>
                      ) : (
                        <>
                          <span>{t('certs.created')}: {cert.created_at.split('T')[0]}</span>
                          <span className={cn(
                            isExpired ? 'text-error font-medium' : isExpiring ? 'text-warning font-medium' : '',
                          )}>
                            {t('certs.expires')}: {cert.expires_at.split('T')[0]}
                            {cert.auto_renew && ` · ${t('certs.autoRenew')}`}
                            {isExpiring && ` · ${t('certs.expiringDays', { days })}`}
                            {isExpired && ` · ${t('certs.expired')}`}
                          </span>
                          {cert.last_renew_error && (
                            <span className="text-error text-[11px]">
                              {t('certs.renewError')}: {cert.last_renew_error}
                            </span>
                          )}
                          <span>
                            {t('certs.bound')}: {bound.length > 0 ? bound.join(', ') : t('certs.notBound')}
                          </span>
                        </>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </>
      ) : (
        <>
          {/* DNS Provider tab */}
          <div className="flex items-center justify-between mb-4">
            <div />
            <Button variant="primary" onClick={() => { setShowDnsForm(true); setDnsName(''); setDnsProvider('cloudflare'); setDnsFields({}); }}>
              <Globe className="w-3.5 h-3.5" />
              {t('dns.addProvider')}
            </Button>
          </div>

          {credentials.length === 0 ? (
            <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] py-16 flex flex-col items-center justify-center">
              <Globe className="w-10 h-10 text-text-tertiary mb-3" />
              <p className="text-[13px] font-medium text-text-secondary">{t('dns.emptyTitle')}</p>
              <p className="text-[12px] text-text-tertiary mt-1">{t('dns.emptyDesc')}</p>
            </div>
          ) : (
            <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] overflow-hidden">
              <table className="w-full text-[12.5px]">
                <thead>
                  <tr className="border-b border-border text-text-tertiary text-left">
                    <th className="px-4 py-2.5 font-medium">{t('dns.colName')}</th>
                    <th className="px-4 py-2.5 font-medium">{t('dns.colProvider')}</th>
                    <th className="px-4 py-2.5 font-medium">{t('dns.colCreated')}</th>
                    <th className="px-4 py-2.5 font-medium text-right">{t('dns.colActions')}</th>
                  </tr>
                </thead>
                <tbody>
                  {credentials.map((cred) => (
                    <tr key={cred.id} className="border-b border-border last:border-b-0 hover:bg-bg-hover transition-colors">
                      <td className="px-4 py-3 font-medium text-text-primary">{cred.name}</td>
                      <td className="px-4 py-3">
                        <Badge variant="http">{providerLabel(cred.provider)}</Badge>
                      </td>
                      <td className="px-4 py-3 text-text-secondary">{cred.created_at.split('T')[0]}</td>
                      <td className="px-4 py-3 text-right">
                        <div className="flex items-center justify-end gap-2">
                          <Button
                            size="sm"
                            onClick={() => handleDnsTest(cred.id)}
                            disabled={testingId === cred.id}
                          >
                            {testingId === cred.id ? (
                              <Loader2 className="w-3 h-3 animate-spin" />
                            ) : (
                              <CheckCircle className="w-3 h-3" />
                            )}
                            {t('dns.test')}
                          </Button>
                          <Button
                            size="sm"
                            variant="danger"
                            onClick={() => setDnsDeleteTarget(cred)}
                          >
                            <Trash2 className="w-3 h-3" />
                          </Button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}

      {/* Generate Self-Signed Dialog */}
      <Dialog
        open={showGenerate}
        onClose={() => setShowGenerate(false)}
        title={t('certs.generateTitle')}
        footer={<>
          <Button onClick={() => setShowGenerate(false)}>{t('common.cancel')}</Button>
          <Button variant="primary" onClick={handleGenerate}>{t('certs.generate')}</Button>
        </>}
      >
        <div className="flex flex-col gap-3">
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('certs.generateName')}</label>
            <Input value={genName} onChange={(e) => setGenName(e.target.value)} placeholder={t('certs.generateNamePlaceholder')} />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('certs.generateDomain')}</label>
            <Input value={genDomain} onChange={(e) => setGenDomain(e.target.value)} placeholder={t('certs.generateDomainPlaceholder')} />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('certs.generateDays')}</label>
            <Input type="number" value={genDays} onChange={(e) => setGenDays(e.target.value)} placeholder={t('certs.generateDaysPlaceholder')} />
          </div>
        </div>
      </Dialog>

      {/* Upload Dialog */}
      <Dialog
        open={showUpload}
        onClose={() => setShowUpload(false)}
        title={t('certs.uploadTitle')}
        footer={<>
          <Button onClick={() => setShowUpload(false)}>{t('common.cancel')}</Button>
          <Button variant="primary" onClick={handleUpload}>{t('certs.upload')}</Button>
        </>}
      >
        <div className="flex flex-col gap-3">
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('certs.uploadName')}</label>
            <Input value={upName} onChange={(e) => setUpName(e.target.value)} placeholder={t('certs.generateNamePlaceholder')} />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('certs.uploadDomain')}</label>
            <Input value={upDomain} onChange={(e) => setUpDomain(e.target.value)} placeholder={t('certs.generateDomainPlaceholder')} />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('certs.uploadCertPem')}</label>
            <textarea className="w-full px-2.5 py-2 border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-secondary text-text-primary outline-none font-mono h-20 resize-y focus:border-accent" value={upCert} onChange={(e) => setUpCert(e.target.value)} />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('certs.uploadKeyPem')}</label>
            <textarea className="w-full px-2.5 py-2 border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-secondary text-text-primary outline-none font-mono h-20 resize-y focus:border-accent" value={upKey} onChange={(e) => setUpKey(e.target.value)} />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('certs.uploadExpires')}</label>
            <Input type="date" value={upExpires} onChange={(e) => setUpExpires(e.target.value)} />
          </div>
        </div>
      </Dialog>

      {/* ACME Request Dialog */}
      <Dialog
        open={showAcme}
        onClose={() => setShowAcme(false)}
        title={t('certs.acmeTitle')}
        footer={<>
          <Button onClick={() => setShowAcme(false)}>{t('common.cancel')}</Button>
          <Button variant="primary" onClick={handleAcmeRequest}>
            <Zap className="w-3.5 h-3.5" />
            {t('certs.acmeRequest')}
          </Button>
        </>}
      >
        <div className="flex flex-col gap-3">
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('certs.acmeDomains')}</label>
            <textarea
              className="w-full px-2.5 py-2 border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-secondary text-text-primary outline-none font-mono h-20 resize-y focus:border-accent"
              value={acmeDomains}
              onChange={(e) => setAcmeDomains(e.target.value)}
              placeholder={t('certs.acmeDomainsPlaceholder')}
            />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('certs.acmeProvider')}</label>
            <Select value={acmeCredId} onChange={(e) => setAcmeCredId(e.target.value)}>
              <option value="">{t('certs.acmeProviderPlaceholder')}</option>
              {credentials.map((c) => (
                <option key={c.id} value={c.id}>{c.name} ({providerLabel(c.provider)})</option>
              ))}
            </Select>
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('certs.acmeEmail')}</label>
            <Input
              value={acmeEmail}
              onChange={(e) => setAcmeEmail(e.target.value)}
              placeholder={t('certs.acmeEmailPlaceholder')}
            />
          </div>
          <div className="flex items-center gap-2">
            <Toggle checked={acmeAutoRenew} onChange={setAcmeAutoRenew} />
            <span className="text-[12px] text-text-secondary">{t('certs.autoRenew')}</span>
          </div>
        </div>
      </Dialog>

      {/* DNS Credential Form Dialog */}
      <Dialog
        open={showDnsForm}
        onClose={() => setShowDnsForm(false)}
        title={t('dns.addTitle')}
        footer={<>
          <Button onClick={() => setShowDnsForm(false)}>{t('common.cancel')}</Button>
          <Button variant="primary" onClick={handleDnsCreate}>{t('common.save')}</Button>
        </>}
      >
        <div className="flex flex-col gap-3">
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('dns.name')}</label>
            <Input value={dnsName} onChange={(e) => setDnsName(e.target.value)} placeholder={t('dns.namePlaceholder')} />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">{t('dns.provider')}</label>
            <Select value={dnsProvider} onChange={(e) => { setDnsProvider(e.target.value); setDnsFields({}); }}>
              {DNS_PROVIDERS.map((p) => (
                <option key={p.value} value={p.value}>{p.label}</option>
              ))}
            </Select>
          </div>
          {PROVIDER_FIELDS[dnsProvider]?.map((field) => (
            <div key={field.key}>
              <label className="block text-[12px] font-medium text-text-secondary mb-1">{field.label}</label>
              <Input
                type="password"
                value={dnsFields[field.key] || ''}
                onChange={(e) => setDnsFields({ ...dnsFields, [field.key]: e.target.value })}
              />
            </div>
          ))}
        </div>
      </Dialog>

      {/* Delete Cert Confirm */}
      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleDelete}
        title={t('common.delete')}
        message={t('certs.deleteConfirm', { name: deleteTarget?.name })}
        confirmText={t('common.delete')}
        danger
      />

      {/* Delete DNS Credential Confirm */}
      <ConfirmDialog
        open={!!dnsDeleteTarget}
        onClose={() => setDnsDeleteTarget(null)}
        onConfirm={handleDnsDelete}
        title={t('common.delete')}
        message={t('dns.deleteConfirm', { name: dnsDeleteTarget?.name })}
        confirmText={t('common.delete')}
        danger
      />
      </div>
    </>
  );
}
