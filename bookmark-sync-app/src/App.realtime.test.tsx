// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render } from '@testing-library/react';
import App from './App';

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(async () => []),
  listenMock: vi.fn(async () => () => {}),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

describe('App realtime refresh', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
    invokeMock.mockResolvedValue([]);
    listenMock.mockResolvedValue(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('应监听 bookmarks-updated 事件', async () => {
    render(<App />);
    await Promise.resolve();
    expect(listenMock).toHaveBeenCalledWith('bookmarks-updated', expect.any(Function));
  });

  it('收到 bookmarks-updated 事件后应立即刷新查询', async () => {
    render(<App />);
    await Promise.resolve();

    const handler = (listenMock as any).mock.calls[0][1] as () => Promise<void>;
    await handler();

    const calls = (invokeMock as any).mock.calls.filter((call: any[]) => call[0] === 'get_bookmarks');
    expect(calls.length).toBeGreaterThanOrEqual(2);
  });
});
