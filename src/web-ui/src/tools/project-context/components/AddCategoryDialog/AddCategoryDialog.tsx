/**
 * Add category dialog component.
 */

import React, { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { 
  X, 
  FolderPlus,
  FileText,
  Code,
  Boxes,
  GitPullRequest,
  FolderOpen,
  Bookmark,
  Database,
  Settings,
  Shield,
  Zap
} from 'lucide-react';
import { Input, Modal, Button, IconButton } from '@/component-library';
import './AddCategoryDialog.scss';

// Available icon list (labels are provided via i18n).
const AVAILABLE_ICONS = [
  { id: 'FolderOpen', icon: FolderOpen, labelKey: 'folder' },
  { id: 'FileText', icon: FileText, labelKey: 'document' },
  { id: 'Code', icon: Code, labelKey: 'code' },
  { id: 'Boxes', icon: Boxes, labelKey: 'module' },
  { id: 'GitPullRequest', icon: GitPullRequest, labelKey: 'review' },
  { id: 'Bookmark', icon: Bookmark, labelKey: 'bookmark' },
  { id: 'Database', icon: Database, labelKey: 'data' },
  { id: 'Settings', icon: Settings, labelKey: 'config' },
  { id: 'Shield', icon: Shield, labelKey: 'security' },
  { id: 'Zap', icon: Zap, labelKey: 'tool' },
];

interface AddCategoryDialogProps {
  /** Whether the dialog is open. */
  isOpen: boolean;
  /** Close callback. */
  onClose: () => void;
  /** Confirm callback. */
  onConfirm: (name: string, icon: string, description?: string) => void;
}

/**
 * Add category dialog component.
 */
export const AddCategoryDialog: React.FC<AddCategoryDialogProps> = ({
  isOpen,
  onClose,
  onConfirm
}) => {
  const { t } = useTranslation('panels/project-context');
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [selectedIcon, setSelectedIcon] = useState('FolderOpen');
  const [error, setError] = useState('');

  const resetForm = useCallback(() => {
    setName('');
    setDescription('');
    setSelectedIcon('FolderOpen');
    setError('');
  }, []);

  const handleClose = useCallback(() => {
    resetForm();
    onClose();
  }, [resetForm, onClose]);

  const handleConfirm = useCallback(() => {
    const trimmedName = name.trim();
    const trimmedDescription = description.trim();

    if (!trimmedName) {
      setError(t('dialog.addCategory.error.nameRequired'));
      return;
    }

    if (trimmedName.length > 20) {
      setError(t('dialog.addCategory.error.nameTooLong'));
      return;
    }

    onConfirm(trimmedName, selectedIcon, trimmedDescription || undefined);
    handleClose();
  }, [name, description, selectedIcon, onConfirm, handleClose, t]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleConfirm();
    } else if (e.key === 'Escape') {
      handleClose();
    }
  }, [handleConfirm, handleClose]);

  return (
    <Modal isOpen={isOpen} onClose={handleClose} title={t('dialog.addCategory.title')} size="small">
      <div className="bitfun-add-category-dialog__content">
        <div className="bitfun-add-category-dialog__header">
          <div className="bitfun-add-category-dialog__icon-wrapper">
            <FolderPlus size={24} />
          </div>
          <p className="bitfun-add-category-dialog__subtitle">{t('dialog.addCategory.subtitle')}</p>
        </div>

        <div className="bitfun-add-category-dialog__form">
          <div className="bitfun-add-category-dialog__field">
            <Input
              label={t('dialog.addCategory.nameLabel')}
              inputSize="small"
              placeholder={t('dialog.addCategory.namePlaceholder')}
              value={name}
              onChange={(e) => {
                setName(e.target.value);
                setError('');
              }}
              onKeyDown={handleKeyDown}
              autoFocus
              maxLength={20}
              error={!!error}
              errorMessage={error}
            />
          </div>

          <div className="bitfun-add-category-dialog__field">
            <label className="bitfun-add-category-dialog__label">{t('dialog.addCategory.iconLabel')}</label>
            <div className="bitfun-add-category-dialog__icons">
              {AVAILABLE_ICONS.map(({ id, icon: Icon, labelKey }) => (
                <IconButton
                  key={id}
                  variant={selectedIcon === id ? 'primary' : 'ghost'}
                  size="small"
                  onClick={() => setSelectedIcon(id)}
                  tooltip={t(`dialog.addCategory.icons.${labelKey}`)}
                >
                  <Icon size={16} />
                </IconButton>
              ))}
            </div>
          </div>

          <div className="bitfun-add-category-dialog__field">
            <Input
              label={t('dialog.addCategory.descriptionLabel')}
              inputSize="small"
              placeholder={t('dialog.addCategory.descriptionPlaceholder')}
              value={description}
              onChange={(e) => {
                setDescription(e.target.value);
              }}
              onKeyDown={handleKeyDown}
              maxLength={100}
            />
          </div>
        </div>

        <div className="bitfun-add-category-dialog__footer">
          <Button variant="secondary" onClick={handleClose}>
            {t('dialog.addCategory.cancel')}
          </Button>
          <Button variant="primary" onClick={handleConfirm}>
            {t('dialog.addCategory.confirm')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default AddCategoryDialog;
