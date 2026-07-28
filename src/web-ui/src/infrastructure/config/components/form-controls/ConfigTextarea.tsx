import React from 'react';

export interface ConfigTextareaProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  rows?: number;
  className?: string;
}

export const ConfigTextarea: React.FC<ConfigTextareaProps> = ({
  value,
  onChange,
  placeholder,
  disabled,
  rows = 3,
  className,
}) => (
  <textarea
    value={value}
    onChange={(e) => onChange(e.target.value)}
    placeholder={placeholder}
    disabled={disabled}
    rows={rows}
    className={className}
  />
);
