import type { ReactNode } from "react";

/** The same header on every page: title, one line of context, actions. */
export function PageHeader({
  title,
  help,
  description,
  actions,
}: {
  title: string;
  help?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <header className="flex flex-wrap items-start justify-between gap-x-6 gap-y-3">
      <div className="min-w-0 max-w-[62ch]">
        <h1 className="flex items-center gap-2 text-[1.35rem] font-semibold leading-tight tracking-tight">
          {title}
          {help}
        </h1>
        {description && <p className="mt-1 text-sm text-muted">{description}</p>}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </header>
  );
}
