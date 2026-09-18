import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useUiStore } from "@/stores/useUiStore";
import { DisplaySettings } from "../DisplaySettings";

const { mockUseDisplayPreferences, mockUseUpdateDisplayPreferences, mockMutate } = vi.hoisted(
  () => ({
    mockUseDisplayPreferences: vi.fn(),
    mockUseUpdateDisplayPreferences: vi.fn(),
    mockMutate: vi.fn(),
  }),
);

vi.mock("@/hooks/useDisplayPreferences", () => ({
  useDisplayPreferences: mockUseDisplayPreferences,
  useUpdateDisplayPreferences: mockUseUpdateDisplayPreferences,
}));

function preferences(language: string) {
  return {
    isLoading: false,
    data: {
      density: "comfortable",
      theme: "system",
      language,
      compose_format: "html",
      deep_index: false,
      animation_mode: "subtle",
      updated_at: "2026-03-14T00:00:00Z",
    },
  };
}

describe("DisplaySettings language", () => {
  beforeEach(() => {
    mockMutate.mockReset();
    mockUseUpdateDisplayPreferences.mockReturnValue({ mutate: mockMutate });
    mockUseDisplayPreferences.mockReturnValue(preferences("en"));
    useUiStore.setState({ locale: "en" });
  });

  afterEach(() => {
    useUiStore.setState({ locale: "en" });
  });

  it("offers every supported locale by its own name", () => {
    render(<DisplaySettings />);

    expect(screen.getByRole("button", { name: "English" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Español" })).toBeTruthy();
  });

  it("marks the stored language as selected", () => {
    mockUseDisplayPreferences.mockReturnValue(preferences("es"));
    render(<DisplaySettings />);

    expect(screen.getByRole("button", { name: "Español" }).className).toContain(
      "bg-background",
    );
  });

  it("falls back to English when the stored language is not supported", () => {
    mockUseDisplayPreferences.mockReturnValue(preferences("klingon"));
    render(<DisplaySettings />);

    expect(screen.getByRole("button", { name: "English" }).className).toContain(
      "bg-background",
    );
  });

  it("persists the choice through the preferences API, not just locally", () => {
    render(<DisplaySettings />);

    fireEvent.click(screen.getByRole("button", { name: "Español" }));

    expect(mockMutate).toHaveBeenCalledWith(
      { language: "es" },
      expect.objectContaining({ onError: expect.any(Function) }),
    );
    expect(useUiStore.getState().locale).toBe("es");
  });

  it("resets the language back to English", () => {
    mockUseDisplayPreferences.mockReturnValue(preferences("es"));
    useUiStore.setState({ locale: "es" });
    render(<DisplaySettings />);

    fireEvent.click(screen.getByRole("button", { name: "Restablecer valores predeterminados" }));

    expect(mockMutate).toHaveBeenCalledWith(
      expect.objectContaining({ language: "en" }),
      expect.objectContaining({ onError: expect.any(Function) }),
    );
    expect(useUiStore.getState().locale).toBe("en");
  });

  it("renders the whole panel in the active locale", () => {
    useUiStore.setState({ locale: "es" });
    render(<DisplaySettings />);

    expect(screen.getByText("Apariencia")).toBeTruthy();
    expect(screen.getByText("Idioma")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Cómoda" })).toBeTruthy();
    expect(screen.queryByText("Display")).toBeNull();
  });
});
