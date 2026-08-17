import { createChatStore } from "@ora/chat";
import type { ContractsClient } from "@ora/contracts";
import { PlatformProvider } from "@ora/platform";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { appI18n } from "../../i18n/i18n-instance";
import { queryKeys } from "../../state/hooks/query-keys";
import { useUiStore } from "../../state/stores/ui-store";
import {
  createHookWrapper,
  createTestQueryClient,
} from "../../test/hook-harness";
import {
  createMockClient,
  createMockClientState,
} from "../../test/mock-client";
import { createStubPlatform } from "../../test/stub-platform";
import { SettingsDialog } from "./settings-dialog";

describe("SettingsDialog developer options", () => {
  beforeEach(async () => {
    await appI18n.changeLanguage("en-US");
    useUiStore.setState({ settingsOpen: true });
  });

  it("keeps Advanced and its switch reachable while developer-only navigation stays hidden", async () => {
    const client = createMockClient(createMockClientState());
    renderDialog(client);

    expect(
      screen.queryByRole("button", { name: "Developer options" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Advanced" }),
    ).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Data & privacy" }),
    );
    expect(
      screen.queryByRole("switch", { name: "Developer mode" }),
    ).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Advanced" }));
    const developerModeSwitch = await screen.findByRole("switch", {
      name: "Developer mode",
    });
    expect(developerModeSwitch).toBeEnabled();
    expect(
      screen.queryByRole("combobox", { name: "Log level" }),
    ).not.toBeInTheDocument();
    await userEvent.click(developerModeSwitch);
    expect(
      await screen.findByRole("button", { name: "Developer options" }),
    ).toBeInTheDocument();
  });

  it("keeps Advanced reachable and the switch disabled when the initial read fails", async () => {
    const client = createMockClient(createMockClientState());
    client.developerMode.get = vi
      .fn()
      .mockRejectedValue(new Error("read failed"));
    renderDialog(client);

    expect(
      screen.getByRole("button", { name: "Advanced" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Developer options" }),
    ).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Advanced" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "could not be loaded",
    );
    expect(
      screen.getByRole("switch", { name: "Developer mode" }),
    ).toHaveAttribute("aria-disabled", "true");
  });

  it("shows the developer category and authoritative effective log level when enabled", async () => {
    const state = createMockClientState();
    state.developerMode = { enabled: true };
    state.runtimeLogLevel = {
      configuredLevel: "info",
      effectiveLevel: "trace",
      startupOverride: "trace",
    };
    renderDialog(createMockClient(state));

    const developerNavigation = await screen.findByRole("button", {
      name: "Developer options",
    });
    await userEvent.click(developerNavigation);

    const selector = await screen.findByRole("combobox", { name: "Log level" });
    expect(selector).toHaveTextContent("Trace (most detailed)");
    expect(screen.queryByText(/ORA_LOG_LEVEL/)).not.toBeInTheDocument();
  });

  it("redirects to Advanced and unmounts developer content when disabled", async () => {
    const state = createMockClientState();
    state.developerMode = { enabled: true };
    const { queryClient } = renderDialog(createMockClient(state));

    await userEvent.click(
      await screen.findByRole("button", { name: "Developer options" }),
    );
    expect(
      await screen.findByRole("combobox", { name: "Log level" }),
    ).toBeInTheDocument();

    act(() => {
      queryClient.setQueryData(queryKeys.developerMode, { enabled: false });
    });

    await waitFor(() => {
      expect(
        screen.queryByRole("button", { name: "Developer options" }),
      ).not.toBeInTheDocument();
    });
    expect(
      screen.queryByRole("combobox", { name: "Log level" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: "Advanced" }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("switch", { name: "Developer mode" }),
    ).toBeInTheDocument();
  });
});

/** Renders the real settings dialog with shared client, query, chat, i18n, and platform providers. */
function renderDialog(client: ContractsClient) {
  const queryClient = createTestQueryClient();
  const AppProviders = createHookWrapper(
    client,
    queryClient,
    createChatStore(client.session),
  );

  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <PlatformProvider adapter={createStubPlatform()}>
        <AppProviders>{children}</AppProviders>
      </PlatformProvider>
    );
  }

  return { ...render(<SettingsDialog />, { wrapper: Wrapper }), queryClient };
}
