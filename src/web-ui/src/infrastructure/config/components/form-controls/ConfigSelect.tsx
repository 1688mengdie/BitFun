import React from 'react';

export interface ConfigSelectOption {
  label: string;
  value: string | number;
  disabled?: boolean;
}

export interface ConfigSelectProps {
  value: string | number;
  onChange: (value: string | number) => void;
  options: ConfigSelectOption[];
  placeholder?: string;
  disabled?: boolean;
  size?: 'small' | 'medium' | 'large';
  className?: string;
}

export const ConfigSelect: React.FC<ConfigSelectProps> = ({
  value,
  onChange,
  options,
  placeholder,
  disabled,
  className,
}) => (
  <select
    value={value}
    onChange={(e) => onChange(e.target.value)}
    disabled={disabled}
    className={className}
  >
    {placeholder && <option value="">{placeholder}</option>}
    {options.map((opt) => (
      <option key={String(opt.value)} value={opt.value} disabled={opt.disabled}>
        {opt.label}
      </option>
    ))}
  </select>
);
