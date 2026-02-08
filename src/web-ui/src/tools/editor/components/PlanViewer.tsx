/** Optimized viewer/editor for `.plan.md` files (frontmatter + markdown body). */

import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { Circle, ArrowRight, Check, XCircle, Loader2, CheckCircle, AlertCircle, FileText, Pencil, X, ChevronDown, ChevronUp } from 'lucide-react';
import yaml from 'yaml';
import { MEditor } from '../meditor';
import type { EditorInstance } from '../meditor';
import { createLogger } from '@/shared/utils/logger';
import { CubeLoading, Button } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { fileSystemService } from '@/tools/file-system/services/FileSystemService';
import './PlanViewer.scss';

const log = createLogger('PlanViewer');

// Styles used by markdown rendering (math + code highlight).
import 'katex/dist/katex.min.css';
import 'highlight.js/styles/github-dark.css';

interface PlanTodo {
  id: string;
  content: string;
  status?: string;
  dependencies?: string[];
}

interface PlanData {
  name: string;
  overview: string;
  todos: PlanTodo[];
}

export interface PlanViewerProps {
  /** File path */
  filePath: string;
  /** Workspace path */
  workspacePath?: string;
  /** File name */
  fileName?: string;
  /** Jump to specified line number */
  jumpToLine?: number;
  /** Jump to specified column number */
  jumpToColumn?: number;
}

// File write guard: prevents file watcher loops during internal writes.
const writingFiles = new Set<string>();

function markFileWriting(filePath: string): void {
  const normalizedPath = filePath.replace(/\\/g, '/');
  writingFiles.add(normalizedPath);
  setTimeout(() => {
    writingFiles.delete(normalizedPath);
  }, 1000);
}

function isFileWriting(filePath: string): boolean {
  const normalizedPath = filePath.replace(/\\/g, '/');
  return writingFiles.has(normalizedPath);
}

const PlanViewer: React.FC<PlanViewerProps> = ({
  filePath,
  workspacePath,
  fileName,
  jumpToLine: _jumpToLine,
  jumpToColumn: _jumpToColumn,
}) => {
  const { t } = useI18n('tools');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [planData, setPlanData] = useState<PlanData | null>(null);
  const [planContent, setPlanContent] = useState<string>('');
  const [isBuildStarted, setIsBuildStarted] = useState(false);
  const [originalContent, setOriginalContent] = useState('');
  // Edit mode: display raw yaml frontmatter
  const [isEditingYaml, setIsEditingYaml] = useState(false);
  const [yamlContent, setYamlContent] = useState<string>('');
  const [originalYamlContent, setOriginalYamlContent] = useState<string>('');
  // Todos list expand/collapse state (collapsed by default)
  const [isTodosExpanded, setIsTodosExpanded] = useState(false);
  
  const editorRef = useRef<EditorInstance>(null);
  const yamlEditorRef = useRef<EditorInstance>(null);
  const isUnmountedRef = useRef(false);

  const basePath = useMemo(() => {
    if (!filePath) return undefined;
    const normalizedPath = filePath.replace(/\\/g, '/');
    const lastSlashIndex = normalizedPath.lastIndexOf('/');
    if (lastSlashIndex >= 0) {
      return normalizedPath.substring(0, lastSlashIndex);
    }
    return undefined;
  }, [filePath, t]);

  const displayFileName = useMemo(() => {
    if (fileName) return fileName;
    if (!filePath) return '';
    const parts = filePath.replace(/\\/g, '/').split('/');
    return parts[parts.length - 1] || '';
  }, [filePath, fileName]);

  useEffect(() => {
    isUnmountedRef.current = false;
    return () => {
      isUnmountedRef.current = true;
      editorRef.current?.destroy();
      yamlEditorRef.current?.destroy();
    };
  }, []);

  const loadFileContent = useCallback(async () => {
    if (!filePath || isUnmountedRef.current) return;

    if (isFileWriting(filePath)) {
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const content = await workspaceAPI.readFileContent(filePath);

      const frontmatterMatch = content.match(/^---\n([\s\S]*?)\n---/);
      if (frontmatterMatch) {
        const rawYaml = frontmatterMatch[1];
        const parsed = yaml.parse(rawYaml);
        const markdownContent = content.replace(/^---\n[\s\S]*?\n---\n*/, '').trim();

        if (!isUnmountedRef.current) {
          setPlanData({
            name: parsed.name || '',
            overview: parsed.overview || '',
            todos: parsed.todos || [],
          });
          setPlanContent(markdownContent);
          setOriginalContent(markdownContent);
          setYamlContent(rawYaml);
          setOriginalYamlContent(rawYaml);
        }
      } else {
        if (!isUnmountedRef.current) {
          setPlanData(null);
          setPlanContent(content);
          setOriginalContent(content);
        }
      }
    } catch (err) {
      if (!isUnmountedRef.current) {
        const errStr = String(err);
        log.error('Failed to load file', err);
        // Simplify error message
        let displayError = t('editor.common.loadFailed');
        if (errStr.includes('does not exist') || errStr.includes('No such file')) {
          displayError = t('editor.common.fileNotFound');
        } else if (errStr.includes('Permission denied') || errStr.includes('permission')) {
          displayError = t('editor.common.permissionDenied');
        }
        setError(displayError);
      }
    } finally {
      if (!isUnmountedRef.current) {
        setLoading(false);
      }
    }
  }, [filePath]);

  useEffect(() => {
    loadFileContent();
  }, [loadFileContent]);

  useEffect(() => {
    if (!filePath) return;

    const normalizedPlanPath = filePath.replace(/\\/g, '/');
    const dirPath = filePath.substring(0, filePath.lastIndexOf('\\') >= 0 
      ? filePath.lastIndexOf('\\') 
      : filePath.lastIndexOf('/'));

    let debounceTimer: ReturnType<typeof setTimeout> | null = null;

    const unwatch = fileSystemService.watchFileChanges(dirPath, (event) => {
      const eventPath = event.path.replace(/\\/g, '/');
      if (eventPath !== normalizedPlanPath) return;
      if (event.type !== 'modified') return;

      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }
      debounceTimer = setTimeout(() => {
        loadFileContent();
      }, 300);
    });

    return () => {
      unwatch();
      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }
    };
  }, [filePath, loadFileContent]);

  useEffect(() => {
    if (!planData?.todos?.length || !filePath) return;

    const handleTodoWriteUpdate = async (event: Event) => {
      const customEvent = event as CustomEvent<{
        todos: Array<{ id: string; content: string; status: string }>;
      }>;
      const { todos: incomingTodos } = customEvent.detail;

      if (!incomingTodos.length) return;

      // Check if there are matching todos
      const todoIds = new Set(planData.todos.map(t => t.id));
      const matchedTodos = incomingTodos.filter(t => todoIds.has(t.id));
      
      if (matchedTodos.length === 0) return;

      try {
        // Read current file content
        const content = await workspaceAPI.readFileContent(filePath);
        const frontmatterMatch = content.match(/^---\n([\s\S]*?)\n---/);
        
        if (!frontmatterMatch) return;

        const parsed = yaml.parse(frontmatterMatch[1]);
        const currentPlanContent = content.replace(/^---\n[\s\S]*?\n---\n*/, '').trim();

        // Update todos status
        const updatedTodos = (parsed.todos || []).map((todo: PlanTodo) => {
          const incomingTodo = incomingTodos.find(t => t.id === todo.id);
          if (incomingTodo) {
            return { ...todo, status: incomingTodo.status };
          }
          return todo;
        });

        // Rebuild frontmatter and write back to file
        const updatedParsed = {
          ...parsed,
          todos: updatedTodos,
        };
        const updatedFrontmatter = yaml.stringify(updatedParsed);
        const updatedContent = `---\n${updatedFrontmatter}---\n\n${currentPlanContent}`;

        markFileWriting(filePath);
        await workspaceAPI.writeFileContent('', filePath, updatedContent);

        // Update component state
        setPlanData({
          name: parsed.name || '',
          overview: parsed.overview || '',
          todos: updatedTodos,
        });
        
        // Sync update yamlContent (ensure edit mode shows latest content)
        const newYamlContent = updatedFrontmatter.trim();
        setYamlContent(newYamlContent);
        setOriginalYamlContent(newYamlContent);

        // Check if all todos are completed
        const allCompleted = updatedTodos.every((t: PlanTodo) => t.status === 'completed');
        if (allCompleted) {
          setIsBuildStarted(false);
        }
      } catch (err) {
        log.error('Failed to sync todo status', err);
      }
    };

    window.addEventListener('bitfun:todowrite-update', handleTodoWriteUpdate);
    return () => {
      window.removeEventListener('bitfun:todowrite-update', handleTodoWriteUpdate);
    };
  }, [planData, filePath]);

  useEffect(() => {
    if (!isBuildStarted) return;

    const handleDialogCancelled = () => {
      setIsBuildStarted(false);
    };

    window.addEventListener('bitfun:dialog-cancelled', handleDialogCancelled);
    return () => {
      window.removeEventListener('bitfun:dialog-cancelled', handleDialogCancelled);
    };
  }, [isBuildStarted]);

  const remainingTodos = useMemo(() => {
    if (!planData?.todos) return 0;
    return planData.todos.filter(t => t.status !== 'completed').length;
  }, [planData]);

  const buildStatus = useMemo((): 'build' | 'building' | 'built' => {
    if (planData?.todos?.length) {
      const statuses = planData.todos.map(t => t.status);
      if (statuses.every(s => s === 'completed')) {
        return 'built';
      }
    }
    if (isBuildStarted) {
      return 'building';
    }
    return 'build';
  }, [planData, isBuildStarted]);

  useEffect(() => {
    if (buildStatus === 'built' && isBuildStarted) {
      setIsBuildStarted(false);
    }
  }, [buildStatus, isBuildStarted]);

  const hasUnsavedChanges = useMemo(() => {
    const yamlChanged = yamlContent !== originalYamlContent;
    const contentChanged = planContent !== originalContent;
    return yamlChanged || contentChanged;
  }, [yamlContent, originalYamlContent, planContent, originalContent]);

  const saveFileContent = useCallback(async () => {
    if (!hasUnsavedChanges || !filePath) return;

    try {
      // Rebuild full content
      let fullContent = '';
      if (yamlContent) {
        fullContent = `---\n${yamlContent}\n---\n\n${planContent}`;
      } else {
        fullContent = planContent;
      }

      await workspaceAPI.writeFileContent(workspacePath || '', filePath, fullContent);
      setOriginalContent(planContent);
      setOriginalYamlContent(yamlContent);
      
      // Re-parse yaml to update planData
      if (yamlContent) {
        try {
          const parsed = yaml.parse(yamlContent);
          setPlanData({
            name: parsed.name || '',
            overview: parsed.overview || '',
            todos: parsed.todos || [],
          });
        } catch (e) {
          log.warn('YAML parse failed', e);
        }
      }
    } catch (err) {
      log.error('Failed to save file', err);
    }
  }, [planContent, yamlContent, filePath, workspacePath, hasUnsavedChanges]);

  const handleContentChange = useCallback((newContent: string) => {
    setPlanContent(newContent);
  }, []);

  const handleYamlChange = useCallback((newContent: string) => {
    setYamlContent(newContent);
  }, []);

  const handleSave = useCallback((_value: string) => {
    saveFileContent();
  }, [saveFileContent]);

  const toggleEditMode = useCallback(() => {
    if (isEditingYaml) {
      try {
        const parsed = yaml.parse(yamlContent);
        setPlanData({
          name: parsed.name || '',
          overview: parsed.overview || '',
          todos: parsed.todos || [],
        });
      } catch (e) {
        log.warn('YAML parse failed', e);
      }
    }
    setIsEditingYaml(!isEditingYaml);
  }, [isEditingYaml, yamlContent]);

  // Build button click handler
  const handleBuild = useCallback(async () => {
    if (!filePath || buildStatus !== 'build' || !planData) return;

    try {
      setIsBuildStarted(true);

      // Process todos, keep only id, content, and status
      const simpleTodos = planData.todos.map(t => ({
        id: t.id,
        content: t.content,
        status: t.status,
      }));

      const message = `Implement the plan as specified, it is attached for your reference. Do NOT edit the plan file itself. To-do's from the plan have already been created. Do not create them again. Mark them as in_progress as you work, starting with the first one. Don't stop until you have completed all the to-dos.

<attached_file path="${filePath}">
<plan>
${planContent}
</plan>
<todos>
${JSON.stringify(simpleTodos, null, 2)}
</todos>
</attached_file>`;

      const displayMessage = t('editor.planViewer.buildPlanTitle', { name: planData.name });
      await flowChatManager.sendMessage(message, undefined, displayMessage, 'agentic', 'agentic');
    } catch (err) {
      log.error('Build failed', err);
      setIsBuildStarted(false);
    }
  }, [filePath, buildStatus, planData, planContent, t]);

  // Get todo status icon
  const getTodoIcon = (status?: string) => {
    switch (status) {
      case 'completed':
        return <Check size={14} className="todo-icon todo-icon--completed" />;
      case 'in_progress':
        return <ArrowRight size={14} className="todo-icon todo-icon--in-progress" />;
      case 'cancelled':
        return <XCircle size={14} className="todo-icon todo-icon--cancelled" />;
      case 'pending':
      default:
        return <Circle size={14} className="todo-icon todo-icon--pending" />;
    }
  };

  // Render loading state
  if (loading) {
    return (
      <div className="bitfun-plan-viewer bitfun-plan-viewer--loading">
        <CubeLoading size="medium" text={t('editor.planViewer.loadingPlan')} />
      </div>
    );
  }

  // Render error state
  if (error) {
    return (
      <div className="bitfun-plan-viewer bitfun-plan-viewer--error">
        <div className="error-content">
          <AlertCircle className="error-icon" />
          <p>{error}</p>
          <Button variant="secondary" size="small" onClick={loadFileContent}>
            {t('editor.common.retry')}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="bitfun-plan-viewer">
      <div className="plan-viewer-header">
        <div className="header-left">
          <FileText size={16} className="file-icon" />
          <span className="file-name">{displayFileName}</span>
          {hasUnsavedChanges && <span className="unsaved-indicator">{t('editor.planViewer.unsaved')}</span>}
        </div>
        <div className="header-right">
          {planData && planData.todos && planData.todos.length > 0 && (
            <button
              className={`build-btn build-btn--${buildStatus}`}
              onClick={handleBuild}
              disabled={buildStatus !== 'build'}
            >
              {buildStatus === 'building' ? (
                <>
                  <Loader2 size={14} className="animate-spin" />
                  <span>{t('editor.planViewer.building')}</span>
                </>
              ) : buildStatus === 'built' ? (
                <>
                  <CheckCircle size={14} />
                  <span>{t('editor.planViewer.built')}</span>
                </>
              ) : (
                <span>{t('editor.planViewer.build')}</span>
              )}
            </button>
          )}
        </div>
      </div>

      {planData && planData.todos && planData.todos.length > 0 && (
        <div className={`plan-viewer-todos ${isTodosExpanded ? 'plan-viewer-todos--expanded' : ''}`}>
          <div 
            className="todos-header"
            onClick={() => !isEditingYaml && setIsTodosExpanded(!isTodosExpanded)}
          >
            <div className="todos-header-left">
              <button 
                className="todos-toggle-btn" 
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setIsTodosExpanded(!isTodosExpanded);
                }}
              >
                {isTodosExpanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
              </button>
              <span className="todos-count">{t('editor.planViewer.remainingTodos', { count: remainingTodos })}</span>
            </div>
            <button
              className={`edit-btn ${isEditingYaml ? 'edit-btn--active' : ''}`}
              onClick={(e) => {
                e.stopPropagation();
                toggleEditMode();
              }}
              title={isEditingYaml ? t('editor.planViewer.toggleYamlEditOff') : t('editor.planViewer.toggleYamlEditOn')}
            >
              {isEditingYaml ? <X size={14} /> : <Pencil size={14} />}
            </button>
          </div>
          
          {isEditingYaml && (
            <div className="yaml-editor-section">
              <div className="yaml-editor-content">
                <MEditor
                  ref={yamlEditorRef}
                  value={yamlContent}
                  onChange={handleYamlChange}
                  onSave={handleSave}
                  mode="edit"
                  theme="dark"
                  height="200px"
                  width="100%"
                  placeholder={t('editor.planViewer.yamlPlaceholder')}
                  readonly={false}
                  toolbar={false}
                  autofocus={true}
                />
              </div>
            </div>
          )}
          
          {!isEditingYaml && isTodosExpanded && (
            <div className="todos-list">
              {planData.todos.map((todo, index) => (
                <div
                  key={todo.id || index}
                  className={`todo-item status-${todo.status || 'pending'}`}
                >
                  {getTodoIcon(todo.status)}
                  <span className="todo-content">{todo.content}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      <div className="plan-viewer-content">
        <div className="plan-markdown">
          <MEditor
            ref={editorRef}
            value={planContent}
            onChange={handleContentChange}
            onSave={handleSave}
            mode="ir"
            theme="dark"
            height="100%"
            width="100%"
            placeholder={t('editor.planViewer.contentPlaceholder')}
            readonly={false}
            toolbar={false}
            basePath={basePath}
          />
        </div>
      </div>
    </div>
  );
};

export default PlanViewer;

