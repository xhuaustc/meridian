import { create } from 'zustand';
import * as api from '../lib/api';

type Theme = 'light' | 'dark' | 'system';

interface SettingsStore {
  theme: Theme;
  language: string;
  initialized: boolean;
  initialize: () => Promise<void>;
  setTheme: (theme: Theme) => Promise<void>;
  setLanguage: (lang: string) => Promise<void>;
  applyTheme: (theme: Theme) => void;
}

function applyThemeToDOM(theme: Theme) {
  const root = document.documentElement;
  if (theme === 'system') {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    root.classList.toggle('dark', prefersDark);
  } else {
    root.classList.toggle('dark', theme === 'dark');
  }
}

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  theme: 'light',
  language: 'zh',
  initialized: false,
  initialize: async () => {
    if (get().initialized) return;
    try {
      const [theme, language] = await Promise.all([
        api.getSetting('theme'),
        api.getSetting('language'),
      ]);
      const t = (theme as Theme) || 'light';
      const l = language || 'zh';
      set({ theme: t, language: l, initialized: true });
      applyThemeToDOM(t);
    } catch {
      set({ initialized: true });
      applyThemeToDOM('light');
    }
  },
  setTheme: async (theme) => {
    set({ theme });
    applyThemeToDOM(theme);
    try {
      await api.setSetting('theme', theme);
    } catch { /* ignore */ }
  },
  setLanguage: async (lang) => {
    set({ language: lang });
    try {
      await api.setSetting('language', lang);
      await api.syncTray();
    } catch { /* ignore */ }
  },
  applyTheme: applyThemeToDOM,
}));
