// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from "vitest";
import { installErrorCapture } from "./errorCapture";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("errorCapture", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("应捕获 window error 并写入 debug log", async () => {
    installErrorCapture();
    const event = new Event("error") as Event & { message?: string; error?: Error };
    event.message = "boom";
    event.error = new Error("boom");
    window.dispatchEvent(event);
    expect(invokeMock).toHaveBeenCalledWith("write_debug_log", {
      message: expect.stringContaining("window error: boom"),
    });
  });

  it("应捕获未处理的 Promise 拒绝并写入 debug log", async () => {
    installErrorCapture();
    const event = new Event("unhandledrejection") as Event & { reason?: unknown };
    event.reason = new Error("nope");
    window.dispatchEvent(event);
    expect(invokeMock).toHaveBeenCalledWith("write_debug_log", {
      message: expect.stringContaining("unhandledrejection: Error: nope"),
    });
  });
});
