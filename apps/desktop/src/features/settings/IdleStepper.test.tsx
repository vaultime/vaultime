// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { IdleStepper } from "./IdleStepper";

function renderAt(seconds: number) {
  const onChange = vi.fn();
  render(<IdleStepper id="idle" seconds={seconds} onChange={onChange} />);
  return { onChange, field: screen.getByRole("spinbutton") };
}

describe("the idle time stepper", () => {
  it("steps a minute at a time with the arrow keys", () => {
    const { onChange, field } = renderAt(150);
    fireEvent.keyDown(field, { key: "ArrowUp" });
    expect(onChange).toHaveBeenLastCalledWith(180);
    fireEvent.keyDown(field, { key: "ArrowDown" });
    expect(onChange).toHaveBeenLastCalledWith(120);
  });

  it("never steps a short typed time up with the key that steps down", () => {
    const { onChange, field } = renderAt(30);
    expect(screen.getByRole("button", { name: "One minute less" }).getAttribute("aria-disabled")).toBe("true");
    fireEvent.keyDown(field, { key: "ArrowDown" });
    expect(onChange).not.toHaveBeenCalled();
  });
});
