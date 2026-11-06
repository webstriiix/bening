import React from "react";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "danger" | "ok" | "disabled";
}

const Button: React.FC<ButtonProps> = ({
  variant = "primary",
  children,
  disabled,
  ...props
}) => {
  return (
    <button
      className={`btn btn-${variant}`}
      disabled={disabled}
      {...props}
    >
      {children}
    </button>
  );
};

export default Button;