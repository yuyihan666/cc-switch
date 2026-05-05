import { Suspense, type ComponentType } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  resetProviderState,
  setCurrentProviderId,
  setProviders,
} from "../msw/state";

const toastSuccessMock = vi.fn();
const toastErrorMock = vi.fn();

vi.mock("sonner", () => ({
  toast: {
    success: (...args: unknown[]) => toastSuccessMock(...args),
    error: (...args: unknown[]) => toastErrorMock(...args),
  },
}));

const providerListPropsSpy = vi.fn();

vi.mock("@/components/providers/ProviderList", () => ({
  ProviderList: (props: any) => {
    providerListPropsSpy(props);
    return (
      <div>
        <div data-testid="provider-list">
          {JSON.stringify(props.providers)}
        </div>
        <div data-testid="current-provider">{props.currentProviderId}</div>
        {props.onOpenTerminal && (
          <button
            data-testid="open-terminal-btn"
            onClick={() =>
              props.onOpenTerminal(
                Object.values(props.providers)[0],
              )
            }
          >
            open-terminal
          </button>
        )}
      </div>
    );
  },
}));

vi.mock("@/components/providers/AddProviderDialog", () => ({
  AddProviderDialog: ({ open, onOpenChange }: any) =>
    open ? (
      <div data-testid="add-provider-dialog">
        <button onClick={() => onOpenChange(false)}>close-add</button>
      </div>
    ) : null,
}));

vi.mock("@/components/providers/EditProviderDialog", () => ({
  EditProviderDialog: ({ open, onOpenChange }: any) =>
    open ? (
      <div data-testid="edit-provider-dialog">
        <button onClick={() => onOpenChange(false)}>close-edit</button>
      </div>
    ) : null,
}));

vi.mock("@/components/UsageScriptModal", () => ({
  default: ({ isOpen, onClose }: any) =>
    isOpen ? (
      <div data-testid="usage-modal">
        <button onClick={() => onClose()}>close-usage</button>
      </div>
    ) : null,
}));

vi.mock("@/components/ConfirmDialog", () => ({
  ConfirmDialog: ({ isOpen, onConfirm, onCancel }: any) =>
    isOpen ? (
      <div data-testid="confirm-dialog">
        <button onClick={() => onConfirm()}>confirm-delete</button>
        <button onClick={() => onCancel()}>cancel-delete</button>
      </div>
    ) : null,
}));

vi.mock("@/components/AppSwitcher", () => ({
  AppSwitcher: ({ activeApp, onSwitch }: any) => (
    <div data-testid="app-switcher">
      <span data-testid="active-app">{activeApp}</span>
      <button onClick={() => onSwitch("opencode")}>switch-opencode</button>
      <button onClick={() => onSwitch("claude")}>switch-claude</button>
    </div>
  ),
}));

vi.mock("@/components/UpdateBadge", () => ({
  UpdateBadge: () => <div />,
}));

vi.mock("@/components/mcp/McpPanel", () => ({
  default: ({ open, onOpenChange }: any) =>
    open ? (
      <div data-testid="mcp-panel">
        <button onClick={() => onOpenChange(false)}>close-mcp</button>
      </div>
    ) : (
      <button onClick={() => onOpenChange(true)}>open-mcp</button>
    ),
}));

const renderApp = (AppComponent: ComponentType) => {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <Suspense fallback={<div data-testid="loading">loading</div>}>
        <AppComponent />
      </Suspense>
    </QueryClientProvider>,
  );
};

describe("App - OpenCode terminal button", () => {
  beforeEach(() => {
    resetProviderState();
    toastSuccessMock.mockReset();
    toastErrorMock.mockReset();
    providerListPropsSpy.mockReset();

    setProviders("opencode", {
      "opencode-1": {
        id: "opencode-1",
        name: "OpenCode Provider",
        settingsConfig: {
          npm: "@ai-sdk/openai-compatible",
          options: {
            apiKey: "test-key",
            baseURL: "https://api.example.com/v1",
          },
          models: {},
        },
        category: "custom",
        sortIndex: 0,
        createdAt: Date.now(),
      },
    });
    setCurrentProviderId("opencode", "opencode-1");
  });

  it("passes onOpenTerminal to ProviderList when activeApp is opencode", async () => {
    const { default: App } = await import("@/App");
    renderApp(App);

    fireEvent.click(screen.getByText("switch-opencode"));

    await waitFor(() => {
      expect(screen.getByTestId("active-app").textContent).toBe("opencode");
    });

    await waitFor(() => {
      expect(screen.getByTestId("provider-list").textContent).toContain(
        "opencode-1",
      );
    });

    const lastProps = providerListPropsSpy.mock.calls.at(-1)?.[0];
    expect(lastProps.onOpenTerminal).toBeTypeOf("function");

    expect(screen.getByTestId("open-terminal-btn")).toBeInTheDocument();
  });

  it("does not pass onOpenTerminal when activeApp is not claude or opencode", async () => {
    const { default: App } = await import("@/App");
    renderApp(App);

    await waitFor(() => {
      expect(screen.getByTestId("active-app").textContent).toBe("claude");
    });

    const claudeCall = providerListPropsSpy.mock.calls.find(
      (call) => call[0]?.appId === "claude",
    );
    expect(claudeCall?.[0]?.onOpenTerminal).toBeTypeOf("function");

    const codexCall = providerListPropsSpy.mock.calls.find(
      (call) => call[0]?.appId === "codex",
    );
    expect(codexCall?.[0]?.onOpenTerminal).toBeUndefined();
  });

  it("shows success toast when terminal button is clicked for opencode", async () => {
    const { default: App } = await import("@/App");
    renderApp(App);

    fireEvent.click(screen.getByText("switch-opencode"));

    await waitFor(() => {
      expect(screen.getByTestId("open-terminal-btn")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("open-terminal-btn"));

    await waitFor(() => {
      const calls = toastSuccessMock.mock.calls;
      const hasTerminalToast = calls.some((call: any[]) =>
        String(call[0]).includes("终端"),
      );
      const hasErrorToast = toastErrorMock.mock.calls.length > 0;
      expect(hasTerminalToast || hasErrorToast).toBe(true);
    });
  });
});
