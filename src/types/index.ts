export type ProxyType = 'http' | 'stream_tcp' | 'stream_udp';
export type TlsMode = 'none' | 'terminate' | 'passthrough';
export type CertSource = 'upload' | 'self_signed' | 'acme';
export type AccessPolicy = 'allow' | 'deny';

export interface ProxyRule {
  id: string;
  name: string;
  proxy_type: ProxyType;
  enabled: boolean;
  listen_port: number;
  listen_host: string;
  domain: string | null;
  path_prefix: string | null;
  upstream_host: string;
  upstream_port: number;
  tls_mode: TlsMode;
  certificate_id: string | null;
  access_list_id: string | null;
  websocket: boolean;
  custom_headers: string | null;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

export interface CreateProxyRule {
  name: string;
  proxy_type: string;
  listen_port: number;
  listen_host?: string;
  domain?: string | null;
  path_prefix?: string | null;
  upstream_host: string;
  upstream_port: number;
  tls_mode?: string;
  certificate_id?: string | null;
  access_list_id?: string | null;
  websocket?: boolean;
  custom_headers?: string | null;
  sort_order?: number;
}

export interface UpdateProxyRule {
  name?: string;
  proxy_type?: string;
  enabled?: boolean;
  listen_port?: number;
  listen_host?: string;
  domain?: string | null;
  path_prefix?: string | null;
  upstream_host?: string;
  upstream_port?: number;
  tls_mode?: string;
  certificate_id?: string | null;
  access_list_id?: string | null;
  websocket?: boolean;
  custom_headers?: string | null;
  sort_order?: number;
}

export type CertStatus = 'pending' | 'ready' | 'failed';

export interface Certificate {
  id: string;
  name: string;
  domain: string;
  cert_path: string;
  key_path: string;
  source: CertSource;
  expires_at: string;
  auto_renew: boolean;
  created_at: string;
  dns_credential_id: string | null;
  acme_account_id: string | null;
  acme_domains: string | null;
  last_renew_error: string | null;
  last_renew_at: string | null;
  status: CertStatus;
}

export type DnsProvider = 'cloudflare' | 'alidns' | 'dnspod' | 'route53';

export interface DnsCredential {
  id: string;
  name: string;
  provider: DnsProvider;
  credentials_json: string;
  created_at: string;
  updated_at: string;
}

export interface RenewalStatus {
  cert_id: string;
  cert_name: string;
  domains: string[];
  expires_at: string;
  auto_renew: boolean;
  last_renew_at: string | null;
  last_renew_error: string | null;
  next_renew_at: string;
}

export interface TestResult {
  success: boolean;
  message: string;
}

export interface AccessList {
  id: string;
  name: string;
  default_policy: AccessPolicy;
  created_at: string;
}

export interface AccessRule {
  id: string;
  access_list_id: string;
  action: AccessPolicy;
  ip_cidr: string;
  sort_order: number;
  created_at: string;
}

export interface AccessListWithRules {
  list: AccessList;
  rules: AccessRule[];
}

export interface AccessListDetail {
  list: AccessList;
  rules: AccessRule[];
  bound_proxies: string[];
}

export interface AppSetting {
  key: string;
  value: string;
}

export interface PortConflict {
  rule_id: string;
  rule_name: string;
  conflict_type: string;
  message: string;
}

export interface NginxStatus {
  status: 'running' | 'stopped' | 'error';
  pid: number | null;
  uptime_seconds: number | null;
  error_message: string | null;
}

export interface ProxyListResponse {
  rules: ProxyRule[];
  stats: Record<string, number>;
}

export interface LogChunk {
  lines: string[];
  total_lines: number;
}

export interface ExportData {
  version: string;
  exported_at: string;
  proxy_rules: ProxyRule[];
  certificates: Certificate[];
  access_lists: AccessList[];
  access_rules: AccessRule[];
  settings: AppSetting[];
}

// Monitoring metrics
export interface MetricSummary {
  total_requests: number;
  error_count: number;
  error_rate: number;
  avg_latency_ms: number;
  total_bytes: number;
}

export interface MetricBucket {
  timestamp: string;
  requests: number;
  errors: number;
  avg_latency_ms: number;
  bytes: number;
}

export interface StatusGroup {
  group: string;
  count: number;
}

export interface ProxyMetrics {
  summary: MetricSummary;
  time_series: MetricBucket[];
  status_distribution: StatusGroup[];
}

export interface HostEntry {
  id: string;
  ip: string;
  hostname: string;
  comment: string | null;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateHostEntry {
  ip: string;
  hostname: string;
  comment?: string | null;
}
