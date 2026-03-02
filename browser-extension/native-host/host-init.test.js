const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('path');

test('require host.js 不应因日志初始化顺序崩溃', () => {
  const hostPath = path.resolve(__dirname, 'host.js');
  assert.doesNotThrow(() => {
    require(hostPath);
  });
});
