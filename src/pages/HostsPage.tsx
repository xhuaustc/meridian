import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, Pencil, Globe, RefreshCw } from 'lucide-react';
import { ContentToolbar } from '../components/layout/ContentToolbar';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';
import { Toggle } from '../components/ui/Toggle';
import { Dialog, ConfirmDialog } from '../components/ui/Dialog';
import { useHostsStore } from '../stores/hosts-store';
import { useToastStore } from '../stores/toast-store';
import type { HostEntry } from '../types';

export function HostsPage() {
  const { t } = useTranslation('common');
  const { entries, fetchEntries, createEntry, updateEntry, deleteEntry, toggleEntry, syncToSystem } =
    useHostsStore();
  const addToast = useToastStore((s) => s.addToast);

  const [search, setSearch] = useState('');
  const [showCreate, setShowCreate] = useState(false);
  const [editTarget, setEditTarget] = useState<HostEntry | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<HostEntry | null>(null);
  const [syncing, setSyncing] = useState(false);

  // Form state (shared by create and edit dialogs)
  const [formHostname, setFormHostname] = useState('');
  const [formIp, setFormIp] = useState('');
  const [formComment, setFormComment] = useState('');

  useEffect(() => {
    fetchEntries();
  }, [fetchEntries]);

  const filteredEntries = search
    ? entries.filter(
        (e) =>
          e.hostname.toLowerCase().includes(search.toLowerCase()) ||
          e.ip.includes(search) ||
          (e.comment && e.comment.toLowerCase().includes(search.toLowerCase())),
      )
    : entries;

  const openCreate = () => {
    setFormHostname('');
    setFormIp('127.0.0.1');
    setFormComment('');
    setShowCreate(true);
  };

  const openEdit = (entry: HostEntry) => {
    setFormHostname(entry.hostname);
    setFormIp(entry.ip);
    setFormComment(entry.comment ?? '');
    setEditTarget(entry);
  };

  const handleCreate = async () => {
    if (!formHostname.trim() || !formIp.trim()) return;
    try {
      await createEntry(formIp.trim(), formHostname.trim(), formComment.trim() || undefined);
      addToast('success', t('hosts.createSuccess'));
      setShowCreate(false);
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleUpdate = async () => {
    if (!editTarget || !formHostname.trim() || !formIp.trim()) return;
    try {
      await updateEntry(
        editTarget.id,
        formIp.trim(),
        formHostname.trim(),
        formComment.trim() || undefined,
      );
      addToast('success', t('hosts.updateSuccess'));
      setEditTarget(null);
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await deleteEntry(deleteTarget.id);
      addToast('success', t('hosts.deleteSuccess'));
    } catch (e) {
      addToast('error', String(e));
    }
    setDeleteTarget(null);
  };

  const handleToggle = async (entry: HostEntry) => {
    try {
      await toggleEntry(entry.id, !entry.enabled);
      addToast('success', t('hosts.toggleSuccess'));
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleSync = async () => {
    setSyncing(true);
    try {
      await syncToSystem();
      addToast('success', t('hosts.syncSuccess'));
    } catch (e) {
      addToast('error', String(e));
    } finally {
      setSyncing(false);
    }
  };

  const formDialog = (
    open: boolean,
    onClose: () => void,
    title: string,
    onSubmit: () => void,
    submitLabel: string,
  ) => (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      footer={
        <>
          <Button onClick={onClose}>{t('common.cancel')}</Button>
          <Button variant="primary" onClick={onSubmit}>
            {submitLabel}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <div>
          <label className="block text-[12px] font-medium text-text-secondary mb-1">
            {t('hosts.hostname')}
          </label>
          <Input
            value={formHostname}
            onChange={(e) => setFormHostname(e.target.value)}
            placeholder={t('hosts.hostnamePlaceholder')}
          />
        </div>
        <div>
          <label className="block text-[12px] font-medium text-text-secondary mb-1">
            {t('hosts.ip')}
          </label>
          <Input
            value={formIp}
            onChange={(e) => setFormIp(e.target.value)}
            placeholder={t('hosts.ipPlaceholder')}
          />
        </div>
        <div>
          <label className="block text-[12px] font-medium text-text-secondary mb-1">
            {t('hosts.comment')}
          </label>
          <Input
            value={formComment}
            onChange={(e) => setFormComment(e.target.value)}
            placeholder={t('hosts.commentPlaceholder')}
          />
        </div>
      </div>
    </Dialog>
  );

  return (
    <>
      <ContentToolbar title={t('hosts.title')}>
        <Button variant="primary" onClick={openCreate}>
          <Plus className="w-3.5 h-3.5" />
          {t('hosts.addEntry')}
        </Button>
      </ContentToolbar>
      <div className="p-6 overflow-y-auto flex-1">
        {/* Search */}
        <div className="mb-4">
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t('hosts.searchPlaceholder')}
            className="max-w-sm"
          />
        </div>

        {filteredEntries.length === 0 ? (
          <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] py-16 flex flex-col items-center justify-center">
            <Globe className="w-10 h-10 text-text-tertiary mb-3" />
            <p className="text-[13px] font-medium text-text-secondary">
              {t('hosts.emptyTitle')}
            </p>
            <p className="text-[12px] text-text-tertiary mt-1">
              {t('hosts.emptyDesc')}
            </p>
          </div>
        ) : (
          <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] overflow-hidden">
            <table className="w-full text-[13px]">
              <thead>
                <tr className="border-b border-border text-text-tertiary text-[11px] uppercase tracking-wide">
                  <th className="px-4 py-2 text-left w-12" />
                  <th className="px-4 py-2 text-left">{t('hosts.hostname')}</th>
                  <th className="px-4 py-2 text-left">{t('hosts.ip')}</th>
                  <th className="px-4 py-2 text-left">{t('hosts.comment')}</th>
                  <th className="px-4 py-2 text-right w-24" />
                </tr>
              </thead>
              <tbody>
                {filteredEntries.map((entry) => (
                  <tr
                    key={entry.id}
                    className="border-b border-border last:border-b-0 hover:bg-bg-hover"
                  >
                    <td className="px-4 py-2">
                      <Toggle
                        checked={entry.enabled}
                        onChange={() => handleToggle(entry)}
                      />
                    </td>
                    <td className="px-4 py-2 font-mono">{entry.hostname}</td>
                    <td className="px-4 py-2 font-mono text-text-secondary">{entry.ip}</td>
                    <td className="px-4 py-2 text-text-tertiary">{entry.comment ?? '-'}</td>
                    <td className="px-4 py-2 text-right">
                      <div className="flex items-center justify-end gap-1">
                        <button
                          onClick={() => openEdit(entry)}
                          className="p-1 rounded hover:bg-bg-hover text-text-tertiary hover:text-text-primary"
                        >
                          <Pencil className="w-3.5 h-3.5" />
                        </button>
                        <button
                          onClick={() => setDeleteTarget(entry)}
                          className="p-1 rounded hover:bg-bg-hover text-text-tertiary hover:text-error"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {/* Sync button */}
        <div className="mt-4 flex items-center gap-2">
          <Button onClick={handleSync} disabled={syncing}>
            <RefreshCw className={`w-3.5 h-3.5 ${syncing ? 'animate-spin' : ''}`} />
            {t('hosts.syncButton')}
          </Button>
          <span className="text-[11px] text-text-tertiary">{t('hosts.syncHint')}</span>
        </div>
      </div>

      {/* Create Dialog */}
      {formDialog(showCreate, () => setShowCreate(false), t('hosts.createTitle'), handleCreate, t('hosts.create'))}

      {/* Edit Dialog */}
      {formDialog(!!editTarget, () => setEditTarget(null), t('hosts.editTitle'), handleUpdate, t('hosts.save'))}

      {/* Delete Confirm */}
      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleDelete}
        title={t('common.delete')}
        message={t('hosts.deleteConfirm', { hostname: deleteTarget?.hostname })}
        confirmText={t('common.delete')}
        danger
      />
    </>
  );
}
