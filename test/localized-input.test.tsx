// @vitest-environment jsdom
import { fireEvent, render, screen, cleanup } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { LocalizedDecimalInput } from "../apps/desktop/src/LocalizedDecimalInput.js";

afterEach(cleanup);
it("passes unfinished numeric input to the draft instead of silently losing it", () => {
  const onChange = vi.fn();
  render(<LocalizedDecimalInput aria-label="Preis" value="10" onChange={onChange} />);
  fireEvent.change(screen.getByLabelText("Preis"),{ target:{value:"12,"} });
  expect(onChange).toHaveBeenLastCalledWith("12,");
  fireEvent.change(screen.getByLabelText("Preis"),{ target:{value:"12,50"} });
  expect(onChange).toHaveBeenLastCalledWith("12.50");
});
