// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render } from '@testing-library/react';
import App from './App';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(async () => []),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

describe('App bootstrap fetch', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    invokeMock.mockReset();
    invokeMock.mockResolvedValue([]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('启动后应立即查询书签，不依赖输入框变更', async () => {
    render(<App />);

    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith('get_bookmarks');
  });
});
