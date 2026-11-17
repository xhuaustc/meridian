export type ProxyType = 'http' | 'stream_tcp' | 'stream_udp';
export type TlsMode = 'none' | 'terminate' | 'passthrough';
export type CertSource = 'upload' | 'self_signed' | 'acme';
export type AccessPolicy = 'allow' | 'deny';

export type UpstreamScheme = 'http' | 'https';

/** The six request parameters. Names are the nginx directives they produce. */
export type RequestParamKey =
  | 'client_max_body_size'
  | 'client_body_timeout'
  | 'proxy_connect_timeout'
  | 'proxy_read_timeout'
  | 'proxy_send_timeout'
  | 'proxy_buffering';

/** Which unit rules the custom input follows. */
export type RequestParamKind = 'size' | 'duration' | 'toggle';

export type RequestParamValues = Record<RequestParamKey, string>;

export interface RequestParamInfo {
  directive: RequestParamKey;
  kind: RequestParamKind;
  /** `proxy_buffering` only takes on/off, so it offers no "unlimited" state. */
  supports_unlimited: boolean;
}

export interface RequestParamPreset {
  id: string;
  values: RequestParamValues;
}

export interface RequestParamCatalog {
  /** The global value an inheriting row shows. */
  defaults: RequestParamValues;
  /** The six rows, in display order. */
  params: RequestParamInfo[];
  presets: RequestParamPreset[];
}

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
  upstream_scheme: UpstreamScheme;
  tls_mode: TlsMode;
  certificate_id: string | null;
  access_list_id: string | null;
  websocket: boolean;
  keep_alive: boolean;
  /** Emit a synthesised port-80 308 redirect for this hostname (http + TLS terminate only). */
  force_https: boolean;
  custom_headers: string | null;
  upstream_targets: string | null;
  /** `null` inherits the global default; `0` means unlimited where nginx accepts it. */
  client_max_body_size: string | null;
  client_body_timeout: string | null;
  proxy_connect_timeout: string | null;
  proxy_read_timeout: string | null;
  proxy_send_timeout: string | null;
  proxy_buffering: string | null;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

/** Why the synthesised HTTP -> HTTPS redirect block will not be generated. */
export type ForceHttpsSkip = 'existing_port_80_rule' | 'own_port_80';

export interface ForceHttpsCheck {
  synthesized: boolean;
  skipped: ForceHttpsSkip | null;
  blocking_rule_name: string | null;
}

/**
 * HSTS is stored per hostname, not per rule. `owner_rule_id` names the rule
 * whose form shows the toggle; every other rule on the hostname shows the
 * read-only hint instead of a second switch.
 */
export interface HostnameHstsCheck {
  enabled: boolean;
  max_age: number;
  include_subdomains: boolean;
  owner_rule_id: string | null;
  owner_rule_name: string | null;
}

/** A stored `hostname_settings` row, as the backend returns it. */
export interface HostnameHstsSettings {
  hostname: string;
  hsts_enabled: boolean;
  hsts_max_age: number;
  hsts_include_subdomains: boolean;
  sec_nosniff: boolean;
  sec_referrer_policy: boolean;
  sec_frame_options: boolean;
  created_at: string;
  updated_at: string;
}

export interface UpstreamTarget {
  host: string;
  port: number;
  weight?: number;
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
  upstream_scheme?: string;
  tls_mode?: string;
  certificate_id?: string | null;
  access_list_id?: string | null;
  websocket?: boolean;
  keep_alive?: boolean;
  force_https?: boolean;
  custom_headers?: string | null;
  upstream_targets?: string | null;
  client_max_body_size?: string | null;
  client_body_timeout?: string | null;
  proxy_connect_timeout?: string | null;
  proxy_read_timeout?: string | null;
  proxy_send_timeout?: string | null;
  proxy_buffering?: string | null;
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
  upstream_scheme?: string;
  tls_mode?: string;
  certificate_id?: string | null;
  access_list_id?: string | null;
  websocket?: boolean;
  keep_alive?: boolean;
  force_https?: boolean;
  custom_headers?: string | null;
  upstream_targets?: string | null;
  client_max_body_size?: string | null;
  client_body_timeout?: string | null;
  proxy_connect_timeout?: string | null;
  proxy_read_timeout?: string | null;
  proxy_send_timeout?: string | null;
  proxy_buffering?: string | null;
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

export interface EngineCapabilities {
  probed: boolean;
  http2: boolean;
}

export interface ProxyListResponse {
  rules: ProxyRule[];
  stats: Record<string, number>;
}

export interface LogChunk {
  lines: string[];
  total_lines: number;
}

export interface RecoveryPreview {
  exported_at: string;
  proxy_count: number;
  certificate_count: number;
  host_count: number;
  dns_credential_count: number;
}

export interface ConfigPreview {
  valid: boolean;
  test_message: string;
  conflicts: PortConflict[];
  changes: Array<{ path: string; before: string | null; after: string | null }>;
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

export interface HostsSyncStatus {
  synced: boolean | null;
  checked_at: string | null;
  error: string | null;
}

export interface CreateHostEntry {
  ip: string;
  hostname: string;
  comment?: string | null;
}
