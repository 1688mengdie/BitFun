import React, { useCallback, useEffect, useImperativeHandle, forwardRef, useRef } from 'react'
import { createLogger } from '@/shared/utils/logger'
import { useEditor } from '../hooks/useEditor'
import { EditArea } from './EditArea'
import { IREditor, IREditorHandle } from './IREditor'
import { Preview } from './Preview'
import type { EditorOptions, EditorInstance } from '../types'
import { useI18n } from '@/infrastructure/i18n'
import './MEditor.scss'

void createLogger('MEditor')

export interface MEditorProps extends EditorOptions {}

export const MEditor = forwardRef<EditorInstance, MEditorProps>((props, ref) => {
  const {
    value: controlledValue,
    defaultValue = '',
    height = '500px',
    width = '100%',
    mode: initialMode = 'ir',
    theme: initialTheme = 'dark',
    toolbar = false,
    placeholder: placeholderProp,
    readonly = false,
    autofocus = false,
    onChange,
    onSave,
    onFocus,
    onBlur,
    onDirtyChange,
    className = '',
    style = {},
    basePath
  } = props

  const { t } = useI18n('tools')
  const placeholder = placeholderProp ?? t('editor.meditor.placeholder')

  const {
    value,
    setValue,
    mode,
    setMode,
    theme,
    setTheme,
    textareaRef,
    editorInstance
  } = useEditor(controlledValue ?? defaultValue, onChange)
  
  const irEditorRef = useRef<IREditorHandle>(null)

  useEffect(() => {
    if (controlledValue !== undefined && controlledValue !== value) {
      setValue(controlledValue)
    }
  }, [controlledValue, value, setValue])

  useEffect(() => {
    if (initialMode) {
      setMode(initialMode)
    }
  }, [initialMode, setMode])

  useEffect(() => {
    if (initialTheme) {
      setTheme(initialTheme)
    }
  }, [initialTheme, setTheme])

  useImperativeHandle(ref, () => ({
    ...editorInstance,
    scrollToLine: (line: number, highlight?: boolean) => {
      if (mode === 'ir' && irEditorRef.current) {
        irEditorRef.current.scrollToLine(line, highlight)
      }
    },
    undo: () => {
      if (mode === 'ir' && irEditorRef.current) {
        return irEditorRef.current.undo()
      }
      return false
    },
    redo: () => {
      if (mode === 'ir' && irEditorRef.current) {
        return irEditorRef.current.redo()
      }
      return false
    },
    get canUndo() {
      if (mode === 'ir' && irEditorRef.current) {
        return irEditorRef.current.canUndo
      }
      return false
    },
    get canRedo() {
      if (mode === 'ir' && irEditorRef.current) {
        return irEditorRef.current.canRedo
      }
      return false
    },
    markSaved: () => {
      if (mode === 'ir' && irEditorRef.current) {
        irEditorRef.current.markSaved()
      }
    },
    setInitialContent: (content: string) => {
      if (mode === 'ir' && irEditorRef.current) {
        irEditorRef.current.setInitialContent(content)
      }
    },
    get isDirty() {
      if (mode === 'ir' && irEditorRef.current) {
        return irEditorRef.current.isDirty
      }
      return false
    }
  }), [editorInstance, mode])

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLDivElement>) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 's') {
      e.preventDefault()
      e.stopPropagation()  // Prevent event bubbling; avoids other listeners handling it.
      onSave?.(value)
    }
  }, [value, onSave])

  const containerStyle: React.CSSProperties = {
    ...style,
    height: typeof height === 'number' ? `${height}px` : height,
    width: typeof width === 'number' ? `${width}px` : width
  }

  const themeClass = theme === 'dark' ? 'm-editor-dark' : 'm-editor-light'
  const modeClass = `m-editor-mode-${mode}`

  return (
    <div
      className={`m-editor ${themeClass} ${modeClass} ${className}`}
      style={containerStyle}
      onKeyDown={handleKeyDown}
      tabIndex={-1}
    >
      {toolbar && <div className="m-editor-toolbar">{t('editor.meditor.toolbarPlaceholder')}</div>}
      
      <div className="m-editor-content">
        {mode === 'preview' && (
          <Preview value={value} basePath={basePath} />
        )}

        {mode === 'edit' && (
          <div className="m-editor-edit-panel">
            <EditArea
              ref={textareaRef}
              value={value}
              onChange={setValue}
              onFocus={onFocus}
              onBlur={onBlur}
              placeholder={placeholder}
              readonly={readonly}
              autofocus={autofocus}
            />
          </div>
        )}

        {mode === 'split' && (
          <>
            <div className="m-editor-edit-panel">
              <EditArea
                ref={textareaRef}
                value={value}
                onChange={setValue}
                onFocus={onFocus}
                onBlur={onBlur}
                placeholder={placeholder}
                readonly={readonly}
                autofocus={autofocus}
              />
            </div>
            <div className="m-editor-preview-panel">
              <Preview value={value} basePath={basePath} />
            </div>
          </>
        )}

        {mode === 'ir' && (
          <div className="m-editor-ir-panel">
            <IREditor
              ref={irEditorRef}
              value={value}
              onChange={setValue}
              onFocus={onFocus}
              onBlur={onBlur}
              onDirtyChange={onDirtyChange}
              placeholder={placeholder}
              readonly={readonly}
              autofocus={autofocus}
              basePath={basePath}
            />
          </div>
        )}
      </div>
    </div>
  )
})

MEditor.displayName = 'MEditor'

