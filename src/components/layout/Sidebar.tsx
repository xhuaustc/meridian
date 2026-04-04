import { useTranslation } from 'react-i18next';
import { useLocation, useNavigate } from 'react-router-dom';
import {
  BarChart3,
  Activity,
  Lock,
  Shield,
  ClipboardList,
  Settings,
} from 'lucide-react';
import { cn } from '../../lib/utils';
import { useProxyStore } from '../../stores/proxy-store';
import { useCertStore } from '../../stores/cert-store';
import { useAccessStore } from '../../stores/access-store';

interface NavItem {
  icon: React.ElementType;
  labelKey: string;
  path: string;
  count?: number;
}

export function Sidebar() {
  const { t } = useTranslation('common');
  const location = useLocation();
  const navigate = useNavigate();
  const proxyCount = useProxyStore((s) => s.proxies.length);
  const certCount = useCertStore((s) => s.certificates.length);
  const accessCount = useAccessStore((s) => s.lists.length);

  const sections: { titleKey: string; items: NavItem[] }[] = [
    {
      titleKey: 'nav.proxyManagement',
      items: [
        { icon: BarChart3, labelKey: 'nav.dashboard', path: '/', count: proxyCount || undefined },
        { icon: Activity, labelKey: 'nav.monitor', path: '/monitor' },
      ],
    },
    {
      titleKey: 'nav.security',
      items: [
        { icon: Lock, labelKey: 'nav.certs', path: '/certs', count: certCount || undefined },
        { icon: Shield, labelKey: 'nav.access', path: '/access', count: accessCount || undefined },
      ],
    },
    {
      titleKey: 'nav.system',
      items: [
        { icon: ClipboardList, labelKey: 'nav.logs', path: '/logs' },
        { icon: Settings, labelKey: 'nav.settings', path: '/settings' },
      ],
    },
  ];

  const isActive = (path: string) => {
    if (path === '/') return location.pathname === '/';
    return location.pathname.startsWith(path);
  };

  return (
    <div className="bg-bg-sidebar border-r border-border py-3 px-2 flex flex-col gap-0.5 overflow-y-auto">
      {sections.map((section, si) => (
        <div key={si}>
          <div
            className={cn(
              'px-3 text-[10px] font-semibold uppercase tracking-[0.05em] text-text-tertiary mb-1',
              si === 0 ? 'mt-1' : 'mt-4',
            )}
          >
            {t(section.titleKey)}
          </div>
          {section.items.map((item) => {
            const active = isActive(item.path);
            return (
              <button
                key={item.path}
                onClick={() => navigate(item.path)}
                className={cn(
                  'w-full flex items-center gap-2.5 px-3 py-2 rounded-[var(--radius-sm)] cursor-pointer text-[13px] transition-all duration-150 text-left',
                  active
                    ? 'bg-bg-secondary text-text-primary font-medium shadow-[0_1px_2px_rgba(0,0,0,0.04)]'
                    : 'text-text-secondary hover:bg-bg-hover hover:text-text-primary',
                )}
              >
                <item.icon
                  className={cn('w-[18px] h-[18px] shrink-0', active ? 'opacity-100' : 'opacity-65')}
                />
                <span className="flex-1">{t(item.labelKey)}</span>
                {item.count !== undefined && (
                  <span className="text-[11px] text-text-tertiary bg-bg-primary px-1.5 py-px rounded-[10px]">
                    {item.count}
                  </span>
                )}
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}
