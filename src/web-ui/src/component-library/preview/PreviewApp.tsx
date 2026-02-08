/**
 * Component preview app
 */

import React, { useState } from 'react';
import { componentRegistry } from '../components/registry';
import type { ComponentCategory } from '../types';
import { FullPageLayout, LargeCardLayout, GridLayout, DemoLayout, ColumnLayout } from './layouts';
import { useI18n } from '@/infrastructure/i18n';
import './preview.css';

export const PreviewApp: React.FC = () => {
  const { t } = useI18n('components');
  const [selectedCategory, setSelectedCategory] = useState<string>(
    componentRegistry[0]?.id || ''
  );

  const handleCategorySelect = (categoryId: string) => {
    setSelectedCategory(categoryId);
  };

  const currentCategory = componentRegistry.find(
    (cat) => cat.id === selectedCategory
  );

  return (
    <div className="preview-app">
      <header className="preview-header">
        <div className="preview-logo">
          <h1>{t('componentLibrary.previewApp.title')}</h1>
          <span className="preview-version">v0.1.0</span>
        </div>
      </header>

      <div className="preview-container">
        <aside className="preview-sidebar">
          <nav className="preview-nav">
            {componentRegistry.map((category: ComponentCategory) => (
              <div key={category.id} className="category-section">
                <button
                  className={`category-button ${
                    selectedCategory === category.id ? 'active' : ''
                  }`}
                  onClick={() => handleCategorySelect(category.id)}
                >
                  <span className="category-name">{category.name}</span>
                  <span className="component-count">
                    {category.components.length}
                  </span>
                </button>
              </div>
            ))}
          </nav>
        </aside>

        <main className={`preview-main ${currentCategory?.layoutType === 'full-page' ? 'preview-main--full' : ''}`}>
          {currentCategory ? (
            <>
              {currentCategory.layoutType !== 'full-page' && (
                <div className="component-header">
                  <h2 className="component-title">{currentCategory.name}</h2>
                  <p className="component-description">
                    {currentCategory.description}
                  </p>
                </div>
              )}

              {currentCategory.layoutType === 'full-page' ? (
                <FullPageLayout components={currentCategory.components} />
              ) : currentCategory.layoutType === 'large-card' ? (
                <LargeCardLayout components={currentCategory.components} />
              ) : currentCategory.layoutType === 'demo' ? (
                <DemoLayout components={currentCategory.components} />
              ) : currentCategory.layoutType === 'column' ? (
                <ColumnLayout components={currentCategory.components} />
              ) : currentCategory.layoutType === 'grid-2' ? (
                <GridLayout components={currentCategory.components} columns={2} />
              ) : currentCategory.layoutType === 'grid-4' ? (
                <GridLayout components={currentCategory.components} columns={4} />
              ) : (
                <GridLayout components={currentCategory.components} columns={3} />
              )}
            </>
          ) : (
            <div className="empty-state">
              <p>{t('componentLibrary.previewApp.emptyState')}</p>
            </div>
          )}
        </main>
      </div>
    </div>
  );
};