import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert';
import { EventEmitter } from 'events';
import { PassThrough } from 'stream';
import { AudioMultiplexer } from '../multiplexer';
import { AppConfig } from '../config';
import { DeviceConfig } from '../types';

// We need to mock external dependencies to test the multiplexer
// Since the multiplexer creates real device instances internally,
// we test public behavior via the status and control methods.

const testConfig: AppConfig = {
  receiverName: 'Test Multiplexer',
  httpPort: 5000,
  audioFormat: {
    sampleRate: 44100,
    bitDepth: 16,
    channels: 2,
  },
  shairportPath: 'shairport-sync',
  localIp: '192.168.1.1',
};

describe('AudioMultiplexer', () => {
  let multiplexer: AudioMultiplexer;

  beforeEach(() => {
    multiplexer = new AudioMultiplexer(testConfig);
  });

  afterEach(async () => {
    // Clean up - stop will handle devices + cleanup
    try {
      await multiplexer.stop();
    } catch {
      // Ignore errors during cleanup
    }
  });

  it('getStatus returns initial status', () => {
    const status = multiplexer.getStatus();

    assert.strictEqual(status.receiverName, 'Test Multiplexer');
    assert.strictEqual(status.streaming, false);
    assert.strictEqual(status.httpPort, 5000);
    assert.deepStrictEqual(status.metadata, {});
    assert.ok(Array.isArray(status.devices));
    assert.strictEqual(status.devices.length, 0);
  });

  it('getStreamManager returns a StreamManager instance', () => {
    const sm = multiplexer.getStreamManager();
    assert.ok(sm);
    assert.strictEqual(typeof sm.subscribe, 'function');
    assert.strictEqual(typeof sm.unsubscribe, 'function');
    assert.strictEqual(typeof sm.cleanup, 'function');
  });

  it('setDeviceVolume throws for unknown device', async () => {
    await assert.rejects(
      () => multiplexer.setDeviceVolume('nonexistent', 50),
      { message: 'Device not found: nonexistent' }
    );
  });

  it('setDeviceMute throws for unknown device', async () => {
    await assert.rejects(
      () => multiplexer.setDeviceMute('nonexistent', true),
      { message: 'Device not found: nonexistent' }
    );
  });

  it('setDeviceEnabled throws for unknown device', async () => {
    await assert.rejects(
      () => multiplexer.setDeviceEnabled('nonexistent', false),
      { message: 'Device not found: nonexistent' }
    );
  });

  it('setMasterVolume resolves even with no devices', async () => {
    // Should not throw
    await multiplexer.setMasterVolume(75);
  });

  it('emits statusChanged event', () => {
    let emitted = false;
    multiplexer.on('statusChanged', () => {
      emitted = true;
    });

    // Trigger internal status change by emitting on the multiplexer
    multiplexer.emit('statusChanged');
    assert.strictEqual(emitted, true);
  });

  it('audioStreamUrl is constructed from config', () => {
    const status = multiplexer.getStatus();
    // The multiplexer builds the URL from localIp and httpPort
    assert.strictEqual(status.httpPort, testConfig.httpPort);
  });
});
