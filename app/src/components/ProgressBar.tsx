import React from "react";

interface ProgressBarProps extends React.HTMLAttributes<HTMLDivElement> {
  target: number;
  current: number;
}

const ProgressBar: React.FC<ProgressBarProps> = ({ target, current, ...props }) => {
  const progress = target > 0 ? Math.min((current / target) * 100, 100) : 0;
  return (
    <div className="progress-bar" {...props}>
      <div className="progress-fill" style={{ width: `${progress}%` }} />
    </div>
  );
};

export default ProgressBar;