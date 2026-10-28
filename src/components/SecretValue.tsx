import { useState } from "react";
import { CheckIcon, CopyIcon, EyeIcon, EyeOffIcon } from "./icons";
import { IconButton } from "./IconButton";
import { copyText } from "../lib/clipboard";

const MASK = "••••••••••••";

interface Props {
  value: string;
  sensitive: boolean;
  revealed: boolean;
  onToggle: () => void;
}

/**
 * A value cell. Secrets show a fixed-length mask (so the length isn't
 * leaked) and can be copied without being revealed.
 */
export function SecretValue({ value, sensitive, revealed, onToggle }: Props) {
  const [copied, setCopied] = useState(false);
  const masked = sensitive && !revealed;

  const copy = async () => {
    await copyText(value, sensitive);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  };

  return (
    <div className="group/value flex min-w-0 items-center gap-1">
      <span
        className={`min-w-0 truncate font-mono text-[0.9rem] ${masked ? "tracking-widest text-faint" : "text-fg"}`}
        title={masked ? undefined : value}
      >
        {value === "" ? <span className="font-sans italic text-faint">empty</span> : masked ? MASK : value}
      </span>
      {sensitive && value !== "" && (
        <IconButton label={masked ? "Show value" : "Hide value"} onClick={onToggle}>
          {masked ? <EyeIcon /> : <EyeOffIcon />}
        </IconButton>
      )}
      {value !== "" && (
        <IconButton
          label={copied ? (sensitive ? "Copied; cleared from the clipboard in 30 seconds" : "Copied") : "Copy value"}
          onClick={() => void copy()}
          className="opacity-0 focus-visible:opacity-100 group-hover/value:opacity-100"
        >
          {copied ? <CheckIcon className="text-synced" /> : <CopyIcon />}
        </IconButton>
      )}
    </div>
  );
}
