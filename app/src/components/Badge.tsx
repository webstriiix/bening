import React from "react";

type BadgeStatus = "pending" | "proof_submitted" | "approved" | "released";

interface BadgeProps {
  status: BadgeStatus;
  children: React.ReactNode;
}

const statusStyles: Record<BadgeStatus, React.CSSProperties> = {
  pending: {
    backgroundColor: "var(--grey)",
    color: "var(--ink)",
  },
  proof_submitted: {
    backgroundColor: "var(--yellow)",
    color: "var(--ink)",
  },
  approved: {
    backgroundColor: "var(--blue)",
    color: "var(--ink)",
  },
  released: {
    backgroundColor: "var(--green)",
    color: "var(--ink)",
  },
};

const Badge: React.FC<BadgeProps> = ({ status, children }) => {
  return (
    <span className={`badge badge-${status}`} style={statusStyles[status]}>
      {children}
    </span>
  );
};

export default Badge;