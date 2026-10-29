import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SubjectInput } from "../ComposeDialog/SubjectInput";

vi.mock("@/hooks/useSubjectSuggestions", () => ({
  useSubjectSuggestions: () => ["Project update", "Project meeting"],
}));
afterEach(cleanup);

describe("SubjectInput", () => {
  it("exposes accessible keyboard suggestions without replacing free text automatically", () => {
    const onChange = vi.fn();
    render(<SubjectInput value="Pro" onChange={onChange} />);
    const field = screen.getByRole("combobox", { name: "Subject" });
    expect(screen.queryByRole("listbox")).toBeNull();
    fireEvent.focus(field);
    expect(field).toHaveAttribute("aria-expanded", "true");
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.keyDown(field, { key: "ArrowDown" });
    const first = screen.getAllByRole("option")[0];
    expect(first).toHaveAttribute("aria-selected", "true");
    expect(field).toHaveAttribute("aria-activedescendant", first.id);
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("Project update");
  });

  it("supports mouse selection, dismissal, blur and normal Tab navigation", () => {
    const onChange = vi.fn();
    render(<SubjectInput value="Pro" onChange={onChange} />);
    const field = screen.getByRole("combobox");
    fireEvent.focus(field);
    fireEvent.click(screen.getByRole("option", { name: "Project meeting" }));
    expect(onChange).toHaveBeenCalledWith("Project meeting");
    fireEvent.keyDown(field, { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();
    fireEvent.change(field, { target: { value: "Project" } });
    fireEvent.keyDown(field, { key: "Tab" });
    expect(onChange).toHaveBeenLastCalledWith("Project");
    fireEvent.blur(field);
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("preserves compose shortcuts and can disable suggestions for replies", () => {
    const onChange = vi.fn();
    const shortcut = vi.fn();
    const view = render(<div onKeyDown={shortcut}><SubjectInput value="Pro" onChange={onChange} /></div>);
    const field = screen.getByRole("combobox");
    fireEvent.focus(field);
    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    expect(onChange).not.toHaveBeenCalled();
    expect(shortcut).toHaveBeenCalled();
    view.rerender(<SubjectInput value="Re: Project" onChange={onChange} enabled={false} />);
    fireEvent.focus(screen.getByRole("combobox"));
    expect(screen.queryByRole("listbox")).toBeNull();
  });
});
