/** An on/off switch for settings (a checkbox underneath, for screen readers). */
export function Switch({
  checked,
  onChange,
  disabled,
  label,
}: {
  checked: boolean;
  onChange: (on: boolean) => void;
  disabled?: boolean;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full border transition-colors disabled:opacity-50 ${
        checked ? "border-process bg-process" : "border-rule bg-raised"
      }`}
    >
      <span
        aria-hidden
        className={`inline-block h-4.5 w-4.5 rounded-full bg-white shadow transition-transform ${checked ? "translate-x-[1.35rem]" : "translate-x-[0.2rem]"}`}
      />
    </button>
  );
}
