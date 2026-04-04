import { create } from 'zustand';
import type { Certificate } from '../types';
import * as api from '../lib/api';

interface CertStore {
  certificates: Certificate[];
  loading: boolean;
  error: string | null;
  fetchCertificates: () => Promise<void>;
  generateSelfSigned: (name: string, domain: string, days?: number) => Promise<Certificate>;
  importCertificate: (name: string, domain: string, certPem: string, keyPem: string, expiresAt: string) => Promise<Certificate>;
  requestAcmeCert: (domains: string[], dnsCredentialId: string, email: string, autoRenew?: boolean) => Promise<Certificate>;
  deleteCertificate: (id: string) => Promise<void>;
  hasPending: () => boolean;
}

export const useCertStore = create<CertStore>((set, get) => ({
  certificates: [],
  loading: false,
  error: null,
  fetchCertificates: async () => {
    const prev = get().certificates;
    // Only show full loading spinner on initial load
    if (prev.length === 0) set({ loading: true, error: null });
    try {
      const certificates = await api.listCertificates();
      set({ certificates, loading: false, error: null });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  generateSelfSigned: async (name, domain, days) => {
    const cert = await api.generateSelfSignedCert(name, domain, days);
    set((state) => ({ certificates: [...state.certificates, cert] }));
    return cert;
  },
  importCertificate: async (name, domain, certPem, keyPem, expiresAt) => {
    const cert = await api.importCertificate(name, domain, certPem, keyPem, expiresAt);
    set((state) => ({ certificates: [...state.certificates, cert] }));
    return cert;
  },
  requestAcmeCert: async (domains, dnsCredentialId, email, autoRenew) => {
    // Returns immediately with a pending cert — prepend so it appears at the top
    const cert = await api.requestAcmeCert(domains, dnsCredentialId, email, autoRenew);
    set((state) => ({ certificates: [cert, ...state.certificates] }));
    return cert;
  },
  deleteCertificate: async (id) => {
    await api.deleteCertificate(id);
    set((state) => ({ certificates: state.certificates.filter((c) => c.id !== id) }));
  },
  hasPending: () => get().certificates.some((c) => c.status === 'pending'),
}));
