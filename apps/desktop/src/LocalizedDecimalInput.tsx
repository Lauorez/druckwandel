import { useEffect, useState } from "react";
import { formatGermanDecimal, parseLocalizedDecimal } from "../../../src/domain/localized-decimal.js";

interface LocalizedDecimalInputProps {
  "aria-label": string;
  value: string;
  onChange: (value: string) => void;
  minimumFractionDigits?: number;
  maximumFractionDigits?: number;
  disabled?: boolean;
}

export function LocalizedDecimalInput({
  value,
  onChange,
  minimumFractionDigits = 2,
  maximumFractionDigits = 2,
  disabled = false,
  ...props
}: LocalizedDecimalInputProps) {
  const [draft, setDraft] = useState(() => formatGermanDecimal(value, minimumFractionDigits, maximumFractionDigits));
  const [focused, setFocused] = useState(false);
  const valid = draft === "" || parseLocalizedDecimal(draft) !== null;

  useEffect(() => {
    if (!focused) setDraft(formatGermanDecimal(value, minimumFractionDigits, maximumFractionDigits));
  }, [focused, maximumFractionDigits, minimumFractionDigits, value]);

  return <input
    {...props}
    value={draft}
    disabled={disabled}
    inputMode="decimal"
    aria-invalid={!valid}
    onFocus={() => setFocused(true)}
    onChange={(event) => {
      const next = event.target.value;
      setDraft(next);
      if (next.trim() === "") onChange("");
      else {
        const parsed = parseLocalizedDecimal(next);
        // Preserve unfinished/invalid input in the draft as well. Otherwise the
        // autosave badge could say "saved" while these visible edits are lost.
        onChange(parsed ?? next);
      }
    }}
    onBlur={() => {
      setFocused(false);
      setDraft(formatGermanDecimal(value, minimumFractionDigits, maximumFractionDigits));
    }}
  />;
}
