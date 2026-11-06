import React from "react";

interface TextInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
}

const TextInput: React.FC<TextInputProps> = ({ label, ...props }) => {
  return (
    <div>
      {label ? <label className="text-input-label">{label}</label> : null}
      <input className="text-input" {...props} />
    </div>
  );
};

export default TextInput;