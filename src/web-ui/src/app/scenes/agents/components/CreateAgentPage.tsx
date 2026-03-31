import React, { useState, useEffect, useCallback } from 'react';
import { ArrowLeft } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Input, Textarea, Switch, Button } from '@/component-library';
import { SubagentAPI } from '@/infrastructure/api/service-api/SubagentAPI';
import type { SubagentLevel } from '@/infrastructure/api/service-api/SubagentAPI';
import { useNotification } from '@/shared/notification-system';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useAgentsStore } from '../agentsStore';
import '../AgentsView.scss';
import './CreateAgentPage.scss';

const NAME_REGEX = /^[a-zA-Z][a-zA-Z0-9_-]*$/;

const CreateAgentPage: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const { openHome, agentEditorMode, editingAgentId } = useAgentsStore();
  const notification = useNotification();
  const { hasWorkspace, workspacePath } = useCurrentWorkspace();

  const isEdit = agentEditorMode === 'edit' && Boolean(editingAgentId);

  const [level, setLevel] = useState<SubagentLevel>('user');
  const [name, setName] = useState('');
  const [nameError, setNameError] = useState<string | null>(null);
  const [description, setDescription] = useState('');
  const [prompt, setPrompt] = useState('');
  const [readonly, setReadonly] = useState(true);
  const [toolNames, setToolNames] = useState<string[]>([]);
  const [selectedTools, setSelectedTools] = useState<Set<string>>(new Set());
  const [submitting, setSubmitting] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);

  useEffect(() => {
    SubagentAPI.listAgentToolNames().then(setToolNames).catch(() => setToolNames([]));
  }, []);

  useEffect(() => {
    if (!hasWorkspace && level === 'project') {
      setLevel('user');
    }
  }, [hasWorkspace, level]);

  useEffect(() => {
    if (!isEdit || !editingAgentId) {
      setDetailLoading(false);
      setDetailError(null);
      return;
    }

    let cancelled = false;
    setDetailLoading(true);
    setDetailError(null);

    (async () => {
      try {
        const d = await SubagentAPI.getSubagentDetail({
          subagentId: editingAgentId,
          workspacePath: workspacePath || undefined,
        });
        if (cancelled) return;
        setName(d.name);
        setDescription(d.description);
        setPrompt(d.prompt);
        setReadonly(d.readonly);
        setLevel(d.level);
        setSelectedTools(new Set(d.tools ?? []));
        setNameError(null);
      } catch (e) {
        if (cancelled) return;
        setDetailError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setDetailLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [isEdit, editingAgentId, workspacePath]);

  const validateName = useCallback((v: string) => {
    if (!v.trim()) return t('agentsOverview.form.nameRequired', '名称不能为空');
    if (!NAME_REGEX.test(v.trim())) return t('agentsOverview.form.nameFormat', '只能以字母开头，包含字母/数字/下划线/连字符');
    return null;
  }, [t]);

  const toggleTool = (tool: string) => {
    setSelectedTools((prev) => {
      const next = new Set(prev);
      if (next.has(tool)) {
        next.delete(tool);
      } else {
        next.add(tool);
      }
      return next;
    });
  };

  const handleSubmit = async () => {
    if (!isEdit) {
      const err = validateName(name);
      if (err) { setNameError(err); return; }
    }
    if (!description.trim()) { notification.error(t('agentsOverview.form.descRequired', '描述不能为空')); return; }
    if (!prompt.trim()) { notification.error(t('agentsOverview.form.promptRequired', '系统提示词不能为空')); return; }
    if (level === 'project' && !workspacePath) {
      notification.error(t('agentsOverview.form.noWorkspace', '需要先打开项目'));
      return;
    }
    if (isEdit && !editingAgentId) {
      return;
    }

    setSubmitting(true);
    try {
      if (isEdit && editingAgentId) {
        await SubagentAPI.updateSubagent({
          subagentId: editingAgentId,
          description: description.trim(),
          prompt: prompt.trim(),
          readonly,
          tools: selectedTools.size > 0 ? Array.from(selectedTools) : undefined,
          workspacePath: level === 'project' ? workspacePath : undefined,
        });
        notification.success(t('agentsOverview.form.updateSuccess', { name: name.trim() }));
      } else {
        await SubagentAPI.createSubagent({
          level,
          name: name.trim(),
          description: description.trim(),
          prompt: prompt.trim(),
          readonly,
          tools: selectedTools.size > 0 ? Array.from(selectedTools) : undefined,
          workspacePath: level === 'project' ? workspacePath : undefined,
        });
        notification.success(t('agentsOverview.form.createSuccess', { name: name.trim() }));
      }
      openHome();
    } catch (err) {
      notification.error(
        (isEdit ? t('agentsOverview.form.updateFailed', '保存失败：') : t('agentsOverview.form.createFailed', '创建失败：')) +
        (err instanceof Error ? err.message : String(err))
      );
    } finally {
      setSubmitting(false);
    }
  };

  const formTitle = isEdit
    ? t('agentsOverview.form.titleEdit', '编辑 Sub-Agent')
    : t('agentsOverview.form.title', '新建 Sub-Agent');
  const formSubtitle = isEdit
    ? t('agentsOverview.form.subtitleEdit', '修改描述、提示词、工具与只读设置。名称与级别不可更改。')
    : t('agentsOverview.form.subtitle', '创建一个自定义的用户级或项目级 Sub-Agent');
  const submitLabel = isEdit
    ? t('agentsOverview.form.save', '保存')
    : t('agentsOverview.form.submit', '创建');

  if (isEdit && detailLoading) {
    return (
      <div className="tv">
        <div className="tv__editor-bar">
          <button className="tv__back-btn" onClick={openHome} type="button">
            <ArrowLeft size={14} />
            <span>{t('agentsOverview.backToOverview', '返回总览')}</span>
          </button>
        </div>
        <div className="th__list-body">
          <div className="th__list-inner">
            <p className="th__title-sub">{t('agentsOverview.form.loadingDetail', '加载中…')}</p>
          </div>
        </div>
      </div>
    );
  }

  if (isEdit && detailError) {
    return (
      <div className="tv">
        <div className="tv__editor-bar">
          <button className="tv__back-btn" onClick={openHome} type="button">
            <ArrowLeft size={14} />
            <span>{t('agentsOverview.backToOverview', '返回总览')}</span>
          </button>
        </div>
        <div className="th__list-body">
          <div className="th__list-inner">
            <p className="th-create-panel__error">{detailError}</p>
            <Button variant="secondary" size="small" onClick={openHome}>{t('agentsOverview.form.cancel', '取消')}</Button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="tv">
      <div className="tv__editor-bar">
        <button className="tv__back-btn" onClick={openHome} type="button">
          <ArrowLeft size={14} />
          <span>{t('agentsOverview.backToOverview', '返回总览')}</span>
        </button>
      </div>

      <div className="th__list-body">
        <div className="th__list-inner">
          <div className="th-create-page__head">
            <h2 className="th__title">{formTitle}</h2>
            <p className="th__title-sub">{formSubtitle}</p>
          </div>

          <div className="th-create-page__form">
            <div className="th-create-panel__field">
              <label className="th-create-panel__label">{t('agentsOverview.form.name', '名称')}</label>
              <Input
                value={name}
                onChange={(e) => { setName(e.target.value); setNameError(validateName(e.target.value)); }}
                onBlur={() => setNameError(validateName(name))}
                placeholder={t('agentsOverview.form.namePlaceholder', '字母开头，可含字母/数字/下划线')}
                inputSize="small"
                error={!!nameError}
                disabled={isEdit}
              />
              {nameError && <span className="th-create-panel__error">{nameError}</span>}
            </div>

            <div className="th-create-panel__field">
              <label className="th-create-panel__label">{t('agentsOverview.form.description', '描述')}</label>
              <Input
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={t('agentsOverview.form.descPlaceholder', '简要描述该 Agent 的用途')}
                inputSize="small"
              />
            </div>

            <div className="th-create-panel__field th-create-panel__field--row">
              <div className="th-create-panel__level-group">
                {(['user', 'project'] as SubagentLevel[]).map((lv) => {
                  const disabled = (lv === 'project' && !hasWorkspace) || isEdit;
                  return (
                    <button
                      key={lv}
                      type="button"
                      disabled={disabled}
                      className={`th-create-panel__level-btn${level === lv ? ' is-active' : ''}`}
                      onClick={() => setLevel(lv)}
                      title={disabled && !isEdit ? t('agentsOverview.form.noWorkspace', '需要先打开项目') : undefined}
                    >
                      {lv === 'user' ? t('agentsOverview.filterUser', '用户级') : t('agentsOverview.filterProject', '项目级')}
                    </button>
                  );
                })}
              </div>
              <div className="th-create-panel__readonly-row">
                <label className="th-create-panel__label">{t('agentsOverview.form.readonly', '只读模式')}</label>
                <Switch checked={readonly} onChange={(e) => setReadonly(e.target.checked)} size="small" />
              </div>
            </div>

            {toolNames.length > 0 && (
              <div className="th-create-panel__field">
                <label className="th-create-panel__label">
                  {t('agentsOverview.form.tools', '工具')}
                  <span className="th-create-panel__label-hint">
                    （{t('agentsOverview.form.toolsOptional', '可选')}，不选则使用默认）
                  </span>
                </label>
                <div className="th-create-panel__tools">
                  {toolNames.map((tool) => (
                    <button
                      key={tool}
                      type="button"
                      className={`th-list__tool-item${selectedTools.has(tool) ? ' is-on' : ''}`}
                      onClick={() => toggleTool(tool)}
                    >
                      <span className="th-list__tool-item-name">{tool}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}

            <div className="th-create-panel__field">
              <label className="th-create-panel__label">{t('agentsOverview.form.prompt', '系统提示词')}</label>
              <Textarea
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder={t('agentsOverview.form.promptPlaceholder', '输入系统提示词，定义 Agent 的行为…')}
                rows={8}
              />
            </div>

            <div className="th-create-page__actions">
              <Button variant="secondary" size="small" onClick={openHome} disabled={submitting}>
                {t('agentsOverview.form.cancel', '取消')}
              </Button>
              <Button variant="primary" size="small" onClick={handleSubmit} disabled={submitting}>
                {submitting ? '…' : submitLabel}
              </Button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default CreateAgentPage;
