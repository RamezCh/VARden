import { useEffect, useRef, type ReactNode } from "react";
import { IconButton } from "./IconButton";
import { XIcon } from "./icons";

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: ReactNode;
  children: ReactNode;
  footer: ReactNode;
}

/** Native <dialog>: focus trapping, Esc to close and inert background come free. */
export function Modal({ open, onClose, title, description, children, footer }: Props) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) {
      d.showModal();
      // Start in the first field, not on a help icon or the close button.
      d.querySelector<HTMLElement>("[data-autofocus], input:not([type=checkbox]), select, textarea")?.focus();
    }
    if (!open && d.open) d.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      aria-labelledby="modal-title"
      className="m-auto w-[min(720px,calc(100vw-2rem))] rounded-xl border border-rule bg-panel p-0 text-fg shadow-2xl shadow-[var(--shadow)] backdrop:bg-[var(--backdrop)]"
    >
      {open && (
        <div className="flex max-h-[min(720px,calc(100vh-4rem))] flex-col">
          <header className="flex items-start gap-4 border-b border-rule px-5 py-4">
            <div className="min-w-0 flex-1">
              <h2 id="modal-title" className="text-lg font-semibold">
                {title}
              </h2>
              {description && <div className="mt-0.5 text-sm text-muted">{description}</div>}
            </div>
            <IconButton label="Close" onClick={onClose}>
              <XIcon />
            </IconButton>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">{children}</div>
          <footer className="flex flex-wrap items-center justify-end gap-2 border-t border-rule px-5 py-3">{footer}</footer>
        </div>
      )}
    </dialog>
  );
}
