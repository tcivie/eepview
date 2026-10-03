export interface Checkable {
  value: string;
  checked: boolean;
}

export interface FieldLike {
  setAttribute(name: string, value: string): void;
}

export interface HintLike {
  textContent: string | null;
  classList: { toggle(token: string, force?: boolean): unknown };
}

export function checkMatching(inputs: Iterable<Checkable>, value: string): void {
  for (const input of inputs) input.checked = input.value === value;
}

export function setFieldError(
  field: FieldLike,
  hint: HintLike,
  message: string | null,
  normalHint: string,
): void {
  field.setAttribute("aria-invalid", String(message !== null));
  hint.textContent = message ?? normalHint;
  hint.classList.toggle("field-error", message !== null);
}
