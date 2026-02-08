/**
 * First-run onboarding wizard
 * OnboardingWizard - main container component
 */

import React, { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronLeft, ChevronRight, AlertTriangle } from 'lucide-react';
import { useOnboarding } from '../hooks/useOnboarding';
import { onboardingService } from '../services/OnboardingService';
import { STEP_ORDER, useOnboardingStore, isModelConfigComplete, type OnboardingStep } from '../store/onboardingStore';
import {
  LanguageStep,
  ThemeStep,
  ModelConfigStep,
  CompletionStep
} from './steps';
import { WindowControls } from '@/component-library';
import { useWindowControls } from '@/app/hooks/useWindowControls';
import { createLogger } from '@/shared/utils/logger';
import './OnboardingWizard.scss';

const log = createLogger('OnboardingWizard');

interface OnboardingWizardProps {
  /** Callback after completion */
  onComplete?: () => void;
}

/**
 * Progress step labels
 */
const STEP_LABELS: Record<OnboardingStep, string> = {
  language: 'progress.step1',
  theme: 'progress.step2',
  model: 'progress.step3',
  completion: 'progress.step4'
};

export const OnboardingWizard: React.FC<OnboardingWizardProps> = ({
  onComplete
}) => {
  const { t } = useTranslation('onboarding');
  const {
    currentStep,
    currentStepIndex,
    totalSteps,
    selectedLanguage,
    selectedTheme,
    nextStep,
    prevStep,
    goToStep,
    skipOnboarding,
    completeOnboarding,
    setLanguage,
    setTheme,
    canGoNext,
    canGoPrev,
    isStepCompleted
  } = useOnboarding(false, false); // Do not auto-initialize; controlled externally

  // Model config state from store (for validation on next step)
  const { modelConfig } = useOnboardingStore();

  // Window controls
  const { handleMinimize, handleMaximize, handleClose, isMaximized } = useWindowControls();
  const isMacOS =
    typeof window !== 'undefined' &&
    '__TAURI__' in window &&
    typeof navigator !== 'undefined' &&
    typeof navigator.platform === 'string' &&
    navigator.platform.toUpperCase().includes('MAC');

  // Handle completion
  const handleComplete = useCallback(async () => {
    try {
      await completeOnboarding();
      onComplete?.();
    } catch (error) {
      log.error('Failed to complete onboarding', { error });
    }
  }, [completeOnboarding, onComplete]);

  const handleSkip = useCallback(async () => {
    skipOnboarding();
    await onboardingService.markCompleted();
    onComplete?.();
  }, [skipOnboarding, onComplete]);

  // Inline confirmation state for incomplete model config
  const [showIncompleteWarning, setShowIncompleteWarning] = useState(false);

  // Handle next step with model config validation
  const handleNextStep = useCallback(() => {
    // On the model step, check if provider is selected but required fields are incomplete
    if (currentStep === 'model' && modelConfig?.provider && !isModelConfigComplete(modelConfig)) {
      setShowIncompleteWarning(true);
      return;
    }
    nextStep();
  }, [currentStep, modelConfig, nextStep]);

  // User confirms to continue without completing model config
  // Keep partial config in store so user can go back and resume editing
  // Incomplete config won't be saved on completion (guarded by isModelConfigComplete)
  const handleConfirmIncomplete = useCallback(() => {
    setShowIncompleteWarning(false);
    nextStep();
  }, [nextStep]);

  // User cancels and stays to configure
  const handleCancelIncomplete = useCallback(() => {
    setShowIncompleteWarning(false);
  }, []);

  // Handle step click
  const handleStepClick = useCallback((step: OnboardingStep) => {
    goToStep(step);
  }, [goToStep]);

  // Render progress indicator
  const renderProgress = () => {
    return (
      <div className="bitfun-onboarding__progress">
        {STEP_ORDER.map((step, index) => {
          const isActive = step === currentStep;
          const isCompleted = isStepCompleted(step);
          const isClickable = isCompleted && !isActive;
          const isLast = index === STEP_ORDER.length - 1;

          return (
            <React.Fragment key={step}>
              <div 
                className={`bitfun-onboarding__progress-step ${
                  isActive ? 'bitfun-onboarding__progress-step--active' : ''
                } ${
                  isClickable ? 'bitfun-onboarding__progress-step--clickable' : ''
                }`}
                onClick={isClickable ? () => handleStepClick(step) : undefined}
                role={isClickable ? 'button' : undefined}
                tabIndex={isClickable ? 0 : undefined}
                onKeyDown={isClickable ? (e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    handleStepClick(step);
                  }
                } : undefined}
              >
                <div 
                  className={`bitfun-onboarding__progress-dot ${
                    isActive ? 'bitfun-onboarding__progress-dot--active' : ''
                  } ${
                    isCompleted ? 'bitfun-onboarding__progress-dot--completed' : ''
                  }`}
                />
                <span 
                  className={`bitfun-onboarding__progress-label ${
                    isActive ? 'bitfun-onboarding__progress-label--active' : ''
                  } ${
                    isCompleted ? 'bitfun-onboarding__progress-label--completed' : ''
                  }`}
                >
                  {t(STEP_LABELS[step])}
                </span>
              </div>
              {!isLast && (
                <div 
                  className={`bitfun-onboarding__progress-line ${
                    isCompleted ? 'bitfun-onboarding__progress-line--completed' : ''
                  }`} 
                />
              )}
            </React.Fragment>
          );
        })}
      </div>
    );
  };

  // Render current step content
  const renderStepContent = () => {
    switch (currentStep) {
      case 'language':
        return (
          <LanguageStep
            selectedLanguage={selectedLanguage}
            onLanguageChange={setLanguage}
          />
        );
      
      case 'theme':
        return (
          <ThemeStep
            selectedTheme={selectedTheme}
            onThemeChange={setTheme}
          />
        );
      
      case 'model':
        return (
          <ModelConfigStep
            onSkipForNow={nextStep}
          />
        );
      
      case 'completion':
        return (
          <CompletionStep
            onComplete={handleComplete}
          />
        );
      
      default:
        return null;
    }
  };

  // Render navigation
  const renderNavigation = () => {
    if (currentStep === 'completion') {
      return (
        <div className="bitfun-onboarding__navigation">
          <div className="bitfun-onboarding__nav-info" />
          <div className="bitfun-onboarding__nav-buttons">
            <button
              className="bitfun-onboarding__nav-btn"
              onClick={prevStep}
            >
              <ChevronLeft size={16} />
              {t('navigation.prev')}
            </button>
          </div>
        </div>
      );
    }

    return (
      <div className="bitfun-onboarding__navigation">
        <div className="bitfun-onboarding__nav-info">
          {t('navigation.stepOf', { 
            current: currentStepIndex + 1, 
            total: totalSteps 
          })}
        </div>
        <div className="bitfun-onboarding__nav-buttons">
          <button
            className="bitfun-onboarding__nav-btn bitfun-onboarding__nav-btn--skip"
            onClick={handleSkip}
          >
            {t('navigation.skip')}
          </button>
          <button
            className="bitfun-onboarding__nav-btn"
            onClick={prevStep}
            disabled={!canGoPrev()}
          >
            <ChevronLeft size={16} />
            {t('navigation.prev')}
          </button>
          <button
            className="bitfun-onboarding__nav-btn bitfun-onboarding__nav-btn--primary"
            onClick={handleNextStep}
            disabled={!canGoNext()}
          >
            {t('navigation.next')}
            <ChevronRight size={16} />
          </button>
        </div>
      </div>
    );
  };

  return (
    <div className="bitfun-onboarding">
      {/* Background decoration */}
      <div className="bitfun-onboarding__background" />

      {/* Window controls (macOS uses native traffic lights) */}
      {!isMacOS && (
        <div className="bitfun-onboarding__window-controls">
          <WindowControls
            onMinimize={handleMinimize}
            onMaximize={handleMaximize}
            onClose={handleClose}
            isMaximized={isMaximized}
          />
        </div>
      )}

      {/* Progress indicator */}
      {renderProgress()}

      {/* Main content */}
      <div className="bitfun-onboarding__content">
        <div className="bitfun-onboarding__step-container">
          {renderStepContent()}
        </div>
      </div>

      {/* Navigation */}
      {renderNavigation()}

      {/* Inline confirm dialog for incomplete model config */}
      {showIncompleteWarning && (
        <div className="bitfun-onboarding__confirm-overlay">
          <div className="bitfun-onboarding__confirm-dialog">
            <div className="bitfun-onboarding__confirm-icon">
              <AlertTriangle size={32} />
            </div>
            <h3 className="bitfun-onboarding__confirm-title">
              {t('model.incompleteConfig.title')}
            </h3>
            <p className="bitfun-onboarding__confirm-message">
              {t('model.incompleteConfig.message')}
            </p>
            <div className="bitfun-onboarding__confirm-actions">
              <button
                className="bitfun-onboarding__confirm-btn"
                onClick={handleCancelIncomplete}
              >
                {t('model.incompleteConfig.stayAndConfigure')}
              </button>
              <button
                className="bitfun-onboarding__confirm-btn bitfun-onboarding__confirm-btn--primary"
                onClick={handleConfirmIncomplete}
              >
                {t('model.incompleteConfig.continueAnyway')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default OnboardingWizard;
