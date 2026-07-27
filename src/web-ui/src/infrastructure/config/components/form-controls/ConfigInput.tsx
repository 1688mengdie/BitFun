import React from 'react';

export interface ConfigInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  size?: 'small' | 'medium' | 'large';
  className?: string;
}

export const ConfigInput: React.FC<ConfigInputProps> = ({
  value,
  onChange,
  placeholder,
  disabled,
  className,
}) => (
  <input
    type="text"
    value={value}
    onChange={(e) => onChange(e.target.value)}
    placeholder={placeholder}
    disabled={disabled}
    className={className}
  />
);
