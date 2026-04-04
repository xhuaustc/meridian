import { create } from 'zustand';
import type { DnsCredential, TestResult } from '../types';
import * as api from '../lib/api';

interface DnsCredentialStore {
  credentials: DnsCredential[];
  loading: boolean;
  error: string | null;
  fetchCredentials: () => Promise<void>;
  createCredential: (name: string, provider: string, credentialsJson: string) => Promise<DnsCredential>;
  updateCredential: (id: string, name?: string, credentialsJson?: string) => Promise<DnsCredential>;
  deleteCredential: (id: string) => Promise<void>;
  testCredential: (id: string) => Promise<TestResult>;
}

export const useDnsCredentialStore = create<DnsCredentialStore>((set) => ({
  credentials: [],
  loading: false,
  error: null,
  fetchCredentials: async () => {
    set({ loading: true, error: null });
    try {
      const credentials = await api.listDnsCredentials();
      set({ credentials, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  createCredential: async (name, provider, credentialsJson) => {
    const cred = await api.createDnsCredential(name, provider, credentialsJson);
    set((state) => ({ credentials: [...state.credentials, cred] }));
    return cred;
  },
  updateCredential: async (id, name, credentialsJson) => {
    const cred = await api.updateDnsCredential(id, name, credentialsJson);
    set((state) => ({
      credentials: state.credentials.map((c) => (c.id === id ? cred : c)),
    }));
    return cred;
  },
  deleteCredential: async (id) => {
    await api.deleteDnsCredential(id);
    set((state) => ({ credentials: state.credentials.filter((c) => c.id !== id) }));
  },
  testCredential: async (id) => {
    return api.testDnsCredential(id);
  },
}));
