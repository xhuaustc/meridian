import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Upload, Lock, Trash2 } from 'lucide-react';
import { Button } from '../components/ui/Button';
import { Badge } from '../components/ui/Badge';
import { Input } from '../components/ui/Input';
import { Dialog, ConfirmDialog } from '../components/ui/Dialog';
import { useCertStore } from '../stores/cert-store';
import { useProxyStore } from '../stores/proxy-store';
import { useToastStore } from '../stores/toast-store';
import { cn } from '../lib/utils';
import type { Certificate } from '../types';

function daysUntil(dateStr: string): number {
  const now = new Date();
  const target = new Date(dateStr);
  return Math.ceil((target.getTime() - now.getTime()) / (1000 * 60 * 60 * 24));
}

export function CertsPage() {
  const { t } = useTranslation('common');
  const { certificates, fetchCertificates, generateSelfSigned, importCertificate, deleteCertificate } = useCertStore();
  const { proxies } = useProxyStore();
  const addToast = useToastStore((s) => s.addToast);

  const [showGenerate, setShowGenerate] = useState(false);
  const [showUpload, setShowUpload] = useState(false);
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

  useEffect(() => {
    fetchCertificates();
  }, [fetchCertificates]);

  const getBoundProxies = (certId: string) =>
    proxies.filter((p) => p.certificate_id === certId).map((p) => p.name);

  const handleGenerate = async () => {
    if (!genName || !genDomain) return;
    try {
      await generateSelfSigned(genName, genDomain, parseInt(genDays) || 365);
      addToast('success', t('certs.generateSuccess'));
      setShowGenerate(false);
      setGenName('');
      setGenDomain('');
      setGenDays('365');
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleUpload = async () => {
    if (!upName || !upDomain || !upCert || !upKey || !upExpires) return;
    try {
      await importCertificate(upName, upDomain, upCert, upKey, upExpires);
      addToast('success', t('certs.uploadSuccess'));
      setShowUpload(false);
      setUpName('');
      setUpDomain('');
      setUpCert('');
      setUpKey('');
      setUpExpires('');
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await deleteCertificate(deleteTarget.id);
      addToast('success', t('certs.deleteSuccess'));
    } catch (e) {
      addToast('error', String(e));
    }
    setDeleteTarget(null);
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

  return (
    <div>
      <div className="flex items-center justify-between mb-5">
        <h1 className="text-[18px] font-semibold tracking-[-0.02em]">
          {t('certs.title')}
        </h1>
        <div className="flex gap-2">
          <Button onClick={() => setShowGenerate(true)}>
            {t('certs.generateSelfSigned')}
          </Button>
          <Button variant="primary" onClick={() => setShowUpload(true)}>
            <Upload className="w-3.5 h-3.5" />
            {t('certs.uploadCert')}
          </Button>
        </div>
      </div>

      {certificates.length === 0 ? (
        <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] py-16 flex flex-col items-center justify-center">
          <Lock className="w-10 h-10 text-text-tertiary mb-3" />
          <p className="text-[13px] font-medium text-text-secondary">
            {t('certs.emptyTitle')}
          </p>
          <p className="text-[12px] text-text-tertiary mt-1">
            {t('certs.emptyDesc')}
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-3">
          {certificates.map((cert) => {
            const days = daysUntil(cert.expires_at);
            const isExpiring = days <= 30 && days > 0;
            const isExpired = days <= 0;
            const bound = getBoundProxies(cert.id);
            return (
              <div
                key={cert.id}
                className="bg-bg-secondary border border-border rounded-[var(--radius-md)] p-4"
              >
                <div className="flex items-start justify-between mb-3">
                  <div>
                    <div className="font-medium text-[13px]">{cert.domain}</div>
                    <div className="text-[11px] text-text-tertiary mt-0.5">{cert.name}</div>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge variant={sourceVariant(cert.source)}>
                      {sourceLabel(cert.source)}
                    </Badge>
                    <button
                      onClick={() => setDeleteTarget(cert)}
                      className="p-1 rounded hover:bg-bg-hover text-text-tertiary hover:text-error"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>
                <div className="text-[11.5px] text-text-secondary flex flex-col gap-1">
                  <span>
                    {t('certs.created')}: {cert.created_at.split('T')[0]}
                  </span>
                  <span
                    className={cn(
                      isExpired
                        ? 'text-error font-medium'
                        : isExpiring
                          ? 'text-warning font-medium'
                          : '',
                    )}
                  >
                    {t('certs.expires')}: {cert.expires_at.split('T')[0]}
                    {cert.auto_renew && ` · ${t('certs.autoRenew')}`}
                    {isExpiring &&
                      ` · ${t('certs.expiringDays', { days })}`}
                    {isExpired && ` · ${t('certs.expired')}`}
                  </span>
                  <span>
                    {t('certs.bound')}:{' '}
                    {bound.length > 0 ? bound.join(', ') : t('certs.notBound')}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Generate Dialog */}
      <Dialog
        open={showGenerate}
        onClose={() => setShowGenerate(false)}
        title={t('certs.generateTitle')}
        footer={
          <>
            <Button onClick={() => setShowGenerate(false)}>
              {t('common.cancel')}
            </Button>
            <Button variant="primary" onClick={handleGenerate}>
              {t('certs.generate')}
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-3">
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('certs.generateName')}
            </label>
            <Input
              value={genName}
              onChange={(e) => setGenName(e.target.value)}
              placeholder={t('certs.generateNamePlaceholder')}
            />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('certs.generateDomain')}
            </label>
            <Input
              value={genDomain}
              onChange={(e) => setGenDomain(e.target.value)}
              placeholder={t('certs.generateDomainPlaceholder')}
            />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('certs.generateDays')}
            </label>
            <Input
              type="number"
              value={genDays}
              onChange={(e) => setGenDays(e.target.value)}
              placeholder={t('certs.generateDaysPlaceholder')}
            />
          </div>
        </div>
      </Dialog>

      {/* Upload Dialog */}
      <Dialog
        open={showUpload}
        onClose={() => setShowUpload(false)}
        title={t('certs.uploadTitle')}
        footer={
          <>
            <Button onClick={() => setShowUpload(false)}>
              {t('common.cancel')}
            </Button>
            <Button variant="primary" onClick={handleUpload}>
              {t('certs.upload')}
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-3">
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('certs.uploadName')}
            </label>
            <Input
              value={upName}
              onChange={(e) => setUpName(e.target.value)}
              placeholder={t('certs.generateNamePlaceholder')}
            />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('certs.uploadDomain')}
            </label>
            <Input
              value={upDomain}
              onChange={(e) => setUpDomain(e.target.value)}
              placeholder={t('certs.generateDomainPlaceholder')}
            />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('certs.uploadCertPem')}
            </label>
            <textarea
              className="w-full px-2.5 py-2 border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-secondary text-text-primary outline-none font-mono h-20 resize-y focus:border-accent"
              value={upCert}
              onChange={(e) => setUpCert(e.target.value)}
            />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('certs.uploadKeyPem')}
            </label>
            <textarea
              className="w-full px-2.5 py-2 border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-secondary text-text-primary outline-none font-mono h-20 resize-y focus:border-accent"
              value={upKey}
              onChange={(e) => setUpKey(e.target.value)}
            />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('certs.uploadExpires')}
            </label>
            <Input
              type="date"
              value={upExpires}
              onChange={(e) => setUpExpires(e.target.value)}
            />
          </div>
        </div>
      </Dialog>

      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleDelete}
        title={t('common.delete')}
        message={t('certs.deleteConfirm', { name: deleteTarget?.name })}
        confirmText={t('common.delete')}
        danger
      />
    </div>
  );
}
