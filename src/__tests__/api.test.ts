import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import { createApiRouter } from '../server/api';
import { EventEmitter } from 'events';

// Minimal mock multiplexer for API tests
function createMockMultiplexer() {
  const emitter = new EventEmitter();
  const devices = new Map<string, any>();

  const multiplexer = Object.assign(emitter, {
    getStatus() {
      return {
        receiverRunning: false,
        receiverName: 'Test',
        streaming: false,
        metadata: {},
        devices: Array.from(devices.values()),
        httpPort: 5000,
      };
    },
    async setDeviceVolume(id: string, volume: number) {
      if (!devices.has(id)) throw new Error(`Device not found: ${id}`);
      devices.get(id).volume = volume;
    },
    async setDeviceMute(id: string, muted: boolean) {
      if (!devices.has(id)) throw new Error(`Device not found: ${id}`);
      devices.get(id).muted = muted;
    },
    async setDeviceEnabled(id: string, enabled: boolean) {
      if (!devices.has(id)) throw new Error(`Device not found: ${id}`);
      devices.get(id).enabled = enabled;
    },
    async setMasterVolume(volume: number) {
      for (const dev of devices.values()) {
        dev.volume = volume;
      }
    },
    _addDevice(id: string, data: any) {
      devices.set(id, { id, volume: 50, muted: false, enabled: true, ...data });
    },
  });

  return multiplexer;
}

// Minimal mock Request/Response for testing the router handlers
function createMockReq(overrides: any = {}) {
  return {
    params: {},
    body: {},
    on: () => {},
    ...overrides,
  };
}

function createMockRes() {
  let statusCode = 200;
  let body: any = null;
  let headers: Record<string, string> = {};

  return {
    status(code: number) {
      statusCode = code;
      return this;
    },
    json(data: any) {
      body = data;
      return this;
    },
    writeHead(code: number, hdrs: Record<string, string>) {
      statusCode = code;
      headers = hdrs;
    },
    write(data: string) {},
    getStatusCode: () => statusCode,
    getBody: () => body,
    getHeaders: () => headers,
  };
}

describe('API Router', () => {
  let mockMultiplexer: ReturnType<typeof createMockMultiplexer>;
  let router: ReturnType<typeof createApiRouter>;

  beforeEach(() => {
    mockMultiplexer = createMockMultiplexer();
    router = createApiRouter(() => mockMultiplexer);
  });

  // Helper to find and call a route handler
  function findHandler(method: string, path: string) {
    const layer = (router as any).stack.find((l: any) => {
      return l.route?.path === path && l.route?.methods[method];
    });
    if (!layer) throw new Error(`Route not found: ${method.toUpperCase()} ${path}`);
    return layer.route.stack[0].handle;
  }

  describe('GET /status', () => {
    it('returns multiplexer status', () => {
      const handler = findHandler('get', '/status');
      const req = createMockReq();
      const res = createMockRes();

      handler(req, res);

      const body = res.getBody();
      assert.strictEqual(body.receiverName, 'Test');
      assert.strictEqual(body.streaming, false);
      assert.ok(Array.isArray(body.devices));
    });
  });

  describe('GET /devices', () => {
    it('returns device list', () => {
      mockMultiplexer._addDevice('dev-1', { name: 'Speaker 1', type: 'sonos' });
      const handler = findHandler('get', '/devices');
      const req = createMockReq();
      const res = createMockRes();

      handler(req, res);

      const body = res.getBody();
      assert.strictEqual(body.length, 1);
    });
  });

  describe('POST /devices/:id/volume', () => {
    it('rejects non-numeric volume', async () => {
      const handler = findHandler('post', '/devices/:id/volume');
      const req = createMockReq({ params: { id: 'dev-1' }, body: { volume: 'loud' } });
      const res = createMockRes();

      handler(req, res);

      assert.strictEqual(res.getStatusCode(), 400);
      assert.ok(res.getBody().error.includes('Volume'));
    });

    it('rejects volume below 0', async () => {
      const handler = findHandler('post', '/devices/:id/volume');
      const req = createMockReq({ params: { id: 'dev-1' }, body: { volume: -5 } });
      const res = createMockRes();

      handler(req, res);

      assert.strictEqual(res.getStatusCode(), 400);
    });

    it('rejects volume above 100', async () => {
      const handler = findHandler('post', '/devices/:id/volume');
      const req = createMockReq({ params: { id: 'dev-1' }, body: { volume: 150 } });
      const res = createMockRes();

      handler(req, res);

      assert.strictEqual(res.getStatusCode(), 400);
    });

    it('sets volume on valid request', async () => {
      mockMultiplexer._addDevice('dev-1', { name: 'Speaker 1' });
      const handler = findHandler('post', '/devices/:id/volume');
      const req = createMockReq({ params: { id: 'dev-1' }, body: { volume: 75 } });
      const res = createMockRes();

      await handler(req, res);
      // Allow promise to resolve
      await new Promise((r) => setTimeout(r, 10));
    });
  });

  describe('POST /devices/:id/mute', () => {
    it('rejects non-boolean muted', () => {
      const handler = findHandler('post', '/devices/:id/mute');
      const req = createMockReq({ params: { id: 'dev-1' }, body: { muted: 'yes' } });
      const res = createMockRes();

      handler(req, res);

      assert.strictEqual(res.getStatusCode(), 400);
      assert.ok(res.getBody().error.includes('boolean'));
    });

    it('accepts valid muted boolean', async () => {
      mockMultiplexer._addDevice('dev-1', { name: 'Speaker 1' });
      const handler = findHandler('post', '/devices/:id/mute');
      const req = createMockReq({ params: { id: 'dev-1' }, body: { muted: true } });
      const res = createMockRes();

      await handler(req, res);
      await new Promise((r) => setTimeout(r, 10));
    });
  });

  describe('POST /devices/:id/enable', () => {
    it('rejects non-boolean enabled', () => {
      const handler = findHandler('post', '/devices/:id/enable');
      const req = createMockReq({ params: { id: 'dev-1' }, body: { enabled: 1 } });
      const res = createMockRes();

      handler(req, res);

      assert.strictEqual(res.getStatusCode(), 400);
      assert.ok(res.getBody().error.includes('boolean'));
    });

    it('accepts valid enabled boolean', async () => {
      mockMultiplexer._addDevice('dev-1', { name: 'Speaker 1' });
      const handler = findHandler('post', '/devices/:id/enable');
      const req = createMockReq({ params: { id: 'dev-1' }, body: { enabled: false } });
      const res = createMockRes();

      await handler(req, res);
      await new Promise((r) => setTimeout(r, 10));
    });
  });

  describe('POST /master-volume', () => {
    it('rejects invalid volume', () => {
      const handler = findHandler('post', '/master-volume');
      const req = createMockReq({ body: { volume: 'max' } });
      const res = createMockRes();

      handler(req, res);

      assert.strictEqual(res.getStatusCode(), 400);
    });

    it('accepts valid volume', async () => {
      const handler = findHandler('post', '/master-volume');
      const req = createMockReq({ body: { volume: 60 } });
      const res = createMockRes();

      await handler(req, res);
      await new Promise((r) => setTimeout(r, 10));
    });
  });
});
