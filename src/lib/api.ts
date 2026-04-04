import { invoke } from '@tauri-apps/api/core';
import type {
  ProxyRule,
  ProxyListResponse,
  CreateProxyRule,
  UpdateProxyRule,
  Certificate,
  DnsCredential,
  RenewalStatus,
  TestResult,
  AccessList,
  AccessListDetail,
  AccessRule,
  AppSetting,
  PortConflict,
  NginxStatus,
  LogChunk,
  ExportData,
  ProxyMetrics,
  HostEntry,
  CreateHostEntry,
} from '../types';

// --- Proxy ---
export const listProxies = (filter?: {
  proxyType?: string;
  enabled?: boolean;
  search?: string;
}) =>
  invoke<ProxyListResponse>('list_proxies', {
    proxyType: filter?.proxyType,
    enabled: filter?.enabled,
    search: filter?.search,
  });

export const getProxy = (id: string) => invoke<ProxyRule>('get_proxy', { id });

export const createProxy = (input: CreateProxyRule) =>
  invoke<ProxyRule>('create_proxy', { input });

export const updateProxy = (id: string, input: UpdateProxyRule) =>
  invoke<ProxyRule>('update_proxy', { id, input });

export const deleteProxy = (id: string) => invoke<void>('delete_proxy', { id });

export const toggleProxy = (id: string, enabled: boolean) =>
  invoke<ProxyRule>('toggle_proxy', { id, enabled });

// --- Certificates ---
export const listCertificates = () => invoke<Certificate[]>('list_certificates');

export const getCertificate = (id: string) =>
  invoke<Certificate>('get_certificate', { id });

export const generateSelfSignedCert = (
  name: string,
  domain: string,
  validityDays?: number,
) =>
  invoke<Certificate>('generate_self_signed_cert', {
    name,
    domain,
    validityDays,
  });

export const importCertificate = (
  name: string,
  domain: string,
  certPem: string,
  keyPem: string,
  expiresAt: string,
) =>
  invoke<Certificate>('import_certificate', {
    name,
    domain,
    certPem,
    keyPem,
    expiresAt,
  });

export const deleteCertificate = (id: string) =>
  invoke<void>('delete_certificate', { id });

export const exportCertificate = (id: string, savePath: string) =>
  invoke<void>('export_certificate', { id, savePath });

export const checkExpiringCerts = (withinDays?: number) =>
  invoke<Certificate[]>('check_expiring_certs', { withinDays });

// --- DNS Credentials ---
export const listDnsCredentials = () =>
  invoke<DnsCredential[]>('list_dns_credentials');

export const createDnsCredential = (
  name: string,
  provider: string,
  credentialsJson: string,
) => invoke<DnsCredential>('create_dns_credential', { name, provider, credentialsJson });

export const updateDnsCredential = (
  id: string,
  name?: string,
  credentialsJson?: string,
) => invoke<DnsCredential>('update_dns_credential', { id, name, credentialsJson });

export const deleteDnsCredential = (id: string) =>
  invoke<void>('delete_dns_credential', { id });

export const testDnsCredential = (id: string) =>
  invoke<TestResult>('test_dns_credential', { id });

// --- ACME ---
export const requestAcmeCert = (
  domains: string[],
  dnsCredentialId: string,
  email: string,
  autoRenew?: boolean,
) =>
  invoke<Certificate>('request_acme_cert', { domains, dnsCredentialId, email, autoRenew });

export const getAcmeRenewalStatus = () =>
  invoke<RenewalStatus[]>('get_acme_renewal_status');

// --- Access Lists ---
export const listAccessLists = () => invoke<AccessListDetail[]>('list_access_lists');

export const getAccessList = (id: string) =>
  invoke<AccessListDetail>('get_access_list', { id });

export const createAccessList = (input: { name: string; default_policy: string }) =>
  invoke<AccessList>('create_access_list', { input });

export const updateAccessList = (
  id: string,
  name?: string,
  defaultPolicy?: string,
) =>
  invoke<AccessList>('update_access_list', {
    id,
    name,
    defaultPolicy,
  });

export const deleteAccessList = (id: string) =>
  invoke<void>('delete_access_list', { id });

export const createAccessRule = (input: {
  access_list_id: string;
  action: string;
  ip_cidr: string;
  sort_order?: number;
}) => invoke<AccessRule>('create_access_rule', { input });

export const deleteAccessRule = (id: string) =>
  invoke<void>('delete_access_rule', { id });

export const reorderAccessRules = (accessListId: string, ruleIds: string[]) =>
  invoke<void>('reorder_access_rules', { accessListId, ruleIds });

// --- Engine ---
export const getEngineStatus = () => invoke<NginxStatus>('get_engine_status');

export const startEngine = () => invoke<void>('start_engine');

export const stopEngine = () => invoke<void>('stop_engine');

export const reloadEngine = () => invoke<PortConflict[]>('reload_engine');

export const applyConfig = () => invoke<PortConflict[]>('apply_config');

export const testNginxConfig = () =>
  invoke<[boolean, string]>('test_nginx_config');

export const detectConflicts = () => invoke<PortConflict[]>('detect_conflicts');

export const restartEngine = () => invoke<void>('restart_engine');

export const checkPortConflict = (
  listenPort: number,
  proxyType: string,
  domain?: string | null,
  pathPrefix?: string | null,
  excludeId?: string | null,
) =>
  invoke<PortConflict[]>('check_port_conflict', {
    listenPort,
    proxyType,
    domain: domain ?? undefined,
    pathPrefix: pathPrefix ?? undefined,
    excludeId: excludeId ?? undefined,
  });

// --- Logs ---
export const readAccessLog = (tailLines?: number, ruleId?: string) =>
  invoke<LogChunk>('read_access_log', { tailLines, ruleId });

export const readErrorLog = (tailLines?: number) =>
  invoke<LogChunk>('read_error_log', { tailLines });

export const clearLogs = () => invoke<void>('clear_logs');

// --- Metrics ---
export const getProxyMetrics = (ruleId?: string, timeRange: string = '1h') =>
  invoke<ProxyMetrics>('get_proxy_metrics', { ruleId, timeRange });

// --- Settings ---
export const getSetting = (key: string) =>
  invoke<string | null>('get_setting', { key });

export const setSetting = (key: string, value: string) =>
  invoke<void>('set_setting', { key, value });

export const listSettings = () => invoke<AppSetting[]>('list_settings');

export const exportData = () => invoke<ExportData>('export_data');

export const importData = (data: ExportData) =>
  invoke<void>('import_data', { data });

export const backupDatabase = () => invoke<string>('backup_database');

// --- Hosts ---
export const listHosts = (keyword?: string) =>
  invoke<HostEntry[]>('list_hosts', { keyword });

export const createHost = (input: CreateHostEntry) =>
  invoke<HostEntry>('create_host', { input });

export const updateHost = (
  id: string,
  ip?: string,
  hostname?: string,
  comment?: string,
) => invoke<HostEntry>('update_host', { id, ip, hostname, comment });

export const deleteHost = (id: string) =>
  invoke<void>('delete_host', { id });

export const toggleHost = (id: string, enabled: boolean) =>
  invoke<HostEntry>('toggle_host', { id, enabled });

export const checkHostnameExists = (hostname: string, excludeId?: string) =>
  invoke<HostEntry | null>('check_hostname_exists', { hostname, excludeId });

export const syncHostsFile = () =>
  invoke<void>('sync_hosts_file');

// --- Tray ---
export const syncTray = () => invoke<void>('sync_tray');

// --- Platform ---
export const getPlatform = () => invoke<string>('get_platform');
