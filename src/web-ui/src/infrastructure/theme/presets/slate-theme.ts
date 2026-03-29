 

import { ThemeConfig } from '../types';

export const bitfunSlateTheme: ThemeConfig = {
  
  id: 'bitfun-slate',
  name: 'Slate',
  type: 'dark',
  description: 'Slate gray geometric theme - Deep immersion, high contrast grayscale aesthetics',
  author: 'BitFun Team',
  version: '1.2.7',
  
  
  colors: {
    background: {
      primary: '#14161a',
      secondary: '#22262c',
      tertiary: '#14161a',
      quaternary: '#2c3038',
      elevated: '#22262c',
      workbench: '#14161a',
      scene: '#22262c',
      tooltip: 'rgba(34, 38, 44, 0.96)',
    },
    
    text: {
      primary: '#eef0f3',
      secondary: '#c8ccd2',
      muted: '#9ea4ab',
      disabled: '#65696f',
    },
    
    
    // Antique gold accent: warm, restrained, reads well on charcoal slate (nav wells, CTAs).
    accent: {
      50: 'rgba(209, 186, 122, 0.06)',
      100: 'rgba(209, 186, 122, 0.11)',
      200: 'rgba(209, 186, 122, 0.20)',
      300: 'rgba(209, 186, 122, 0.34)',
      400: 'rgba(209, 186, 122, 0.50)',
      500: '#cfba7a',
      600: '#b89d52',
      700: 'rgba(184, 157, 82, 0.88)',
      800: 'rgba(184, 157, 82, 0.94)',
    },
    
    
    purple: {
      50: 'rgba(184, 198, 255, 0.04)',
      100: 'rgba(184, 198, 255, 0.08)',
      200: 'rgba(184, 198, 255, 0.15)',
      300: 'rgba(184, 198, 255, 0.25)',
      400: 'rgba(184, 198, 255, 0.4)',
      500: '#b8c4ff',
      600: '#9dacf5',
      700: 'rgba(157, 172, 245, 0.8)',
      800: 'rgba(157, 172, 245, 0.9)',
    },
    
    semantic: {
      success: '#7fb899',       
      successBg: 'rgba(127, 184, 153, 0.1)',
      successBorder: 'rgba(127, 184, 153, 0.3)',
      
      warning: '#c4916a',
      warningBg: 'rgba(196, 145, 106, 0.12)',
      warningBorder: 'rgba(196, 145, 106, 0.32)',
      
      error: '#c9878d',         
      errorBg: 'rgba(201, 135, 141, 0.1)',
      errorBorder: 'rgba(201, 135, 141, 0.3)',
      
      info: '#cfba7a',
      infoBg: 'rgba(209, 186, 122, 0.12)',
      infoBorder: 'rgba(209, 186, 122, 0.34)',
      
      
      highlight: '#e2e4e7',
      highlightBg: 'rgba(212, 214, 216, 0.12)',
    },
    
    border: {
      subtle: 'rgba(255, 255, 255, 0.12)',    
      base: 'rgba(255, 255, 255, 0.18)',      
      medium: 'rgba(255, 255, 255, 0.24)',    
      strong: 'rgba(255, 255, 255, 0.32)',    
      prominent: 'rgba(255, 255, 255, 0.45)',  
    },
    
    element: {
      subtle: 'rgba(255, 255, 255, 0.05)',
      soft: 'rgba(255, 255, 255, 0.07)',
      base: 'rgba(255, 255, 255, 0.095)',
      medium: 'rgba(255, 255, 255, 0.125)',
      strong: 'rgba(255, 255, 255, 0.155)',
      elevated: 'rgba(255, 255, 255, 0.19)',
    },
    
    git: {
      branch: '#cfba7a',
      branchBg: 'rgba(209, 186, 122, 0.12)',
      changes: 'rgb(212, 165, 116)',
      changesBg: 'rgba(212, 165, 116, 0.1)',
      added: 'rgb(127, 184, 153)',
      addedBg: 'rgba(127, 184, 153, 0.1)',
      deleted: 'rgb(201, 135, 141)',
      deletedBg: 'rgba(201, 135, 141, 0.1)',
      staged: 'rgb(127, 184, 153)',
      stagedBg: 'rgba(127, 184, 153, 0.1)',
    },
    
    scrollbar: {
      thumb: 'rgba(200, 188, 168, 0.24)',
      thumbHover: 'rgba(212, 198, 172, 0.38)',
    },
  },
  
  
  effects: {
    shadow: {
      xs: '0 1px 2px rgba(0, 0, 0, 0.85)',
      sm: '0 2px 4px rgba(0, 0, 0, 0.8)',
      base: '0 4px 8px rgba(0, 0, 0, 0.75)',
      lg: '0 8px 16px rgba(0, 0, 0, 0.7)',
      xl: '0 12px 24px rgba(0, 0, 0, 0.85)',
      '2xl': '0 16px 32px rgba(0, 0, 0, 0.9)',
    },
    
    glow: {
      blue: '0 12px 32px rgba(209, 186, 122, 0.22), 0 6px 16px rgba(209, 186, 122, 0.14), 0 3px 8px rgba(0, 0, 0, 0.2)',
      purple: '0 12px 32px rgba(184, 198, 255, 0.22), 0 6px 16px rgba(184, 198, 255, 0.15), 0 3px 8px rgba(0, 0, 0, 0.2)',
      mixed: '0 12px 32px rgba(209, 186, 122, 0.18), 0 6px 16px rgba(184, 198, 255, 0.14), 0 3px 8px rgba(0, 0, 0, 0.2)',
    },
    
    blur: {
      subtle: 'blur(4px) saturate(1.05) brightness(0.98)',
      base: 'blur(8px) saturate(1.08) brightness(0.98)',
      medium: 'blur(12px) saturate(1.12) brightness(0.97)',
      strong: 'blur(16px) saturate(1.15) brightness(0.97)',
      intense: 'blur(20px) saturate(1.18) brightness(0.96)',
    },
    
    radius: {
      sm: '4px',
      base: '6px',
      lg: '8px',
      xl: '12px',
      '2xl': '16px',
      full: '9999px',
    },
    
    spacing: {
      1: '4px',
      2: '8px',
      3: '12px',
      4: '16px',
      5: '20px',
      6: '24px',
      8: '32px',
      10: '40px',
      12: '48px',
      16: '64px',
    },
    
    opacity: {
      disabled: 0.5,
      hover: 0.75,
      focus: 0.85,
      overlay: 0.5,
    },
  },
  
  
  motion: {
    duration: {
      instant: '0.08s',
      fast: '0.12s',
      base: '0.25s',
      slow: '0.5s',
      lazy: '0.8s',
    },
    
    easing: {
      standard: 'cubic-bezier(0.4, 0, 0.2, 1)',
      decelerate: 'cubic-bezier(0, 0, 0.2, 1)',
      accelerate: 'cubic-bezier(0.4, 0, 1, 1)',
      bounce: 'cubic-bezier(0.68, -0.55, 0.265, 1.55)',
      smooth: 'cubic-bezier(0.4, 0, 0.2, 1)',
    },
  },
  
  
  typography: {
    font: {
      sans: "'Noto Sans SC', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'SF Pro Display', Roboto, sans-serif",
      mono: "'FiraCode', 'JetBrains Mono', 'SF Mono', 'Consolas', 'Liberation Mono', monospace",
    },
    
    weight: {
      normal: 400,
      medium: 500,
      semibold: 600,
      bold: 700,
    },
    
    size: {
      xs: '12px',
      sm: '14px',
      base: '15px',
      lg: '16px',
      xl: '18px',
      '2xl': '20px',
      '3xl': '24px',
      '4xl': '30px',
      '5xl': '36px',
    },
    
    lineHeight: {
      tight: 1.2,
      base: 1.5,
      relaxed: 1.6,
    },
  },
  
  
  components: {
    
    windowControls: {
      minimize: {
        dot: 'rgba(209, 186, 122, 0.55)',
        dotShadow: '0 0 4px rgba(209, 186, 122, 0.26)',
        hoverBg: 'rgba(209, 186, 122, 0.18)',
        hoverColor: '#e8d9a8',
        hoverBorder: 'rgba(209, 186, 122, 0.32)',
        hoverShadow: '0 2px 8px rgba(209, 186, 122, 0.20), inset 0 1px 0 rgba(255, 255, 255, 0.08)',
      },
      maximize: {
        dot: 'rgba(209, 186, 122, 0.55)',
        dotShadow: '0 0 4px rgba(209, 186, 122, 0.26)',
        hoverBg: 'rgba(209, 186, 122, 0.18)',
        hoverColor: '#e8d9a8',
        hoverBorder: 'rgba(209, 186, 122, 0.32)',
        hoverShadow: '0 2px 8px rgba(209, 186, 122, 0.20), inset 0 1px 0 rgba(255, 255, 255, 0.08)',
      },
      close: {
        dot: 'rgba(201, 135, 141, 0.5)',
        dotShadow: '0 0 4px rgba(201, 135, 141, 0.25)',
        hoverBg: 'rgba(201, 135, 141, 0.15)',
        hoverColor: '#c9878d',
        hoverBorder: 'rgba(201, 135, 141, 0.25)',
        hoverShadow: '0 2px 8px rgba(201, 135, 141, 0.18), inset 0 1px 0 rgba(255, 255, 255, 0.08)',
      },
      common: {
        defaultColor: 'rgba(232, 234, 236, 0.92)',
        defaultDot: 'rgba(198, 202, 208, 0.48)',
        disabledDot: 'rgba(168, 171, 176, 0.2)',
        flowGradient: 'linear-gradient(90deg, transparent, rgba(200, 188, 168, 0.09), rgba(200, 188, 168, 0.15), rgba(200, 188, 168, 0.09), transparent)',
      },
    },
    
    button: {
      
      default: {
        background: 'rgba(200, 192, 176, 0.14)',
        color: '#c8c0b0',
        border: 'transparent',
        shadow: 'none',
      },
      hover: {
        background: 'rgba(210, 200, 180, 0.22)',
        color: '#ebe4d6',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      active: {
        background: 'rgba(205, 195, 175, 0.18)',
        color: '#ebe4d6',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      
      
      primary: {
        default: {
          background: 'rgba(209, 186, 122, 0.38)',
          color: '#f5e9c8',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(209, 186, 122, 0.50)',
          color: '#fff8e8',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(209, 186, 122, 0.42)',
          color: '#fff8e8',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
      
      
      ghost: {
        default: {
          background: 'transparent',
          color: '#c8c0b0',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(200, 192, 176, 0.16)',
          color: '#ebe4d6',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(200, 192, 176, 0.12)',
          color: '#ebe4d6',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
    },
  },
  
  
  monaco: {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '9ca2a9', fontStyle: 'italic' },
      { token: 'keyword', foreground: 'cfba7a' },
      { token: 'string', foreground: '8fc8a9' },
      { token: 'number', foreground: 'b5c4fc' },
      { token: 'type', foreground: 'e4b584' },
      { token: 'class', foreground: 'e4b584' },
      { token: 'function', foreground: 'e8d9a8' },
      { token: 'variable', foreground: 'c4c8ce' },
      { token: 'constant', foreground: 'b5c4fc' },
      { token: 'operator', foreground: 'cfba7a' },
      { token: 'tag', foreground: 'e4b584' },
      { token: 'attribute.name', foreground: 'c4c8ce' },
      { token: 'attribute.value', foreground: '8fc8a9' },
    ],
    colors: {
      background: '#1a1c1e',
      foreground: '#eef0f3',
      lineHighlight: '#22252a',
      selection: 'rgba(209, 186, 122, 0.30)',
      cursor: '#cfba7a',
    },
  },
};
