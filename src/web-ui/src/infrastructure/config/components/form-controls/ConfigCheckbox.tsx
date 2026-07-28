import React from 'react';

export interface ConfigCheckboxProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  disabled?: boolean;
  className?: string;
}

export const ConfigCheckbox: React.FC<ConfigCheckboxProps> = ({
  checked,
  onChange,
  label,
  disabled,
  className,
}) => (
  <label className={className}>
    <input
      type="checkbox"
      checked={checked}
      onChange={(e) => onChange(e.target.checked)}
      disabled={disabled}
    />
    {label && <span>{label}</span>}
  </label>
);
