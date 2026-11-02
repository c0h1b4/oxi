import { act } from "@testing-library/react";
import { hydrateRoot } from "react-dom/client";
import { renderToString } from "react-dom/server";
import { afterEach, expect, it } from "vitest";

import { useLocale, useT } from "@/hooks/useI18n";
import { useUiStore } from "@/stores/useUiStore";

function Greeting() {
  const locale = useLocale();
  const t = useT();
  return <p lang={locale}>{t("login.title")}</p>;
}

afterEach(() => {
  useUiStore.setState({ locale: "en" });
  localStorage.removeItem("oxi-locale");
});

it("hydrates exported English markup before applying the saved Spanish locale", async () => {
  useUiStore.getState().setLocale("es");
  const container = document.createElement("div");
  container.innerHTML = renderToString(<Greeting />);
  expect(container.textContent).toBe("Welcome back");
  document.body.appendChild(container);
  const errors: unknown[] = [];
  let root: ReturnType<typeof hydrateRoot>;
  await act(async () => {
    root = hydrateRoot(container, <Greeting />, {
      onRecoverableError: (error) => errors.push(error),
    });
  });
  expect(errors).toEqual([]);
  expect(container.textContent).toBe("Hola de nuevo");
  expect(container.querySelector("p")?.lang).toBe("es");
  expect(localStorage.getItem("oxi-locale")).toBe("es");
  await act(async () => {
    useUiStore.getState().setLocale("en");
  });
  expect(container.textContent).toBe("Welcome back");
  await act(async () => root.unmount());
  container.remove();
});
