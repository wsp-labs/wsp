import { useState } from "react";
import type { Props } from "./props";

export function Button({ label, onClick }: Props) {
  const [open, setOpen] = useState(false);
  const size = open ? 2 : 1;
  return (
    <div className="row" onClick={() => setOpen(!open)}>
      <span>{label}</span>
      <Icon name="x" size={size} />
    </div>
  );
}
