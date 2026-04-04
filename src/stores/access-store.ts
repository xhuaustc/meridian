import { create } from 'zustand';
import type { AccessList, AccessListDetail } from '../types';
import * as api from '../lib/api';

interface AccessStore {
  lists: AccessListDetail[];
  loading: boolean;
  error: string | null;
  fetchLists: () => Promise<void>;
  fetchListDetail: (id: string) => Promise<AccessListDetail>;
  createList: (name: string, defaultPolicy: string) => Promise<AccessList>;
  deleteList: (id: string) => Promise<void>;
  createRule: (accessListId: string, action: string, ipCidr: string) => Promise<void>;
  deleteRule: (ruleId: string, accessListId: string) => Promise<void>;
}

export const useAccessStore = create<AccessStore>((set, get) => ({
  lists: [],
  loading: false,
  error: null,
  fetchLists: async () => {
    set({ loading: true, error: null });
    try {
      const lists = await api.listAccessLists();
      set({ lists, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  fetchListDetail: async (id) => {
    const detail = await api.getAccessList(id);
    set((state) => ({
      lists: state.lists.map((l) => (l.list.id === id ? detail : l)),
    }));
    return detail;
  },
  createList: async (name, defaultPolicy) => {
    const list = await api.createAccessList({ name, default_policy: defaultPolicy });
    await get().fetchLists();
    return list;
  },
  deleteList: async (id) => {
    await api.deleteAccessList(id);
    set((state) => ({
      lists: state.lists.filter((l) => l.list.id !== id),
    }));
  },
  createRule: async (accessListId, action, ipCidr) => {
    await api.createAccessRule({ access_list_id: accessListId, action, ip_cidr: ipCidr });
    await get().fetchListDetail(accessListId);
  },
  deleteRule: async (ruleId, accessListId) => {
    await api.deleteAccessRule(ruleId);
    await get().fetchListDetail(accessListId);
  },
}));
