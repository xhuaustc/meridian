import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { ProxyForm } from '../components/proxy/ProxyForm';
import { getProxy } from '../lib/api';
import type { ProxyRule } from '../types';

export function ProxyFormPage() {
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

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-text-tertiary text-[13px]">
        Loading...
      </div>
    );
  }

  return <ProxyForm key={id} rule={rule} />;
}
