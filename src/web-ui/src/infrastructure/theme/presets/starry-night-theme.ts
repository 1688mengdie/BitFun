 

import { ThemeConfig } from '../types';

export const bitfunStarryNightTheme: ThemeConfig = {
  
  id: 'bitfun-starry-night',
  name: 'Starry Night',
  type: 'dark',
  description: 'Starry theme - Deep night sky, sparkling stars, feel the tranquility and mystery of the universe',
  author: 'BitFun Team',
  version: '1.0.0',
  
  
  colors: {
    background: {
      primary: '#0a0e17',        
      secondary: '#0d1117',      
      tertiary: '#12171f',       
      quaternary: '#161c27',     
      elevated: '#080b12',       
      workbench: '#0a0e17',      
      flowchat: '#0a0e17',       
      tooltip: 'rgba(10, 14, 23, 0.95)',  
    },
    
    text: {
      primary: '#e6edf3',        
      secondary: '#c9d1d9',      
      muted: '#8b949e',          
      disabled: '#484f58',       
    },
    
    accent: {
      50: 'rgba(107, 141, 214, 0.05)',
      100: 'rgba(107, 141, 214, 0.1)',
      200: 'rgba(107, 141, 214, 0.18)',
      300: 'rgba(107, 141, 214, 0.3)',
      400: 'rgba(107, 141, 214, 0.45)',
      500: '#6B8DD6',            
      600: '#5B7CC6',            
      700: 'rgba(91, 124, 198, 0.85)',
      800: 'rgba(91, 124, 198, 0.95)',
    },
    
    purple: {
      50: 'rgba(177, 156, 217, 0.05)',
      100: 'rgba(177, 156, 217, 0.1)',
      200: 'rgba(177, 156, 217, 0.18)',
      300: 'rgba(177, 156, 217, 0.3)',
      400: 'rgba(177, 156, 217, 0.45)',
      500: '#B19CD9',            
      600: '#9B86C3',            
      700: 'rgba(155, 134, 195, 0.85)',
      800: 'rgba(155, 134, 195, 0.95)',
    },
    
    semantic: {
      success: '#7EE787',        
      successBg: 'rgba(126, 231, 135, 0.12)',
      successBorder: 'rgba(126, 231, 135, 0.35)',
      
      warning: '#FFD580',        
      warningBg: 'rgba(255, 213, 128, 0.12)',
      warningBorder: 'rgba(255, 213, 128, 0.35)',
      
      error: '#FF7B7B',          
      errorBg: 'rgba(255, 123, 123, 0.12)',
      errorBorder: 'rgba(255, 123, 123, 0.35)',
      
      info: '#6B8DD6',           
      infoBg: 'rgba(107, 141, 214, 0.12)',
      infoBorder: 'rgba(107, 141, 214, 0.35)',
      
      
      highlight: '#f5c563',
      highlightBg: 'rgba(245, 197, 99, 0.15)',
    },
    
    border: {
      subtle: 'rgba(107, 141, 214, 0.14)',
      base: 'rgba(107, 141, 214, 0.20)',
      medium: 'rgba(107, 141, 214, 0.28)',
      strong: 'rgba(107, 141, 214, 0.36)',
      prominent: 'rgba(107, 141, 214, 0.48)',
    },
    
    element: {
      subtle: 'rgba(107, 141, 214, 0.06)',
      soft: 'rgba(107, 141, 214, 0.09)',
      base: 'rgba(107, 141, 214, 0.13)',
      medium: 'rgba(107, 141, 214, 0.17)',
      strong: 'rgba(107, 141, 214, 0.22)',
      elevated: 'rgba(107, 141, 214, 0.27)',
    },
    
    git: {
      branch: 'rgb(107, 141, 214)',
      branchBg: 'rgba(107, 141, 214, 0.12)',
      changes: 'rgb(255, 213, 128)',
      changesBg: 'rgba(255, 213, 128, 0.12)',
      added: 'rgb(126, 231, 135)',
      addedBg: 'rgba(126, 231, 135, 0.12)',
      deleted: 'rgb(255, 123, 123)',
      deletedBg: 'rgba(255, 123, 123, 0.12)',
      staged: 'rgb(126, 231, 135)',
      stagedBg: 'rgba(126, 231, 135, 0.12)',
    },
  },
  
  
  effects: {
    shadow: {
      xs: '0 1px 3px rgba(0, 0, 0, 0.95)',
      sm: '0 2px 6px rgba(0, 0, 0, 0.9)',
      base: '0 4px 12px rgba(0, 0, 0, 0.85)',
      lg: '0 8px 20px rgba(0, 0, 0, 0.8)',
      xl: '0 12px 28px rgba(0, 0, 0, 0.75)',
      '2xl': '0 16px 36px rgba(0, 0, 0, 0.7)',
    },
    
    glow: {
      
      blue: '0 0 10px rgba(107, 141, 214, 0.3), 0 0 20px rgba(107, 141, 214, 0.2), 0 0 30px rgba(107, 141, 214, 0.1), 0 4px 12px rgba(0, 0, 0, 0.3)',
      
      purple: '0 0 10px rgba(177, 156, 217, 0.3), 0 0 20px rgba(177, 156, 217, 0.2), 0 0 30px rgba(177, 156, 217, 0.1), 0 4px 12px rgba(0, 0, 0, 0.3)',
      
      mixed: '0 0 12px rgba(107, 141, 214, 0.25), 0 0 24px rgba(177, 156, 217, 0.2), 0 0 36px rgba(107, 141, 214, 0.1), 0 4px 16px rgba(0, 0, 0, 0.35)',
    },
    
    blur: {
      subtle: 'blur(4px) saturate(1.15)',
      base: 'blur(8px) saturate(1.25)',
      medium: 'blur(12px) saturate(1.35)',
      strong: 'blur(16px) saturate(1.45) brightness(1.1)',
      intense: 'blur(20px) saturate(1.55) brightness(1.15)',
    },
    
    radius: {
      sm: '4px',
      base: '6px',
      lg: '10px',
      xl: '14px',
      '2xl': '18px',
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
      hover: 0.85,
      focus: 0.95,
      overlay: 0.45,
    },
  },
  
  
  motion: {
    duration: {
      instant: '0.1s',
      fast: '0.2s',
      base: '0.35s',
      slow: '0.6s',
      lazy: '0.9s',
    },
    
    easing: {
      standard: 'cubic-bezier(0.4, 0, 0.2, 1)',
      decelerate: 'cubic-bezier(0, 0, 0.2, 1)',
      accelerate: 'cubic-bezier(0.4, 0, 1, 1)',
      bounce: 'cubic-bezier(0.68, -0.55, 0.265, 1.55)',
      smooth: 'cubic-bezier(0.25, 0.46, 0.45, 0.94)',
    },
  },
  
  
  typography: {
    font: {
      sans: "'Noto Sans SC', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'SF Pro Display', 'Roboto', sans-serif",
      mono: "'FiraCode', 'JetBrains Mono', 'SF Mono', 'Cascadia Code', 'Consolas', monospace",
    },
    
    weight: {
      normal: 400,
      medium: 500,
      semibold: 600,
      bold: 700,
    },
    
    size: {
      xs: '12px',
      sm: '13px',
      base: '14px',
      lg: '16px',
      xl: '18px',
      '2xl': '20px',
      '3xl': '24px',
      '4xl': '30px',
      '5xl': '36px',
    },
    
    lineHeight: {
      tight: 1.3,
      base: 1.5,
      relaxed: 1.65,
    },
  },
  
  
  components: {
    
    windowControls: {
      minimize: {
        dot: 'rgba(107, 141, 214, 0.45)',
        dotShadow: '0 0 4px rgba(107, 141, 214, 0.25)',
        hoverBg: 'rgba(107, 141, 214, 0.12)',
        hoverColor: '#6B8DD6',
        hoverBorder: 'rgba(107, 141, 214, 0.25)',
        hoverShadow: '0 2px 8px rgba(107, 141, 214, 0.2), inset 0 1px 0 rgba(255, 255, 255, 0.1)',
      },
      maximize: {
        dot: 'rgba(107, 141, 214, 0.45)',
        dotShadow: '0 0 4px rgba(107, 141, 214, 0.25)',
        hoverBg: 'rgba(107, 141, 214, 0.12)',
        hoverColor: '#6B8DD6',
        hoverBorder: 'rgba(107, 141, 214, 0.25)',
        hoverShadow: '0 2px 8px rgba(107, 141, 214, 0.2), inset 0 1px 0 rgba(255, 255, 255, 0.1)',
      },
      close: {
        dot: 'rgba(255, 123, 123, 0.45)',
        dotShadow: '0 0 4px rgba(255, 123, 123, 0.25)',
        hoverBg: 'rgba(255, 123, 123, 0.12)',
        hoverColor: '#FF7B7B',
        hoverBorder: 'rgba(255, 123, 123, 0.25)',
        hoverShadow: '0 2px 8px rgba(255, 123, 123, 0.2), inset 0 1px 0 rgba(255, 255, 255, 0.1)',
      },
      common: {
        defaultColor: 'rgba(230, 237, 243, 0.9)',
        defaultDot: 'rgba(230, 237, 243, 0.2)',
        disabledDot: 'rgba(230, 237, 243, 0.1)',
        flowGradient: 'linear-gradient(90deg, transparent, rgba(107, 141, 214, 0.05), rgba(107, 141, 214, 0.08), rgba(107, 141, 214, 0.05), transparent)',
      },
    },
    
    button: {
      
      default: {
        background: 'rgba(107, 141, 214, 0.08)',
        color: '#8b949e',
        border: 'rgba(107, 141, 214, 0.15)',
        shadow: '0 0 6px rgba(107, 141, 214, 0.08)',
      },
      hover: {
        background: 'rgba(107, 141, 214, 0.14)',
        color: '#c9d1d9',
        border: 'rgba(107, 141, 214, 0.28)',
        shadow: '0 0 12px rgba(107, 141, 214, 0.15), 0 2px 8px rgba(0, 0, 0, 0.3)',
        transform: 'translateY(-1px)',
      },
      active: {
        background: 'rgba(107, 141, 214, 0.12)',
        color: '#c9d1d9',
        border: 'rgba(107, 141, 214, 0.3)',
        shadow: '0 0 10px rgba(107, 141, 214, 0.12)',
        transform: 'translateY(0)',
      },
      
      
      primary: {
        default: {
          background: 'rgba(107, 141, 214, 0.18)',
          color: '#e6edf3',
          border: 'rgba(107, 141, 214, 0.35)',
          shadow: '0 0 12px rgba(107, 141, 214, 0.2)',
        },
        hover: {
          background: 'rgba(107, 141, 214, 0.25)',
          color: '#ffffff',
          border: 'rgba(107, 141, 214, 0.55)',
          shadow: '0 0 20px rgba(107, 141, 214, 0.35), 0 0 32px rgba(107, 141, 214, 0.15), 0 4px 12px rgba(0, 0, 0, 0.3)',
          transform: 'translateY(-2px)',
        },
        active: {
          background: 'rgba(107, 141, 214, 0.22)',
          color: '#ffffff',
          border: 'rgba(107, 141, 214, 0.45)',
          shadow: '0 0 16px rgba(107, 141, 214, 0.25)',
          transform: 'translateY(-1px)',
        },
      },
      
      
      ghost: {
        default: {
          background: 'transparent',
          color: '#8b949e',
          border: 'rgba(107, 141, 214, 0.18)',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(107, 141, 214, 0.1)',
          color: '#c9d1d9',
          border: 'rgba(107, 141, 214, 0.32)',
          shadow: '0 0 10px rgba(107, 141, 214, 0.12)',
          transform: 'translateY(-1px)',
        },
        active: {
          background: 'rgba(107, 141, 214, 0.08)',
          color: '#c9d1d9',
          border: 'rgba(107, 141, 214, 0.28)',
          shadow: '0 0 8px rgba(107, 141, 214, 0.1)',
          transform: 'translateY(0)',
        },
      },
    },
  },
  
  
  monaco: {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '8b949e', fontStyle: 'italic' },
      { token: 'keyword', foreground: 'B19CD9' },
      { token: 'string', foreground: '7EE787' },
      { token: 'number', foreground: '6B8DD6' },
      { token: 'type', foreground: 'FFD580' },
      { token: 'class', foreground: 'FFD580' },
      { token: 'function', foreground: 'D2A8FF' },
      { token: 'variable', foreground: 'e6edf3' },
      { token: 'constant', foreground: '79C0FF' },
      { token: 'operator', foreground: 'B19CD9' },
      { token: 'tag', foreground: '7EE787' },
      { token: 'attribute.name', foreground: 'FFA657' },
      { token: 'attribute.value', foreground: '7EE787' },
    ],
    colors: {
      background: '#0a0e17',
      foreground: '#e6edf3',
      lineHighlight: '#0d1117',
      selection: '#1f3a5f',
      cursor: '#6B8DD6',
    },
  },
};

