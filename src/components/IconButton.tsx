import type { ButtonHTMLAttributes, ReactNode } from "react";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  children: ReactNode;
}

export function IconButton({ label, children, className = "", ...rest }: Props) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={`grid h-7 w-7 shrink-0 place-items-center rounded-md text-muted transition-colors hover:bg-raised hover:text-fg disabled:opacity-40 ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}

const variants = {
  primary: "bg-fg text-ink hover:opacity-90",
  quiet: "text-muted hover:bg-raised hover:text-fg",
  secondary: "border border-rule text-fg hover:bg-raised",
  danger: "bg-missing/15 text-missing hover:bg-missing/25",
};

export function Button({
  variant = "quiet",
  className = "",
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: keyof typeof variants }) {
  return (
    <button
      type="button"
      className={`inline-flex h-8 items-center gap-1.5 whitespace-nowrap rounded-md px-3 text-sm font-semibold transition-colors disabled:opacity-50 ${variants[variant]} ${className}`}
      {...rest}
    />
  );
}
