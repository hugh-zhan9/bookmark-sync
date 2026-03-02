// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';
import App from './App';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

describe('App bookmark management', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    cleanup();
  });

  beforeEach(() => {
    invokeMock.mockReset();
    let getFoldersCount = 0;
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_folders') {
        getFoldersCount += 1;
        if (getFoldersCount <= 1) {
          return [
            { id: 'f1', name: '工作', parent_id: null },
            { id: 'f2', name: '灵感', parent_id: null },
          ];
        }
        return [
          { id: 'f1', name: '工作', parent_id: null },
          { id: 'f2', name: '灵感', parent_id: null },
          { id: 'f3', name: '新建夹', parent_id: null },
        ];
      }
      if (cmd === 'get_tags') {
        return [{ id: 't1', name: '效率' }];
      }
      if (cmd === 'get_delete_sync_setting') {
        return false;
      }
      if (cmd === 'get_browser_auto_sync_settings') {
        return { startup_enabled: false, interval_enabled: false, interval_minutes: 5 };
      }
      if (cmd === 'get_git_sync_repo_dir') {
        return '';
      }
      if (cmd === 'get_event_auto_sync_settings') {
        return {
          startup_pull_enabled: false,
          interval_enabled: false,
          interval_minutes: 5,
          close_push_enabled: true,
        };
      }
      if (cmd === 'get_ui_appearance_settings') {
        return {
          theme_mode: 'system',
          background_enabled: false,
          background_image_data_url: null,
          background_overlay_opacity: 45,
        };
      }
      if (cmd === 'get_bookmarks') {
        return [{
          id: 'b1',
          url: 'https://example.com',
          title: '示例',
          host: 'example.com',
          created_at: '2026-03-02T00:00:00Z',
          tags: ['效率'],
        }];
      }
      if (cmd === 'get_bookmark_folders') {
        return ['f1'];
      }
      if (cmd === 'search_bookmarks') {
        return [];
      }
      if (cmd === 'update_bookmark' || cmd === 'rename_folder' || cmd === 'remove_bookmark_from_folder' || cmd === 'add_bookmark_to_folder' || cmd === 'remove_tag_from_bookmark' || cmd === 'add_tag_to_bookmark' || cmd === 'create_folder' || cmd === 'delete_folder' || cmd === 'set_delete_sync_setting' || cmd === 'set_browser_auto_sync_settings' || cmd === 'set_git_sync_repo_dir' || cmd === 'delete_bookmark' || cmd === 'write_debug_log' || cmd === 'set_event_auto_sync_settings' || cmd === 'sync_event_pull_only' || cmd === 'sync_event_push_only' || cmd === 'sync_github_incremental' || cmd === 'set_ui_appearance_settings') {
        return null;
      }
      return [];
    });
  });

  it('点击文件夹重命名后应调用 rename_folder', async () => {
    render(<App />);

    const renameButtons = await screen.findAllByRole('button', { name: '重命名文件夹' });
    fireEvent.click(renameButtons[0]);

    const input = await screen.findByPlaceholderText('文件夹名称');
    fireEvent.change(input, { target: { value: '项目A' } });
    fireEvent.click(screen.getByRole('button', { name: '保存重命名' }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('rename_folder', {
        id: 'f1',
        name: '项目A',
      });
    });
  });

  it('编辑书签时应支持手动输入文件夹，存在则关联，不存在则创建后关联', async () => {
    render(<App />);

    const editButton = await screen.findByRole('button', { name: '编辑书签-示例' });
    fireEvent.click(editButton);

    const folderInput = await screen.findByLabelText('所属文件夹（逗号分隔）');
    fireEvent.change(folderInput, { target: { value: '工作, 新建夹' } });

    const tagInput = screen.getByLabelText('标签（逗号分隔）');
    fireEvent.change(tagInput, { target: { value: '重要' } });

    fireEvent.click(screen.getByRole('button', { name: '保存书签' }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('update_bookmark', expect.any(Object));
      expect(invokeMock).not.toHaveBeenCalledWith('remove_bookmark_from_folder', {
        bookmarkId: 'b1',
        folderId: 'f1',
      });
      expect(invokeMock).toHaveBeenCalledWith('create_folder', {
        name: '新建夹',
        parentId: null,
      });
      expect(invokeMock).toHaveBeenCalledWith('add_bookmark_to_folder', {
        bookmarkId: 'b1',
        folderId: 'f3',
      });
      expect(invokeMock).toHaveBeenCalledWith('remove_tag_from_bookmark', {
        bookmarkId: 'b1',
        tagName: '效率',
      });
      expect(invokeMock).toHaveBeenCalledWith('add_tag_to_bookmark', {
        bookmarkId: 'b1',
        tagName: '重要',
      });
    });
  });

  it('点击删除文件夹应调用 delete_folder', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    render(<App />);
    const deleteButtons = await screen.findAllByRole('button', { name: '删除文件夹' });
    fireEvent.click(deleteButtons[0]);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('delete_folder', { id: 'f1' });
    });
  });

  it('搜索输入变化应调用 search_bookmarks', async () => {
    render(<App />);

    const input = await screen.findByPlaceholderText('搜索标题、域名或标签...');
    fireEvent.change(input, { target: { value: 'xiao' } });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('search_bookmarks', { query: 'xiao' });
    });
  });

  it('启动时应按事件同步设置执行 pull-only', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_folders') return [];
      if (cmd === 'get_tags') return [];
      if (cmd === 'get_bookmarks') return [];
      if (cmd === 'get_delete_sync_setting') return false;
      if (cmd === 'get_browser_auto_sync_settings') {
        return { startup_enabled: false, interval_enabled: false, interval_minutes: 5 };
      }
      if (cmd === 'get_git_sync_repo_dir') return '';
      if (cmd === 'get_event_auto_sync_settings') {
        return {
          startup_pull_enabled: true,
          interval_enabled: false,
          interval_minutes: 5,
          close_push_enabled: true,
        };
      }
      return null;
    });
    render(<App />);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('sync_event_pull_only');
    });
  });

  it('事件定时同步开启后应按间隔调用增量同步', async () => {
    vi.useFakeTimers();
    try {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_folders') return [];
        if (cmd === 'get_tags') return [];
        if (cmd === 'get_bookmarks') return [];
        if (cmd === 'get_delete_sync_setting') return false;
        if (cmd === 'get_browser_auto_sync_settings') {
          return { startup_enabled: false, interval_enabled: false, interval_minutes: 5 };
        }
        if (cmd === 'get_git_sync_repo_dir') return '';
        if (cmd === 'get_event_auto_sync_settings') {
          return {
            startup_pull_enabled: false,
            interval_enabled: true,
            interval_minutes: 5,
            close_push_enabled: true,
          };
        }
        return null;
      });

      render(<App />);
      await vi.advanceTimersByTimeAsync(0);
      expect(invokeMock).toHaveBeenCalledWith('get_event_auto_sync_settings');

      await vi.advanceTimersByTimeAsync(5 * 60 * 1000);
      expect(invokeMock).toHaveBeenCalledWith('sync_github_incremental');
    } finally {
      vi.useRealTimers();
    }
  });

  it('启动时应加载外观设置', async () => {
    render(<App />);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('get_ui_appearance_settings');
    });
  });

  it('设置页切换主题后应保存外观设置', async () => {
    const { container } = render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: '打开设置' }));
    fireEvent.click(await screen.findByRole('button', { name: '主题：跟随系统' }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('set_ui_appearance_settings', {
        themeMode: 'light',
        backgroundEnabled: false,
        backgroundImageDataUrl: null,
        backgroundOverlayOpacity: 45,
      });
    });
    await waitFor(() => {
      expect((container.firstChild as HTMLElement | null)?.getAttribute('data-theme')).toBe('light');
    });
  });

  it('设置页操作按钮应使用语义化按钮样式类', async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: '打开设置' }));
    const themeButton = await screen.findByRole('button', { name: '主题：跟随系统' });
    expect(themeButton.className).toContain('btn-neutral');
    const syncButton = await screen.findByRole('button', { name: '立即同步' });
    expect(syncButton.className).toContain('btn-accent');
  });

  it('主界面关键操作按钮应使用语义化按钮样式类', async () => {
    render(<App />);
    const addButton = await screen.findByRole('button', { name: '添加' });
    expect(addButton.className).toContain('btn-accent');
    const deleteButtons = await screen.findAllByRole('button', { name: '🗑️' });
    expect(deleteButtons[0].className).toContain('btn-danger');
  });

  it('侧栏选中项应使用语义化导航样式类', async () => {
    render(<App />);
    const allButton = await screen.findByRole('button', { name: '🏠 全部书签' });
    expect(allButton.className).toContain('nav-item-active');
    const folderButton = await screen.findByText('工作');
    fireEvent.click(folderButton);
    expect(folderButton.closest('div')?.className).toContain('nav-item-active');
  });

  it('设置页输入框和面板应使用语义化样式类', async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: '打开设置' }));
    const repoInput = await screen.findByPlaceholderText('本机 Git 仓库目录（必须已 git init/clone）');
    expect(repoInput.className).toContain('input-field');
    const panelLabel = await screen.findByText('主题与背景');
    expect(panelLabel.closest('div')?.className).toContain('panel-section');
  });

  it('书签卡片的新增标签按钮应与编辑删除同一行且使用图标按钮样式', async () => {
    render(<App />);
    const addTagButton = await screen.findByRole('button', { name: '新增标签-示例' });
    expect(addTagButton.className).toContain('btn-icon');
    const editButton = await screen.findByRole('button', { name: '编辑书签-示例' });
    expect(addTagButton.parentElement).toBe(editButton.parentElement);
  });

  it('新增标签保存时应调用 add_tag_to_bookmark（自动去除首尾空格）', async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: '新增标签-示例' }));
    const input = await screen.findByPlaceholderText('标签名称 (如: 工作, 灵感)');
    fireEvent.change(input, { target: { value: '  待阅读  ' } });
    fireEvent.click(screen.getByRole('button', { name: '保存标签' }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('add_tag_to_bookmark', {
        bookmarkId: 'b1',
        tagName: '待阅读',
      });
    });
  });
});
