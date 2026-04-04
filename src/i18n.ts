import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import zh from './locales/zh/common.json';
import en from './locales/en/common.json';

function detectLanguage(): string {
  const lang = navigator.language || '';
  return lang.startsWith('zh') ? 'zh' : 'en';
}

i18n.use(initReactI18next).init({
  resources: {
    zh: { common: zh },
    en: { common: en },
  },
  lng: detectLanguage(),
  fallbackLng: 'en',
  ns: ['common'],
  defaultNS: 'common',
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
