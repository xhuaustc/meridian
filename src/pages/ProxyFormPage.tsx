import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { ContentToolbar } from '../components/layout/ContentToolbar';
import { ProxyForm } from '../components/proxy/ProxyForm';
import { getProxy } from '../lib/api';
import type { ProxyRule } from '../types';

export function ProxyFormPage() {
  const { t } = useTranslation('common');
  const { id } = useParams<{ id: string }>();
  const isNew = !id || id === 'new';
  const [rule, setRule] = useState<ProxyRule | undefined>(undefined);
  const [loading, setLoading] = useState(!isNew);

  useEffect(() => {
    if (!isNew && id) {
      setLoading(true);
      getProxy(id)
        .then(setRule)
        .catch(() => {})
        .finally(() => setLoading(false));
    }
  }, [id, isNew]);

  const title = isNew
    ? t('proxyForm.createTitle')
    : t('proxyForm.editTitle');

  if (loading) {
    return (
      <>
        <ContentToolbar title={title} />
        <div className="p-6 overflow-y-auto flex-1">
          <div className="flex items-center justify-center h-full text-text-tertiary text-[13px]">
            Loading...
          </div>
        </div>
      </>
    );
  }

  return (
    <>
      <ContentToolbar title={title} />
      <div className="p-6 overflow-y-auto flex-1">
        <ProxyForm key={id} rule={rule} />
      </div>
    </>
  );
}
