import React from "react";

interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  title: string;
  children: React.ReactNode;
}

const Card: React.FC<CardProps> = ({ title, children, ...props }) => {
  return (
    <div className="card" {...props}>
      <h2>{title}</h2>
      {children}
    </div>
  );
};

export default Card;