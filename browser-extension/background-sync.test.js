const test = require('node:test');
const assert = require('node:assert/strict');

const { flattenBookmarkTree, buildPageCapturedMessage } = require('./background-sync');

test('flattenBookmarkTree 仅输出有 url 的节点', () => {
  const tree = [{
    id: '0', title: 'root', children: [
      { id: '1', title: 'folder', children: [{ id: '2', title: 'A', url: 'https://a.com' }] },
      { id: '3', title: 'B', url: 'https://b.com' },
    ],
  }];

  const result = flattenBookmarkTree(tree);
  assert.deepEqual(result.map(x => x.url), ['https://a.com', 'https://b.com']);
});

test('buildPageCapturedMessage 应构造 PageCaptured 事件', () => {
  const msg = buildPageCapturedMessage({ id: 11, url: 'https://c.com', title: 'C' });
  assert.equal(msg.type, 'PageCaptured');
  assert.equal(msg.payload.url, 'https://c.com');
  assert.equal(msg.payload.title, 'C');
});
