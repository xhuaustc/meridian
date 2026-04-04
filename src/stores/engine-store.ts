import { create } from 'zustand';
import type { NginxStatus } from '../types';
import * as api from '../lib/api';

interface EngineStore {
  status: NginxStatus | null;
  loading: boolean;
  fetchStatus: () => Promise<void>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  reload: () => Promise<void>;
  restart: () => Promise<void>;
}

export const useEngineStore = create<EngineStore>((set, get) => ({
  status: null,
  loading: false,
  fetchStatus: async () => {
    try {
      const status = await api.getEngineStatus();
      set({ status });
    } catch {
      set({ status: null });
    }
  },
  start: async () => {
    set({ loading: true });
    try {
      await api.startEngine();
      await get().fetchStatus();
    } finally {
      set({ loading: false });
    }
  },
  stop: async () => {
    set({ loading: true });
    try {
      await api.stopEngine();
      await get().fetchStatus();
    } finally {
      set({ loading: false });
    }
  },
  reload: async () => {
    set({ loading: true });
    try {
      await api.reloadEngine();
      await get().fetchStatus();
    } finally {
      set({ loading: false });
    }
  },
  restart: async () => {
    set({ loading: true });
    try {
      await api.restartEngine();
      await get().fetchStatus();
    } finally {
      set({ loading: false });
    }
  },
}));
