/**
 * PROTOTYPE — throwaway. Not production code.
 *
 * Question this answers: where do the new "request params" and "security" controls live,
 * and what do the three presets look like?
 *
 * Three variants of the request-params + security sections, switchable via `?variant=`,
 * on the throwaway route `/prototype/params-ui` (mounted inside the real AppShell).
 *
 * Variant A: inline segmented rows + preset dropdown
 * Variant B: preset cards + grouped cards with switches
 * Variant C: dense tables (advanced view) + table-based settings
 *
 * No persistence, no IPC, no i18n — state is in memory on purpose.
 */
import { useEffect, useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';

type Mode = 'inherit' | 'custom' | 'unlimited';

type ParamKey =
  | 'client_max_body_size'
  | 'client_body_timeout'
  | 'proxy_connect_timeout'
  | 'proxy_read_timeout'
  | 'proxy_send_timeout'
  | 'proxy_buffering';

const PARAMS: { key: ParamKey; label: string; inherit: string; unit: string; kind: 'size' | 'time' | 'switch' }[] = [
  { key: 'client_max_body_size', label: '请求体上限', inherit: '100m', unit: 'k/m/g', kind: 'size' },
  { key: 'client_body_timeout', label: '请求体读取超时', inherit: '60s', unit: 's/m/h', kind: 'time' },
  { key: 'proxy_connect_timeout', label: '连接上游超时', inherit: '60s', unit: 's/m/h', kind: 'time' },
  { key: 'proxy_read_timeout', label: '读取上游超时', inherit: '60s', unit: 's/m/h', kind: 'time' },
  { key: 'proxy_send_timeout', label: '发送上游超时', inherit: '60s', unit: 's/m/h', kind: 'time' },
  { key: 'proxy_buffering', label: '响应缓冲', inherit: 'on', unit: 'on/off', kind: 'switch' },
];

const PRESETS = [
  {
    key: 'upload',
    name: '大文件上传',
    desc: '传大附件、模型权重、镜像',
    values: { client_max_body_size: '2g', client_body_timeout: '300s', proxy_connect_timeout: '10s', proxy_read_timeout: '300s', proxy_send_timeout: '300s', proxy_buffering: 'on' },
  },
  {
    key: 'sse',
    name: 'AI 流式 / SSE',
    desc: '流式输出被缓冲会卡住',
    values: { client_max_body_size: '100m', client_body_timeout: '60s', proxy_connect_timeout: '10s', proxy_read_timeout: '3600s', proxy_send_timeout: '3600s', proxy_buffering: 'off' },
  },
  {
    key: 'long',
    name: '长任务 / 大响应',
    desc: '长轮询、大文件下载',
    values: { client_max_body_size: '100m', client_body_timeout: '60s', proxy_connect_timeout: '10s', proxy_read_timeout: '3600s', proxy_send_timeout: '3600s', proxy_buffering: 'on' },
  },
] as const;

const initialModes: Record<ParamKey, Mode> = {
  client_max_body_size: 'inherit',
  client_body_timeout: 'inherit',
  proxy_connect_timeout: 'inherit',
  proxy_read_timeout: 'inherit',
  proxy_send_timeout: 'inherit',
  proxy_buffering: 'inherit',
};

function useParamsState() {
  const [modes, setModes] = useState<Record<ParamKey, Mode>>({ ...initialModes });
  const [values, setValues] = useState<Partial<Record<ParamKey, string>>>({});
  const setMode = (key: ParamKey, mode: Mode) => setModes((m) => ({ ...m, [key]: mode }));
  const setValue = (key: ParamKey, v: string) => setValues((s) => ({ ...s, [key]: v }));
  const applyPreset = (presetKey: string) => {
    const preset = PRESETS.find((p) => p.key === presetKey);
    if (!preset) return;
    setModes(() => {
      const next = { ...initialModes };
      (Object.keys(initialModes) as ParamKey[]).forEach((k) => { next[k] = 'custom'; });
      return next;
    });
    setValues({ ...preset.values });
  };
  const effective = (p: (typeof PARAMS)[number]) => {
    const mode = modes[p.key];
    if (mode === 'unlimited') return '0（不限制）';
    if (mode === 'custom') return values[p.key] || '—';
    return `继承中：${p.inherit}`;
  };
  return { modes, values, setMode, setValue, applyPreset, effective };
}

const Seg = ({ mode, onChange }: { mode: Mode; onChange: (m: Mode) => void }) => (
  <div className="flex border border-border rounded-[var(--radius-sm)] overflow-hidden w-fit text-[11px]">
    {([
      ['inherit', '继承'],
      ['custom', '自定义'],
      ['unlimited', '不限'],
    ] as const).map(([k, label]) => (
      <button
        key={k}
        type="button"
        onClick={() => onChange(k as Mode)}
        className={`px-2.5 py-1 border-r border-border last:border-r-0 ${
          mode === k ? 'bg-accent text-white' : 'bg-bg-secondary text-text-secondary hover:bg-bg-hover'
        }`}
      >
        {label}
      </button>
    ))}
  </div>
);

function SectionTitle({ children, hint }: { children: string; hint?: string }) {
  return (
    <div className="mb-3 pb-2 border-b border-border flex items-baseline gap-3">
      <h2 className="text-[13px] font-semibold">{children}</h2>
      {hint && <span className="text-[11px] text-text-tertiary">{hint}</span>}
    </div>
  );
}

/* ------------------------------- Variant A ------------------------------- */

function VariantA() {
  const s = useParamsState();
  return (
    <div className="max-w-[680px]">
      <SectionTitle hint="三态行内控件 · 预设收在下拉里">请求参数</SectionTitle>
      <div className="mb-3 flex items-center gap-2">
        <select
          className="text-[12px] border border-border rounded-[var(--radius-sm)] px-2 py-1.5 bg-bg-secondary"
          defaultValue=""
          onChange={(e) => e.target.value && s.applyPreset(e.target.value)}
        >
          <option value="">套用预设…</option>
          {PRESETS.map((p) => <option key={p.key} value={p.key}>{p.name}</option>)}
        </select>
        <span className="text-[11px] text-text-tertiary">预设只填充一次，之后可自由改</span>
      </div>
      <div className="flex flex-col gap-2">
        {PARAMS.map((p) => (
          <div key={p.key} className="flex items-center gap-3 px-3 py-2 bg-bg-secondary border border-border rounded-[var(--radius-md)]">
            <div className="w-[150px] text-[12px]">{p.label}</div>
            <Seg mode={s.modes[p.key]} onChange={(m) => s.setMode(p.key, m)} />
            <div className="flex-1 text-right">
              {s.modes[p.key] === 'custom' ? (
                <input
                  className="w-24 px-2 py-1 border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-primary text-right"
                  placeholder={p.unit}
                  value={s.values[p.key] ?? ''}
                  onChange={(e) => s.setValue(p.key, e.target.value)}
                />
              ) : (
                <span className="text-[11px] text-text-tertiary">{s.effective(p)}</span>
              )}
            </div>
          </div>
        ))}
      </div>

      <div className="mt-8">
        <SectionTitle hint="按域名生效 · 全局默认 + 站点覆盖">安全与 TLS</SectionTitle>
        <div className="flex flex-col gap-2">
          {[
            ['强制 HTTPS 跳转', '生成 80→443 的 308 跳转', true],
            ['HSTS（本域名）', 'max-age 300s · 不含子域', false],
            ['X-Content-Type-Options', 'nosniff · 关掉后老站点可能丢样式', true],
            ['Referrer-Policy', 'strict-origin-when-cross-origin', true],
            ['X-Frame-Options', 'SAMEORIGIN · 跨子域 iframe 会白屏', true],
          ].map(([name, desc, on]) => (
            <label key={name as string} className="flex items-center justify-between px-3 py-2 bg-bg-secondary border border-border rounded-[var(--radius-md)]">
              <div>
                <div className="text-[12px]">{name as string}</div>
                <div className="text-[11px] text-text-tertiary mt-0.5">{desc as string}</div>
              </div>
              <input type="checkbox" defaultChecked={on as boolean} className="accent-[var(--color-accent)]" />
            </label>
          ))}
        </div>
      </div>
    </div>
  );
}

/* ------------------------------- Variant B ------------------------------- */

function VariantB() {
  const s = useParamsState();
  const [applied, setApplied] = useState<string | null>(null);
  return (
    <div>
      <SectionTitle hint="预设卡片在最上，点了就填">请求参数</SectionTitle>
      <div className="grid grid-cols-3 gap-3 mb-5">
        {PRESETS.map((p) => (
          <button
            key={p.key}
            type="button"
            onClick={() => { s.applyPreset(p.key); setApplied(p.key); }}
            className={`text-left p-3 border rounded-[var(--radius-md)] ${
              applied === p.key ? 'border-accent bg-accent-light' : 'border-border bg-bg-secondary hover:bg-bg-hover'
            }`}
          >
            <div className="text-[13px] font-medium">{p.name}</div>
            <div className="text-[11px] text-text-tertiary mt-1">{p.desc}</div>
          </button>
        ))}
      </div>
      <div className="grid grid-cols-2 gap-4">
        {[
          { title: '请求体', keys: ['client_max_body_size', 'client_body_timeout'] as ParamKey[] },
          { title: '上游超时', keys: ['proxy_connect_timeout', 'proxy_read_timeout', 'proxy_send_timeout'] as ParamKey[] },
          { title: '缓冲', keys: ['proxy_buffering'] as ParamKey[] },
        ].map((group) => (
          <div key={group.title} className="border border-border rounded-[var(--radius-md)] bg-bg-secondary">
            <div className="px-3 py-2 border-b border-border text-[12px] font-medium">{group.title}</div>
            <div className="divide-y divide-border">
              {group.keys.map((k) => {
                const p = PARAMS.find((x) => x.key === k)!;
                return (
                  <div key={k} className="px-3 py-2">
                    <div className="flex items-center justify-between">
                      <span className="text-[12px]">{p.label}</span>
                      <span className="text-[11px] text-text-tertiary">{s.effective(p)}</span>
                    </div>
                    <div className="mt-1.5">
                      <Seg mode={s.modes[k]} onChange={(m) => s.setMode(k, m)} />
                      {s.modes[k] === 'custom' && (
                        <input
                          className="mt-1.5 w-full px-2 py-1 border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-primary"
                          placeholder={`如 2g（${p.unit}）`}
                          value={s.values[k] ?? ''}
                          onChange={(e) => s.setValue(k, e.target.value)}
                        />
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        ))}
      </div>
      <div className="mt-6 border border-warning/40 bg-warning-bg rounded-[var(--radius-md)] px-3 py-2 text-[11px] text-warning">
        安全头（XCTO / Referrer-Policy / XFO）默认为开，HSTS 默认关 —— 与「请求参数」分开成独立分区。
      </div>
    </div>
  );
}

/* ------------------------------- Variant C ------------------------------- */

function VariantC() {
  const s = useParamsState();
  return (
    <div>
      <SectionTitle hint="表格视图 · 密度优先">请求参数</SectionTitle>
      <div className="mb-3 flex items-center gap-2">
        {PRESETS.map((p) => (
          <button
            key={p.key}
            type="button"
            onClick={() => s.applyPreset(p.key)}
            className="text-[11px] px-2.5 py-1 border border-border rounded-[var(--radius-sm)] bg-bg-secondary hover:bg-bg-hover"
          >
            {p.name}
          </button>
        ))}
        <span className="text-[11px] text-text-tertiary ml-1">一次性填充</span>
      </div>
      <div className="border border-border rounded-[var(--radius-md)] overflow-hidden bg-bg-secondary">
        <table className="w-full text-[12px]">
          <thead>
            <tr className="bg-bg-sidebar text-text-secondary text-[11px]">
              <th className="text-left px-3 py-2 font-medium">参数</th>
              <th className="text-left px-3 py-2 font-medium">模式</th>
              <th className="text-left px-3 py-2 font-medium">值</th>
              <th className="text-left px-3 py-2 font-medium">生效值</th>
            </tr>
          </thead>
          <tbody>
            {PARAMS.map((p) => (
              <tr key={p.key} className="border-t border-border">
                <td className="px-3 py-1.5">{p.label}</td>
                <td className="px-3 py-1.5"><Seg mode={s.modes[p.key]} onChange={(m) => s.setMode(p.key, m)} /></td>
                <td className="px-3 py-1.5">
                  {s.modes[p.key] === 'custom' ? (
                    <input
                      className="w-20 px-2 py-0.5 border border-border rounded-[var(--radius-sm)] text-[12px] bg-bg-primary"
                      placeholder={p.unit}
                      value={s.values[p.key] ?? ''}
                      onChange={(e) => s.setValue(p.key, e.target.value)}
                    />
                  ) : (
                    <span className="text-text-tertiary">—</span>
                  )}
                </td>
                <td className="px-3 py-1.5 text-text-secondary">{s.effective(p)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="mt-6">
        <SectionTitle hint="同一张表里管全局默认与域名覆盖">设置页：默认值与域名</SectionTitle>
        <div className="border border-border rounded-[var(--radius-md)] overflow-hidden bg-bg-secondary">
          <table className="w-full text-[12px]">
            <thead>
              <tr className="bg-bg-sidebar text-text-secondary text-[11px]">
                <th className="text-left px-3 py-2 font-medium">域名</th>
                <th className="text-left px-3 py-2 font-medium">HSTS</th>
                <th className="text-left px-3 py-2 font-medium">max-age</th>
                <th className="text-left px-3 py-2 font-medium">引用规则</th>
              </tr>
            </thead>
            <tbody>
              {[
                ['app.example.com', '开', '300s', '2 条'],
                ['api.example.com', '关', '—', '1 条'],
                ['old.example.com', '开', '300s', '未引用 · 孤儿'],
              ].map(([host, hsts, age, refs]) => (
                <tr key={host} className="border-t border-border">
                  <td className="px-3 py-1.5">{host}</td>
                  <td className="px-3 py-1.5">{hsts}</td>
                  <td className="px-3 py-1.5">{age}</td>
                  <td className={`px-3 py-1.5 ${refs.includes('孤儿') ? 'text-warning' : 'text-text-tertiary'}`}>{refs}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="mt-2 flex items-center justify-between text-[11px] text-text-tertiary">
          <span>清理孤儿记录不会清除浏览器的 HSTS 缓存</span>
          <button type="button" className="px-2.5 py-1 border border-border rounded-[var(--radius-sm)] bg-bg-secondary">清理未使用</button>
        </div>
      </div>
    </div>
  );
}

/* ------------------------------- Switcher -------------------------------- */

const VARIANTS = [
  { key: 'A', name: 'Segmented rows', render: <VariantA /> },
  { key: 'B', name: 'Preset cards', render: <VariantB /> },
  { key: 'C', name: 'Dense tables', render: <VariantC /> },
];

function PrototypeSwitcher({ current, onCycle }: { current: string; onCycle: (dir: 1 | -1) => void }) {
  const idx = VARIANTS.findIndex((v) => v.key === current);
  return (
    <div className="fixed bottom-5 left-1/2 -translate-x-1/2 z-50 flex items-center gap-3 px-3 py-2 rounded-full bg-text-primary text-bg-primary shadow-lg text-[12px]">
      <button type="button" onClick={() => onCycle(-1)} className="px-1.5" aria-label="previous variant">←</button>
      <span className="font-medium">{current} ({VARIANTS[idx]?.name})</span>
      <button type="button" onClick={() => onCycle(1)} className="px-1.5" aria-label="next variant">→</button>
      <span className="text-[10px] opacity-60">prototype · ?variant=</span>
    </div>
  );
}

/* -------------------------------- Page ----------------------------------- */

export function PrototypeParamsUiPage() {
  const [search, setSearch] = useSearchParams();
  const variant = (search.get('variant') ?? 'A').toUpperCase();
  const valid = useMemo(() => VARIANTS.some((v) => v.key === variant), [variant]);

  const cycle = (dir: 1 | -1) => {
    const i = VARIANTS.findIndex((v) => v.key === variant);
    const next = VARIANTS[(i + dir + VARIANTS.length) % VARIANTS.length].key;
    const params = new URLSearchParams(search);
    params.set('variant', next);
    setSearch(params, { replace: true });
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
      if (e.key === 'ArrowLeft') cycle(-1);
      if (e.key === 'ArrowRight') cycle(1);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  const active = valid ? variant : 'A';

  return (
    <div className="p-6 overflow-y-auto flex-1">
      <div className="mb-5">
        <div className="text-[15px] font-semibold">PROTOTYPE · 请求参数与安全区</div>
        <div className="text-[11px] text-text-tertiary mt-1">
          一次性原型，不并入主干。用底部胶囊或 ←/→ 切换变体；URL 里带 <code>?variant=A|B|C</code>
        </div>
      </div>
      {VARIANTS.find((v) => v.key === active)?.render}
      {import.meta.env.DEV && <PrototypeSwitcher current={active} onCycle={cycle} />}
    </div>
  );
}
