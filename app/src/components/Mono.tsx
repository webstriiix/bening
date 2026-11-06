import React from "react";

interface MonoProps extends React.HTMLAttributes<HTMLPreElement> {
  text: string;
}

const Mono: React.FC<MonoProps> = ({ text, ...props }) => {
  return <pre className="mono" {...props}>{text}</pre>;
};

export default Mono;