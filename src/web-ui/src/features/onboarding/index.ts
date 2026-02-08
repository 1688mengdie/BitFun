/**
 * First-launch onboarding module.
 */

// Components
export { OnboardingWizard } from './components';
export * from './components/steps';

// Hooks
export { useOnboarding } from './hooks/useOnboarding';

// Store
export { 
  useOnboardingStore,
  STEP_ORDER,
  type OnboardingStep,
  type OnboardingModelConfig
} from './store/onboardingStore';

// Services
export { onboardingService } from './services/OnboardingService';
