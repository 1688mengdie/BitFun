/**
 * Input component
 */

import React, { forwardRef } from 'react';
import './Input.scss';

export interface InputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size'> {
  variant?: 'default' | 'filled' | 'outlined';
  inputSize?: 'small' | 'medium' | 'large';
  error?: boolean;
  errorMessage?: string;
  prefix?: React.ReactNode;
  suffix?: React.ReactNode;
  label?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(({
  variant = 'default',
  inputSize = 'medium',
  error = false,
  errorMessage,
  prefix,
  suffix,
  label,
  className = '',
  disabled,
  ...props
}, ref) => {
  const classNames = [
    'bitfun-input-wrapper',
    `bitfun-input-wrapper--${variant}`,
    `bitfun-input-wrapper--${inputSize}`,
    error && 'bitfun-input-wrapper--error',
    disabled && 'bitfun-input-wrapper--disabled',
    className
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className={classNames}>
      {label && <label className="bitfun-input-label">{label}</label>}
      <div className="bitfun-input-container">
        {prefix && <span className="bitfun-input-prefix">{prefix}</span>}
        <input
          ref={ref}
          className="bitfun-input"
          disabled={disabled}
          {...props}
        />
        {suffix && <span className="bitfun-input-suffix">{suffix}</span>}
      </div>
      {error && errorMessage && (
        <span className="bitfun-input-error-message">{errorMessage}</span>
      )}
    </div>
  );
});

Input.displayName = 'Input';