import type { RequestParamKey } from '../types';

/** The six request parameters, in the order the backend lists them. */
export const REQUEST_PARAM_KEYS: RequestParamKey[] = [
  'client_max_body_size',
  'client_body_timeout',
  'proxy_connect_timeout',
  'proxy_read_timeout',
  'proxy_send_timeout',
  'proxy_buffering',
];

/** Row labels, shared by the rule form and the global defaults settings. */
export const REQUEST_PARAM_LABEL_KEYS: Record<RequestParamKey, string> = {
  client_max_body_size: 'proxyForm.paramBodySize',
  client_body_timeout: 'proxyForm.paramBodyReadTimeout',
  proxy_connect_timeout: 'proxyForm.paramConnectTimeout',
  proxy_read_timeout: 'proxyForm.paramReadTimeout',
  proxy_send_timeout: 'proxyForm.paramSendTimeout',
  proxy_buffering: 'proxyForm.paramBuffering',
};

/** The unit hint a row shows, as placeholder or supporting text. */
export const REQUEST_PARAM_HINT_KEYS: Record<RequestParamKey, string> = {
  client_max_body_size: 'proxyForm.requestParamUnitSize',
  client_body_timeout: 'proxyForm.requestParamUnitTime',
  proxy_connect_timeout: 'proxyForm.requestParamUnitTime',
  proxy_read_timeout: 'proxyForm.requestParamUnitTime',
  proxy_send_timeout: 'proxyForm.requestParamUnitTime',
  proxy_buffering: 'proxyForm.requestParamUnitToggle',
};

/**
 * Mirrors the Rust unit rules (`request_params::is_valid`) so a bad value is
 * caught before saving. The backend validates every value again; this is only
 * the fast path, and keeping it in one place stops the rule form and the
 * settings page from growing different grammars.
 */
export function isValidRequestParamValue(directive: RequestParamKey, value: string): boolean {
  if (!value) return false;
  if (directive === 'proxy_buffering') return value === 'on' || value === 'off';
  if (directive === 'client_max_body_size') return /^\d+[kmg]$/i.test(value);
  return /^\d+(ms|s|m|h)$/.test(value);
}
