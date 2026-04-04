import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { Button } from '../components/ui/Button';

export function NotFoundPage() {
  const { t } = useTranslation('common');
  const navigate = useNavigate();

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-6">
      <div className="text-[64px] font-bold text-text-tertiary mb-2">404</div>
      <p className="text-[13px] text-text-secondary mb-6">{t('notFound.message')}</p>
      <Button variant="primary" onClick={() => navigate('/')}>
        {t('notFound.backHome')}
      </Button>
    </div>
  );
}
