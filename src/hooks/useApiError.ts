import { useTranslation } from 'react-i18next';
import { parseApiError } from '../lib/api';

export function useApiError() {
  const { t } = useTranslation('common');

  return (e: unknown): string => {
    const { code, message } = parseApiError(e);
    const i18nKey = `errors.${code}`;
    const translated = t(i18nKey);
    // If translation exists (not same as key), use it; otherwise fall back to raw message
    return translated !== i18nKey ? translated : message;
  };
}
