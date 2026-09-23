import { create } from 'zustand';
import type { HostEntry, HostsSyncStatus } from '../types';
import * as api from '../lib/api';

interface HostsStore {
  entries: HostEntry[];
  loading: boolean;
  error: string | null;
  syncStatus: HostsSyncStatus | null;
  refreshSyncStatus: () => Promise<void>;
  fetchEntries: (keyword?: string) => Promise<void>;
  createEntry: (ip: string, hostname: string, comment?: string) => Promise<HostEntry>;
  updateEntry: (id: string, ip?: string, hostname?: string, comment?: string) => Promise<HostEntry>;
  deleteEntry: (id: string) => Promise<void>;
  toggleEntry: (id: string, enabled: boolean) => Promise<HostEntry>;
  syncToSystem: () => Promise<void>;
}

export const useHostsStore = create<HostsStore>((set, get) => ({
  entries: [],
  loading: false,
  error: null,
  syncStatus: null,
  refreshSyncStatus: async () => {
    try {
      set({ syncStatus: await api.getHostsSyncStatus() });
    } catch (e) {
      set({ syncStatus: { synced: null, checked_at: null, error: String(e) } });
    }
  },
  fetchEntries: async (keyword?: string) => {
    set({ loading: true, error: null });
    try {
      const entries = await api.listHosts(keyword);
      set({ entries, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  createEntry: async (ip, hostname, comment) => {
    const entry = await api.createHost({ ip, hostname, comment });
    await get().fetchEntries();
    await get().refreshSyncStatus();
    return entry;
  },
  updateEntry: async (id, ip, hostname, comment) => {
    const entry = await api.updateHost(id, ip, hostname, comment);
    set((state) => ({
      entries: state.entries.map((e) => (e.id === id ? entry : e)),
    }));
    await get().refreshSyncStatus();
    return entry;
  },
  deleteEntry: async (id) => {
    await api.deleteHost(id);
    set((state) => ({
      entries: state.entries.filter((e) => e.id !== id),
    }));
    await get().refreshSyncStatus();
  },
  toggleEntry: async (id, enabled) => {
    const entry = await api.toggleHost(id, enabled);
    set((state) => ({
      entries: state.entries.map((e) => (e.id === id ? entry : e)),
    }));
    await get().refreshSyncStatus();
    return entry;
  },
  syncToSystem: async () => {
    try {
      await api.syncHostsFile();
    } finally {
      await get().refreshSyncStatus();
    }
  },
}));
