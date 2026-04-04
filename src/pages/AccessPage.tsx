import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, Shield } from 'lucide-react';
import { ContentToolbar } from '../components/layout/ContentToolbar';
import { Button } from '../components/ui/Button';
import { Badge } from '../components/ui/Badge';
import { Input } from '../components/ui/Input';
import { Select } from '../components/ui/Select';
import { Dialog, ConfirmDialog } from '../components/ui/Dialog';
import { SkeletonTable } from '../components/ui/Skeleton';
import { useAccessStore } from '../stores/access-store';
import { useToastStore } from '../stores/toast-store';
import { useApiError } from '../hooks/useApiError';
import type { AccessList } from '../types';

export function AccessPage() {
  const { t } = useTranslation('common');
  const { lists, loading, fetchLists, createList, deleteList, createRule, deleteRule } = useAccessStore();
  const addToast = useToastStore((s) => s.addToast);
  const formatError = useApiError();

  const [showCreate, setShowCreate] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<AccessList | null>(null);
  const [newName, setNewName] = useState('');
  const [newPolicy, setNewPolicy] = useState('deny');

  // Inline add rule state per list
  const [addingRule, setAddingRule] = useState<string | null>(null);
  const [ruleIp, setRuleIp] = useState('');
  const [ruleAction, setRuleAction] = useState('allow');

  useEffect(() => {
    fetchLists();
  }, [fetchLists]);

  const handleCreate = async () => {
    if (!newName.trim()) return;
    try {
      await createList(newName, newPolicy);
      addToast('success', t('access.createSuccess'));
      setShowCreate(false);
      setNewName('');
      setNewPolicy('deny');
    } catch (e) {
      addToast('error', formatError(e));
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await deleteList(deleteTarget.id);
      addToast('success', t('access.deleteSuccess'));
    } catch (e) {
      addToast('error', formatError(e));
    }
    setDeleteTarget(null);
  };

  const handleAddRule = async (listId: string) => {
    if (!ruleIp.trim()) return;
    try {
      await createRule(listId, ruleAction, ruleIp.trim());
      addToast('success', t('access.ruleCreateSuccess'));
      setAddingRule(null);
      setRuleIp('');
      setRuleAction('allow');
    } catch (e) {
      addToast('error', formatError(e));
    }
  };

  const handleDeleteRule = async (ruleId: string, listId: string) => {
    try {
      await deleteRule(ruleId, listId);
      addToast('success', t('access.ruleDeleteSuccess'));
    } catch (e) {
      addToast('error', formatError(e));
    }
  };

  return (
    <>
      <ContentToolbar title={t('access.title')}>
        <Button variant="primary" onClick={() => setShowCreate(true)}>
          <Plus className="w-3.5 h-3.5" />
          {t('access.createList')}
        </Button>
      </ContentToolbar>
      <div className="p-6 overflow-y-auto flex-1">
      {loading && lists.length === 0 ? (
        <SkeletonTable rows={3} />
      ) : lists.length === 0 ? (
        <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] py-16 flex flex-col items-center justify-center">
          <Shield className="w-10 h-10 text-text-tertiary mb-3" />
          <p className="text-[13px] font-medium text-text-secondary">
            {t('access.emptyTitle')}
          </p>
          <p className="text-[12px] text-text-tertiary mt-1">
            {t('access.emptyDesc')}
          </p>
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          {lists.map((detail) => {
            const list = detail.list;
            const rules = detail.rules;
            const bound = detail.bound_proxies;

            return (
              <div
                key={list.id}
                className="bg-bg-secondary border border-border rounded-[var(--radius-md)] p-4"
              >
                <div className="flex items-center justify-between mb-2">
                  <div>
                    <div className="font-medium text-[13px]">{list.name}</div>
                    <div className="text-[11px] text-text-tertiary mt-0.5">
                      {t('access.bound')}: {bound.length > 0 ? bound.join(', ') : '-'}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge variant={list.default_policy === 'allow' ? 'allow' : 'deny'}>
                      {list.default_policy === 'allow'
                        ? t('access.policyAllow')
                        : t('access.policyDeny')}
                    </Badge>
                    <button
                      onClick={() => setDeleteTarget(list)}
                      className="p-1 rounded hover:bg-bg-hover text-text-tertiary hover:text-error"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>

                {/* IP Rules */}
                <ul className="list-none text-[12px] font-mono">
                  {rules.map((rule) => (
                    <li
                      key={rule.id}
                      className="py-1 text-text-secondary flex items-center gap-2 group"
                    >
                      <Badge
                        variant={rule.action === 'allow' ? 'allow' : 'deny'}
                        className="text-[11px] font-semibold px-1.5 py-0"
                      >
                        {rule.action === 'allow'
                          ? t('access.actionAllow')
                          : t('access.actionDeny')}
                      </Badge>
                      <span>{rule.ip_cidr}</span>
                      <button
                        onClick={() => handleDeleteRule(rule.id, list.id)}
                        className="ml-auto opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-bg-hover text-text-tertiary hover:text-error"
                      >
                        <Trash2 className="w-3 h-3" />
                      </button>
                    </li>
                  ))}
                </ul>

                {/* Add Rule Inline */}
                {addingRule === list.id ? (
                  <div className="flex items-center gap-2 mt-2">
                    <Select
                      className="w-24"
                      value={ruleAction}
                      onChange={(e) => setRuleAction(e.target.value)}
                    >
                      <option value="allow">{t('access.actionAllow')}</option>
                      <option value="deny">{t('access.actionDeny')}</option>
                    </Select>
                    <Input
                      className="flex-1"
                      value={ruleIp}
                      onChange={(e) => setRuleIp(e.target.value)}
                      placeholder={t('access.ipCidrPlaceholder')}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') handleAddRule(list.id);
                        if (e.key === 'Escape') {
                          setAddingRule(null);
                          setRuleIp('');
                        }
                      }}
                    />
                    <Button size="sm" variant="primary" onClick={() => handleAddRule(list.id)}>
                      {t('access.addRule')}
                    </Button>
                    <Button
                      size="sm"
                      onClick={() => {
                        setAddingRule(null);
                        setRuleIp('');
                      }}
                    >
                      {t('common.cancel')}
                    </Button>
                  </div>
                ) : (
                  <button
                    onClick={() => {
                      setAddingRule(list.id);
                      setRuleIp('');
                      setRuleAction('allow');
                    }}
                    className="mt-2 text-[11px] text-accent hover:text-accent/80 flex items-center gap-1"
                  >
                    <Plus className="w-3 h-3" />
                    {t('access.addRule')}
                  </button>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Create Dialog */}
      <Dialog
        open={showCreate}
        onClose={() => setShowCreate(false)}
        title={t('access.createTitle')}
        footer={
          <>
            <Button onClick={() => setShowCreate(false)}>
              {t('common.cancel')}
            </Button>
            <Button variant="primary" onClick={handleCreate}>
              {t('access.create')}
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-3">
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('access.listName')}
            </label>
            <Input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder={t('access.listNamePlaceholder')}
            />
          </div>
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              {t('access.defaultPolicy')}
            </label>
            <Select
              value={newPolicy}
              onChange={(e) => setNewPolicy(e.target.value)}
            >
              <option value="deny">{t('access.policyDeny')}</option>
              <option value="allow">{t('access.policyAllow')}</option>
            </Select>
          </div>
        </div>
      </Dialog>

      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleDelete}
        title={t('common.delete')}
        message={t('access.deleteConfirm', { name: deleteTarget?.name })}
        confirmText={t('common.delete')}
        danger
      />
      </div>
    </>
  );
}
