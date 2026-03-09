import { useCallback, useEffect, useMemo, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import { configAPI } from '@/infrastructure/api';
import type { SkillInfo, SkillLevel, SkillValidationResult } from '@/infrastructure/config/types';
import { useCurrentWorkspace } from '@/infrastructure/hooks/useWorkspace';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import type { InstalledFilter } from '../skillsSceneStore';

const log = createLogger('SkillsScene:useInstalledSkills');

interface UseInstalledSkillsOptions {
  searchQuery: string;
  activeFilter: InstalledFilter;
}

export function useInstalledSkills({ searchQuery, activeFilter }: UseInstalledSkillsOptions) {
  const { t } = useTranslation('scenes/skills');
  const notification = useNotification();
  const { workspacePath, hasWorkspace } = useCurrentWorkspace();

  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [formLevel, setFormLevel] = useState<SkillLevel>('user');
  const [formPath, setFormPath] = useState('');
  const [validationResult, setValidationResult] = useState<SkillValidationResult | null>(null);
  const [isValidating, setIsValidating] = useState(false);
  const [isAdding, setIsAdding] = useState(false);

  const loadSkills = useCallback(async (forceRefresh?: boolean) => {
    try {
      setLoading(true);
      setError(null);
      const list = await configAPI.getSkillConfigs(forceRefresh);
      setSkills(list);
    } catch (err) {
      log.error('Failed to load skills', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSkills();
  }, [loadSkills]);

  const validatePath = useCallback(async (path: string) => {
    if (!path.trim()) {
      setValidationResult(null);
      return;
    }
    try {
      setIsValidating(true);
      const result = await configAPI.validateSkillPath(path);
      setValidationResult(result);
    } catch (err) {
      setValidationResult({
        valid: false,
        error: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setIsValidating(false);
    }
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      validatePath(formPath);
    }, 300);
    return () => window.clearTimeout(timer);
  }, [formPath, validatePath]);

  const handleBrowse = useCallback(async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('form.path.label'),
      });
      if (selected) {
        setFormPath(selected as string);
      }
    } catch (err) {
      log.error('Failed to open file dialog', err);
    }
  }, [t]);

  const resetForm = useCallback(() => {
    setFormPath('');
    setFormLevel('user');
    setValidationResult(null);
  }, []);

  const handleAdd = useCallback(async () => {
    if (!validationResult?.valid || !formPath.trim()) {
      notification.warning(t('messages.invalidPath'));
      return false;
    }
    if (formLevel === 'project' && !hasWorkspace) {
      notification.warning(t('messages.noWorkspace'));
      return false;
    }
    try {
      setIsAdding(true);
      await configAPI.addSkill(formPath, formLevel);
      notification.success(t('messages.addSuccess', { name: validationResult.name }));
      resetForm();
      await loadSkills(true);
      return true;
    } catch (err) {
      notification.error(
        t('messages.addFailed', {
          error: err instanceof Error ? err.message : String(err),
        }),
      );
      return false;
    } finally {
      setIsAdding(false);
    }
  }, [formLevel, formPath, hasWorkspace, loadSkills, notification, resetForm, t, validationResult]);

  const handleDelete = useCallback(async (skill: SkillInfo) => {
    try {
      await configAPI.deleteSkill(skill.name);
      notification.success(t('messages.deleteSuccess', { name: skill.name }));
      await loadSkills(true);
      return true;
    } catch (err) {
      notification.error(
        t('messages.deleteFailed', {
          error: err instanceof Error ? err.message : String(err),
        }),
      );
      return false;
    }
  }, [loadSkills, notification, t]);

  const normalizedQuery = searchQuery.trim().toLowerCase();

  const filteredSkills = useMemo(() => {
    return skills.filter((skill) => {
      const matchesFilter = activeFilter === 'all' || skill.level === activeFilter;
      const matchesQuery = !normalizedQuery || [
        skill.name,
        skill.description,
        skill.path,
      ].some((field) => field?.toLowerCase().includes(normalizedQuery));
      return matchesFilter && matchesQuery;
    });
  }, [activeFilter, normalizedQuery, skills]);

  const counts = useMemo(() => ({
    all: skills.length,
    user: skills.filter((skill) => skill.level === 'user').length,
    project: skills.filter((skill) => skill.level === 'project').length,
  }), [skills]);

  return {
    skills,
    filteredSkills,
    counts,
    loading,
    error,
    loadSkills,
    handleDelete,
    formLevel,
    setFormLevel,
    formPath,
    setFormPath,
    validationResult,
    isValidating,
    isAdding,
    handleBrowse,
    handleAdd,
    resetForm,
    workspacePath,
    hasWorkspace,
  };
}
