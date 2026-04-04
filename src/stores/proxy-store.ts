import { create } from 'zustand';
import type { ProxyRule, CreateProxyRule, UpdateProxyRule } from '../types';
import * as api from '../lib/api';

interface ProxyStore {
  proxies: ProxyRule[];
  stats: Record<string, number>;
  loading: boolean;
  error: string | null;
  fetchProxies: (filter?: {
    proxyType?: string;
    enabled?: boolean;
    search?: string;
  }) => Promise<void>;
  createProxy: (input: CreateProxyRule) => Promise<ProxyRule>;
  updateProxy: (id: string, input: UpdateProxyRule) => Promise<ProxyRule>;
  deleteProxy: (id: string) => Promise<void>;
  toggleProxy: (id: string, enabled: boolean) => Promise<ProxyRule>;
}

export const useProxyStore = create<ProxyStore>((set) => ({
  proxies: [],
  stats: {},
  loading: false,
  error: null,
  fetchProxies: async (filter) => {
    set({ loading: true, error: null });
    try {
      const response = await api.listProxies(filter);
      set({ proxies: response.rules, stats: response.stats, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  createProxy: async (input) => {
    const proxy = await api.createProxy(input);
    set((state) => ({ proxies: [...state.proxies, proxy] }));
    return proxy;
  },
  updateProxy: async (id, input) => {
    const proxy = await api.updateProxy(id, input);
    set((state) => ({
      proxies: state.proxies.map((p) => (p.id === id ? proxy : p)),
    }));
    return proxy;
  },
  deleteProxy: async (id) => {
    await api.deleteProxy(id);
    set((state) => ({ proxies: state.proxies.filter((p) => p.id !== id) }));
  },
  toggleProxy: async (id, enabled) => {
    const proxy = await api.toggleProxy(id, enabled);
    set((state) => ({
      proxies: state.proxies.map((p) => (p.id === id ? proxy : p)),
    }));
    return proxy;
  },
}));
