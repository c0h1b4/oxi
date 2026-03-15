import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { DisplaySettings } from "../DisplaySettings";

const { mockUseDisplayPreferences, mockUseUpdateDisplayPreferences, mockMutate } = vi.hoisted(() => ({
  mockUseDisplayPreferences: vi.fn(),
  mockUseUpdateDisplayPreferences: vi.fn(),
  mockMutate: vi.fn(),
}));

vi.mock("@/hooks/useDisplayPreferences", () => ({
  useDisplayPreferences: mockUseDisplayPreferences,
  useUpdateDisplayPreferences: mockUseUpdateDisplayPreferences,
}));

describe("DisplaySettings animation mode", () => {
  beforeEach(() => {
    mockMutate.mockReset();
    mockUseUpdateDisplayPreferences.mockReturnValue({ mutate: mockMutate });
    mockUseDisplayPreferences.mockReturnValue({
      isLoading: false,
      data: {
        density: "comfortable",
        theme: "system",
        language: "en",
        compose_format: "html",
        deep_index: false,
        animation_mode: "subtle",
        updated_at: "2026-03-14T00:00:00Z",
      },
    });
  });

  it("shows the existing animation mode selected", () => {
    render(<DisplaySettings />);

    const subtleButton = screen.getByRole("button", { name: "Subtle" });
    expect(subtleButton.className).toContain("bg-background");
  });

  it("updates animation_mode when selecting each option", () => {
    render(<DisplaySettings />);

    const options: Array<{ label: string; value: "rich" | "medium" | "subtle" | "off" }> = [
      { label: "Rich", value: "rich" },
      { label: "Medium", value: "medium" },
      { label: "Subtle", value: "subtle" },
      { label: "Off", value: "off" },
    ];

    for (const option of options) {
      fireEvent.click(screen.getByRole("button", { name: option.label }));
      expect(mockMutate).toHaveBeenCalledWith(
        { animation_mode: option.value },
        expect.objectContaining({ onError: expect.any(Function) }),
      );
    }
  });

  it("resets animation_mode to null", () => {
    render(<DisplaySettings />);

    fireEvent.click(screen.getByRole("button", { name: "Reset to defaults" }));

    expect(mockMutate).toHaveBeenCalledWith(
      expect.objectContaining({ animation_mode: null }),
      expect.objectContaining({ onError: expect.any(Function) }),
    );
  });
});
